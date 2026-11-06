// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The Unicorn harness: map `eboot.bin`, stub the handful of imports the codecs reach, and call a
//! function by address.
//!
//! Port of `capture-rig/oodle/d1-oodle-harness.py`. Every warning comment in that file is carried
//! across, because each one is a paid-for failure mode:
//!
//! * **AVX is not available in this Unicorn.** Destiny's RSAT codecs are AVX-compiled; `vxorps
//!   ymm0,ymm0,ymm0` raises `UC_ERR_INSN_INVALID`, which reads like a mapping fault but is a missing
//!   CPU feature. Setting a CPU model does **not** fix it. [`Harness`] installs the same
//!   **fail-closed** shim: two encodings emulated by hand, everything else still raises.
//! * **A `memset` through an unpopulated scaffolding pointer wipes the stack**, the return address
//!   goes with it, and the run then "dispatches to a null import" - a harness artefact that looks
//!   exactly like a client defect. Refused by default; see [`Stub::MemSet`].
//! * **One hook, resolved late.** The Python harness registered `self._code` by value, so a caller
//!   that replaced the attribute got its wrapper registered as a *second* hook while the original
//!   still ran - every import stub executed twice, `_ret` popped the stack twice, and RIP landed on
//!   garbage. Here there is exactly one code hook, ever, and per-address behaviour is **data**
//!   ([`Stub`]) rather than a replaced callback. The trap is closed by construction.
//!
//! Nothing in this module knows anything about RSAT. It maps an image and calls functions.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use unicorn_engine::{Arch, Mode, Prot, RegisterX86, Unicorn};

use d1_eboot::image::Image;
use d1_eboot::reloc;

use crate::error::{OracleError, Result};

/// Guest window: `[0, GUEST_LIMIT)` holds the whole image including BSS. Every LOAD segment is
/// asserted to fit - a different eboot fails loudly instead of being half-mapped.
pub const GUEST_LIMIT: u64 = 0x300_0000;
pub const STACK: u64 = 0x400_0000;
pub const STACK_SIZE: u64 = 0x20_0000;
pub const HEAP: u64 = 0x500_0000;
pub const HEAP_SIZE: u64 = 0x100_0000;
/// The largest length a stubbed `memcpy`/`memset`/`alloc_aligned` will act on. Every guest buffer
/// is in the image, the stack or the heap, so a longer request means the call's arguments are wrong.
pub const MAX_GUEST_LEN: u64 = 0x1000_0000;
/// The return trampoline. `call()` pushes this and stops when RIP reaches it.
pub const STOP: u64 = 0x7ff_f000;
pub const CANARY_PTR: u64 = 0x7ff_e000;
pub const CANARY: u64 = 0x1122_3344_5566_7788;
/// Stack-protector indirection these functions read through.
pub const CANARY_SLOT: u64 = 0x1A0_6C78;

/// Matches `d1-oodle-harness.py`. A run that needs more than this is a loop, not a codec.
pub const STEP_LIMIT: u64 = 50_000_000;

// ── imports the codecs reach, by address ─────────────────────────────────────────────────────────
pub const IMP_NOP: u64 = 0x00F8_ED00;
pub const IMP_ALLOC_ALIGNED: u64 = 0x0127_3000;
pub const IMP_FREE: u64 = 0x0127_3090;
pub const IMP_MEMMOVE: u64 = 0x0129_D080;
pub const IMP_MEMSET: u64 = 0x0129_D240;
pub const IMP_MEMCPY: u64 = 0x0129_D370;
pub const IMP_STACK_CHK_FAIL: u64 = 0x0129_D0B0;

/// The harness's image-specific addresses, chosen by the loaded image's SHA-256. The `IMP_*`
/// constants above are the 01.29 reference set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildAddrs {
    pub canary_slot: u64,
    /// A function the 01.29 decode path reaches that is stubbed to return 0. Not yet identified
    /// for 56578; `None` leaves the game's own code to run.
    pub imp_nop: Option<u64>,
    pub alloc_aligned: u64,
    pub free: u64,
    pub memcpy: u64,
    pub memmove: u64,
    pub memset: u64,
    pub stack_chk_fail: u64,
}

pub const ADDRS_0129: BuildAddrs = BuildAddrs {
    canary_slot: CANARY_SLOT,
    imp_nop: Some(IMP_NOP),
    alloc_aligned: IMP_ALLOC_ALIGNED,
    free: IMP_FREE,
    memcpy: IMP_MEMCPY,
    memmove: IMP_MEMMOVE,
    memset: IMP_MEMSET,
    stack_chk_fail: IMP_STACK_CHK_FAIL,
};

/// Build 56578 (01.33). PLT stubs confirmed by NID (memcpy `Q3VBxCXhUHs`, memmove `+P6FRGH4LfA`,
/// memset `8zTFvBIAIN8`, `__stack_chk_fail` `Ou3iL1abvng`); the stack guard slot is the one every
/// 56578 prologue reads; the allocator pair are Oodle's plugin-routed aligned alloc / free
/// (`[0x2954538]` / `[0x2954540]`), at the same offset from `DecodeSome` as in 01.29.
pub const ADDRS_56578: BuildAddrs = BuildAddrs {
    canary_slot: 0x01A1_F100,
    imp_nop: None,
    alloc_aligned: 0x0127_8720,
    free: 0x0127_87B0,
    memcpy: 0x012A_A7C0,
    memmove: 0x012A_AAB0,
    memset: 0x012A_A980,
    stack_chk_fail: 0x012A_A7F0,
};

/// The address set for an image, by its SHA-256. `None` = no measured set: refuse to run.
pub fn addrs_for(sha256: &str) -> Option<BuildAddrs> {
    match sha256 {
        "7271fcb926401df8defb126cb8eb2b247134138b81e401bbacb2ab60791b7954" => Some(ADDRS_0129),
        "672f03411c0503dfbcda804cbb9fb27cdd9a8d73a8bcb71bf0c4f688073b0833" => Some(ADDRS_56578),
        _ => None,
    }
}

const ARG_REGS: [RegisterX86; 6] = [
    RegisterX86::RDI,
    RegisterX86::RSI,
    RegisterX86::RDX,
    RegisterX86::RCX,
    RegisterX86::R8,
    RegisterX86::R9,
];

/// What the single code hook does at a given address. Behaviour is **data**, never a replaced
/// callback - see the module docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stub {
    /// `_ret(value)`: pop the return address, set `rax`, resume there.
    Return(u64),
    /// `rdi = size`, `rsi = alignment` → bump-allocate and return the pointer.
    AllocAligned,
    /// `free` - a no-op; the bump heap is reclaimed with the harness.
    Free,
    /// `memcpy`/`memmove`: `rdi = dst`, `rsi = src`, `rdx = n`.
    MemCopy,
    /// `memset`: `rdi = dst`, `rsi = value`, `rdx = n`.
    ///
    /// ⚠ Refuses any destination overlapping the guest stack **unless** the return address is in
    /// [`Harness::allow_stack_memset_from`]. The RSAT codec's dynamic-field bitset is an ordinary
    /// frame local and is memset twice per encode, so a blanket refusal is too wide - but a blanket
    /// permission would let an unpopulated pointer silently wipe a return address.
    MemSet,
    /// `__stack_chk_fail` - always a fault.
    StackChkFail,
    /// Halt: the return trampoline.
    Stop,
    /// Dispatched through a null function pointer. **Always a fault**, never a silent stub: an
    /// unrelocated pointer and an optional callback are indistinguishable without the call site.
    NullDispatch,
}

struct State {
    heap_next: u64,
    /// ⭐ The furthest `heap_next` has ever reached, **not** reset by [`State::rewind`]. This is the
    /// number a leak control watches: a bump arena that is rewound correctly has a high-water mark
    /// that stops rising after the first full cycle, and a leaking one has one that rises forever.
    heap_high_water: u64,
    /// Allocations made by guest code through [`Stub::AllocAligned`], as opposed to ones the host
    /// made through [`Harness::alloc`]. ⭐ **The rewind's safety hinges on this being zero across a
    /// codec call**: a guest allocation whose pointer outlives the call would be handed out twice.
    /// Counted rather than assumed - see [`Harness::guest_allocs`].
    guest_allocs: u64,
    steps: u64,
    step_limit: u64,
    fault: Option<String>,
    stubs: HashMap<u64, Stub>,
    /// Return addresses at which a stack-overlapping `memset` is legitimate.
    memset_allow: HashSet<u64>,
    /// Shim-visible YMM state - only what the AVX shim actually emulates.
    ymm: HashMap<u8, [u8; 32]>,
    notes: Vec<String>,
}

impl State {
    fn alloc(&mut self, size: u64, alignment: u64) -> Result<u64> {
        let a = alignment.max(16);
        let p = (self.heap_next + a - 1) & !(a - 1);
        let next = p + size.max(1);
        if next > HEAP + HEAP_SIZE {
            return Err(OracleError::Fault(format!(
                "scratch heap exhausted: wanted {size:#x} bytes at {p:#x}, the arena is \
                 {HEAP:#x}..{:#x} ({HEAP_SIZE:#x} bytes) and {} are already committed. This is a \
                 BUMP arena - if the caller is in a loop it must rewind between iterations \
                 (`Harness::heap_rewind`); enlarging the arena only moves this failure later",
                HEAP + HEAP_SIZE,
                self.heap_next - HEAP
            )));
        }
        self.heap_next = next;
        self.heap_high_water = self.heap_high_water.max(next);
        Ok(p)
    }

    /// Release everything allocated after `mark`. Refuses a mark outside `HEAP..=heap_next`, so a
    /// stale or fabricated mark is an error rather than a pointer handed out twice.
    fn rewind(&mut self, mark: u64) -> Result<()> {
        if mark < HEAP || mark > self.heap_next {
            return Err(OracleError::Fault(format!(
                "heap rewind to {mark:#x} is outside the live arena {HEAP:#x}..{:#x} - a mark from \
                 a different harness, or one already rewound past",
                self.heap_next
            )));
        }
        self.heap_next = mark;
        Ok(())
    }
}

pub struct Harness {
    uc: Unicorn<'static, ()>,
    st: Rc<RefCell<State>>,
    scalar_float: Option<crate::scalar_float::ScalarFloat>,
}

fn uce(e: unicorn_engine::uc_error) -> OracleError {
    OracleError::Unicorn(format!("{e:?}"))
}

/// `_ret(rax)` - pop the return address and resume there. The one primitive every stub uses.
fn do_ret(uc: &mut Unicorn<'_, ()>, rax: u64) -> std::result::Result<(), unicorn_engine::uc_error> {
    let rsp = uc.reg_read(RegisterX86::RSP)?;
    let mut b = [0u8; 8];
    uc.mem_read(rsp, &mut b)?;
    uc.reg_write(RegisterX86::RAX, rax)?;
    uc.reg_write(RegisterX86::RSP, rsp + 8)?;
    uc.reg_write(RegisterX86::RIP, u64::from_le_bytes(b))?;
    Ok(())
}

impl Harness {
    /// Map `image`, apply its `R_X86_64_RELATIVE` relocations, and install the standard stubs.
    ///
    /// ⚠ Relocations are **not** optional and **not** hardcoded here: PS4 eboots store pointer
    /// tables as zero in the file and fill them at load. The table bounds come from
    /// [`d1_eboot::reloc`], which derives them from `PT_DYNAMIC` rather than remembering
    /// `0x1A0D100 + 0xBDD0` (ATLAS §4.3 - a superseded constant surviving in a sibling tool is a
    /// failure this project has already paid for).
    pub fn new(image: &Image) -> Result<Self> {
        let addrs = addrs_for(&image.sha256()).ok_or_else(|| {
            OracleError::Eboot(format!(
                "no harness address set for image sha256 {} - stubs would land on the wrong code",
                image.sha256()
            ))
        })?;
        let mut uc = Unicorn::new(Arch::X86, Mode::MODE_64).map_err(uce)?;
        uc.mem_map(0, GUEST_LIMIT, Prot::ALL).map_err(uce)?;
        uc.mem_map(STACK, STACK_SIZE, Prot::ALL).map_err(uce)?;
        uc.mem_map(HEAP, HEAP_SIZE, Prot::ALL).map_err(uce)?;
        uc.mem_map(STOP & !0xFFF, 0x1000, Prot::ALL).map_err(uce)?;
        uc.mem_map(CANARY_PTR & !0xFFF, 0x1000, Prot::ALL).map_err(uce)?;

        for s in image.segments().iter().filter(|s| s.is_load() && s.filesz > 0) {
            let end = s.vaddr + s.memsz.max(s.filesz);
            if end > GUEST_LIMIT {
                return Err(OracleError::Eboot(format!(
                    "LOAD segment {} spans {:#x}..{:#x}, past the {GUEST_LIMIT:#x} guest window - \
                     this is not the pinned image the harness constants were measured on",
                    s.index, s.vaddr, end
                )));
            }
            let bytes = image
                .raw(s.offset as usize, s.filesz as usize)
                .map_err(|e| OracleError::Eboot(e.to_string()))?;
            uc.mem_write(s.vaddr, bytes).map_err(uce)?;
        }

        for r in reloc::relative(image).map_err(|e| OracleError::Eboot(e.to_string()))? {
            // `+ 8`: the addend is a qword, so a slot in the last seven bytes of the window would
            // spill past it. There is no such relocation in the pinned image; the bound is here so
            // that a different one faults instead of writing outside the map.
            if r.slot + 8 <= GUEST_LIMIT {
                uc.mem_write(r.slot, &r.addend.to_le_bytes()).map_err(uce)?;
            }
        }

        uc.mem_write(STOP, &[0xCC]).map_err(uce)?;
        uc.mem_write(CANARY_PTR, &CANARY.to_le_bytes()).map_err(uce)?;
        uc.mem_write(addrs.canary_slot, &CANARY_PTR.to_le_bytes()).map_err(uce)?;

        let mut stubs = HashMap::new();
        stubs.insert(0u64, Stub::NullDispatch);
        stubs.insert(STOP, Stub::Stop);
        if let Some(nop) = addrs.imp_nop {
            stubs.insert(nop, Stub::Return(0));
        }
        stubs.insert(addrs.alloc_aligned, Stub::AllocAligned);
        stubs.insert(addrs.free, Stub::Free);
        stubs.insert(addrs.memmove, Stub::MemCopy);
        stubs.insert(addrs.memcpy, Stub::MemCopy);
        stubs.insert(addrs.memset, Stub::MemSet);
        stubs.insert(addrs.stack_chk_fail, Stub::StackChkFail);

        let st = Rc::new(RefCell::new(State {
            heap_next: HEAP,
            heap_high_water: HEAP,
            guest_allocs: 0,
            steps: 0,
            step_limit: STEP_LIMIT,
            fault: None,
            stubs,
            memset_allow: HashSet::new(),
            ymm: HashMap::new(),
            notes: Vec::new(),
        }));

        // ── THE ONE CODE HOOK ────────────────────────────────────────────────────────────────────
        // `(1, 0)` is Unicorn's "every address" idiom (begin > end). There is exactly one, for the
        // life of the harness; callers change *data*, never this callback.
        let hs = st.clone();
        uc.add_code_hook(1, 0, move |uc, address, _size| {
            let stub = {
                let mut s = hs.borrow_mut();
                s.steps += 1;
                if s.steps > s.step_limit && s.fault.is_none() {
                    let n = s.step_limit;
                    s.fault = Some(format!("instruction limit ({n}) exceeded"));
                }
                if s.fault.is_some() {
                    None
                } else {
                    s.stubs.get(&address).copied()
                }
            };
            let Some(stub) = stub else {
                if hs.borrow().fault.is_some() {
                    let _ = uc.emu_stop();
                }
                return;
            };
            let r = dispatch(&hs, uc, address, stub);
            if let Err(msg) = r {
                let mut s = hs.borrow_mut();
                if s.fault.is_none() {
                    s.fault = Some(msg);
                }
                drop(s);
                let _ = uc.emu_stop();
            }
        })
        .map_err(uce)?;

        // ── AVX shim, FAIL CLOSED ────────────────────────────────────────────────────────────────
        let hs = st.clone();
        uc.add_insn_invalid_hook(move |uc| avx_shim(&hs, uc)).map_err(uce)?;

        // ── interrupts ───────────────────────────────────────────────────────────────────────────
        let hs = st.clone();
        uc.add_intr_hook(move |uc, intno| {
            if intno == 3 || intno == 0x41 {
                // Assertions are diagnostic; the Python steps past the two-byte int.
                hs.borrow_mut().notes.push(format!("int {intno:#x} ignored"));
                return;
            }
            let rip = uc.pc_read().unwrap_or(0);
            let mut s = hs.borrow_mut();
            if s.fault.is_none() {
                s.fault = Some(format!("interrupt {intno:#x} at RIP {rip:#x}"));
            }
            drop(s);
            let _ = uc.emu_stop();
        })
        .map_err(uce)?;

        Ok(Self {
            uc,
            st,
            scalar_float: None,
        })
    }

    /// Correct measured VEX scalar aliasing in explicitly identified RSAT ranges only.
    pub(crate) fn correct_rsat_scalar_float(&mut self, ranges: &[(u64, u64)]) -> Result<()> {
        if !ranges.is_empty() {
            self.scalar_float = Some(crate::scalar_float::ScalarFloat::install_ieee(
                &mut self.uc,
                ranges,
            )?);
        }
        Ok(())
    }

    /// Open the pinned eboot (or `$D1_EBOOT`) and build a harness on it.
    pub fn open_default() -> Result<(Image, Self)> {
        let image = Image::open_default().map_err(|e| OracleError::Eboot(e.to_string()))?;
        let h = Self::new(&image)?;
        Ok((image, h))
    }

    /// [`Harness::open_default`] on the 01.29 address reference (tests and controls).
    pub fn open_reference() -> Result<(Image, Self)> {
        let image = Image::open_reference().map_err(|e| OracleError::Eboot(e.to_string()))?;
        let h = Self::new(&image)?;
        Ok((image, h))
    }

    /// The underlying engine, for callers that need their own hooks, registers or single-stepping.
    ///
    /// ⚠ Adding a **second** global `UC_HOOK_CODE` is safe here (this harness owns exactly one and
    /// never replaces it), but adding one that also calls `_ret` is not: two handlers popping the
    /// same return address is the Python bug this port exists to make impossible. Observe freely;
    /// dispatch through [`Harness::stub`].
    pub fn uc(&mut self) -> &mut Unicorn<'static, ()> {
        &mut self.uc
    }

    /// Map an additional guest region - e.g. a live-process snapshot rehydrated alongside the
    /// static image. `address` and `size` are page-aligned outward by Unicorn's rules, so pass
    /// page-aligned values.
    ///
    /// ⚠ This does **not** overlap-check against the regions [`Harness::new`] already mapped (the
    /// guest window `0 .. GUEST_LIMIT`, the stack, the heap, the two trampoline pages); Unicorn
    /// returns `UC_ERR_MAP` if you collide, which is the fail-closed answer.
    pub fn map_region(&mut self, address: u64, size: u64, perms: Prot) -> Result<()> {
        self.uc.mem_map(address, size, perms).map_err(uce)
    }

    /// Bump-allocate zeroed guest memory.
    pub fn alloc(&mut self, size: u64) -> Result<u64> {
        self.alloc_aligned(size, 16)
    }

    pub fn alloc_aligned(&mut self, size: u64, alignment: u64) -> Result<u64> {
        let p = self.st.borrow_mut().alloc(size, alignment)?;
        let zeros = vec![0u8; size.max(1) as usize];
        self.uc.mem_write(p, &zeros).map_err(uce)?;
        Ok(p)
    }

    // ── the bump arena, and the one way to give memory back ──────────────────────────────────────
    //
    // ⭐⭐ **THE ARENA IS A BUMP POINTER AND `free` IS A NO-OP** ([`Stub::Free`]). That is the right
    // shape for a one-shot oracle and a **leak** for a long-lived one: `d1-activity-host` re-encodes
    // `global_activity_state` on every 5,125 ms heartbeat, each encode taking `0x1DF00` bytes of
    // scratch, and on 2026-08-09 run `20260809-120451-MOONBASE-FRESH` the 133rd encode died with
    // `scratch heap exhausted` sixteen minutes into an activity - a *silent, time-delayed* failure
    // whose observable was `error.private_activity_host_host_crashed`, i.e. it reads as the client
    // rejecting us.
    //
    // The fix is a **mark/rewind**, not a bigger arena: enlarging only moves the failure later,
    // which is the worst outcome for a bug whose signature is "fails after N minutes".

    /// The current bump pointer. Pair with [`Harness::heap_rewind`] to scope an allocation.
    pub fn heap_mark(&self) -> u64 {
        self.st.borrow().heap_next
    }

    /// Release everything allocated since `mark`.
    ///
    /// ⚠ **Only safe when nothing that outlives the call points into the released region.** The
    /// callers here rewind exactly one codec operation, whose allocations (object image, wire
    /// buffer, bitstream, flush out-param) are all read back into host `Vec`s before returning -
    /// and [`Harness::guest_allocs`] is the control that guest code did not squirrel a pointer away
    /// in the meantime.
    pub fn heap_rewind(&mut self, mark: u64) -> Result<()> {
        self.st.borrow_mut().rewind(mark)
    }

    /// Bytes currently committed in the arena.
    pub fn heap_used(&self) -> u64 {
        self.st.borrow().heap_next - HEAP
    }

    /// ⭐ The furthest the arena has *ever* reached. **The number a leak control watches** - it is
    /// monotonic and survives [`Harness::heap_rewind`], so a flat high-water mark across thousands
    /// of iterations is the proof that the arena is being reused rather than consumed.
    pub fn heap_high_water(&self) -> u64 {
        self.st.borrow().heap_high_water - HEAP
    }

    /// How many allocations **guest code** has made through [`Stub::AllocAligned`]. Host
    /// allocations ([`Harness::alloc`]) are not counted.
    pub fn guest_allocs(&self) -> u64 {
        self.st.borrow().guest_allocs
    }

    pub fn stub(&mut self, address: u64, stub: Stub) {
        self.st.borrow_mut().stubs.insert(address, stub);
    }

    /// Permit a stack-overlapping `memset` when it returns to one of these addresses.
    pub fn allow_stack_memset_from(&mut self, sites: impl IntoIterator<Item = u64>) {
        self.st.borrow_mut().memset_allow.extend(sites);
    }

    pub fn write(&mut self, addr: u64, bytes: &[u8]) -> Result<()> {
        self.uc.mem_write(addr, bytes).map_err(uce)
    }

    pub fn read(&self, addr: u64, len: usize) -> Result<Vec<u8>> {
        self.uc.mem_read_as_vec(addr, len).map_err(uce)
    }

    pub fn write_u16(&mut self, addr: u64, v: u16) -> Result<()> {
        self.write(addr, &v.to_le_bytes())
    }
    pub fn write_u32(&mut self, addr: u64, v: u32) -> Result<()> {
        self.write(addr, &v.to_le_bytes())
    }
    pub fn write_i32(&mut self, addr: u64, v: i32) -> Result<()> {
        self.write(addr, &v.to_le_bytes())
    }
    pub fn write_u64(&mut self, addr: u64, v: u64) -> Result<()> {
        self.write(addr, &v.to_le_bytes())
    }
    pub fn read_u32(&self, addr: u64) -> Result<u32> {
        Ok(u32::from_le_bytes(self.read(addr, 4)?.try_into().unwrap()))
    }
    pub fn read_u64(&self, addr: u64) -> Result<u64> {
        Ok(u64::from_le_bytes(self.read(addr, 8)?.try_into().unwrap()))
    }

    pub fn notes(&self) -> Vec<String> {
        self.st.borrow().notes.clone()
    }

    /// Call a guest function by address, System V AMD64, up to six register args then the stack.
    ///
    /// The frame is rebuilt for every call exactly as `d1-oodle-harness.py` does, so a stale
    /// register from a previous call cannot leak in. ⚠ That matters: `0x72560` takes an out-param
    /// in `rsi`, and omitting it leaves a stale pointer that the flush stores through - which
    /// surfaces as `WRITE_UNMAPPED` from a function that looks argument-less.
    pub fn call(&mut self, addr: u64, args: &[u64]) -> Result<u64> {
        let rsp = STACK + STACK_SIZE - 0x10008;
        self.uc.mem_write(rsp, &STOP.to_le_bytes()).map_err(uce)?;
        for (i, a) in args.iter().take(6).enumerate() {
            self.uc.reg_write(ARG_REGS[i], *a).map_err(uce)?;
        }
        for (i, a) in args.iter().skip(6).enumerate() {
            self.uc.mem_write(rsp + 8 + i as u64 * 8, &a.to_le_bytes()).map_err(uce)?;
        }
        self.uc.reg_write(RegisterX86::RSP, rsp).map_err(uce)?;
        self.uc.reg_write(RegisterX86::RBP, 0).map_err(uce)?;
        {
            let mut s = self.st.borrow_mut();
            s.steps = 0;
            s.fault = None;
        }
        let run = self.uc.emu_start(addr, STOP, 0, 0);
        if let Some(scalar) = &self.scalar_float {
            scalar.check()?;
        }
        if let Some(f) = self.st.borrow().fault.clone() {
            return Err(OracleError::Fault(format!("{f} (calling {addr:#x})")));
        }
        run.map_err(|e| {
            let rip = self.uc.pc_read().unwrap_or(0);
            OracleError::Unicorn(format!("{e:?} calling {addr:#x}, RIP {rip:#x}"))
        })?;
        let pc = self.uc.pc_read().map_err(uce)?;
        if pc != STOP {
            return Err(OracleError::Fault(format!(
                "emulation of {addr:#x} stopped at {pc:#x}, not the return trampoline"
            )));
        }
        self.uc.reg_read(RegisterX86::RAX).map_err(uce)
    }
}

fn dispatch(
    st: &Rc<RefCell<State>>,
    uc: &mut Unicorn<'_, ()>,
    address: u64,
    stub: Stub,
) -> std::result::Result<(), String> {
    let e = |e: unicorn_engine::uc_error| format!("{e:?}");
    // ⚠ FAIL CLOSED on a length no guest buffer can have. A wrong call target or unpopulated
    // scaffolding shows up here as a garbage length; allocating it host-side aborted the whole
    // process ("capacity overflow") instead of reporting where the guest went wrong.
    let bounded = |uc: &mut Unicorn<'_, ()>, what: &str, n: u64| -> std::result::Result<usize, String> {
        if n <= MAX_GUEST_LEN {
            return Ok(n as usize);
        }
        let rsp = uc.reg_read(RegisterX86::RSP).map_err(e)?;
        let mut b = [0u8; 8];
        uc.mem_read(rsp, &mut b).map_err(e)?;
        let r13 = uc.reg_read(RegisterX86::R13).map_err(e)?;
        let rbx = uc.reg_read(RegisterX86::RBX).map_err(e)?;
        let mut stk = vec![0u8; 0x800];
        let _ = uc.mem_read(rsp, &mut stk);
        let frames: Vec<String> = stk
            .chunks_exact(8)
            .map(|c| u64::from_le_bytes(c.try_into().unwrap()))
            .filter(|&q| (0x1000..0x18D_8000).contains(&q))
            .take(16)
            .map(|q| format!("{q:#x}"))
            .collect();
        let rbx = (rbx, frames.join(","));
        Err(format!(
            "{what} with length {n:#x} (return address {:#x}, r13 {r13:#x}, rbx {:#x}, stack {}) - no guest              buffer is that large; the caller's arguments are not what this stub was told they are",
            u64::from_le_bytes(b),
            rbx.0,
            rbx.1
        ))
    };
    match stub {
        Stub::Stop => {
            uc.emu_stop().map_err(e)?;
        }
        Stub::Return(v) => do_ret(uc, v).map_err(e)?,
        Stub::Free => do_ret(uc, 0).map_err(e)?,
        Stub::NullDispatch => {
            let rsp = uc.reg_read(RegisterX86::RSP).map_err(e)?;
            let mut b = [0u8; 8];
            uc.mem_read(rsp, &mut b).map_err(e)?;
            return Err(format!(
                "the native codec dispatched through a NULL function pointer (return address \
                 {:#x}). This is an unrelocated pointer, not an optional callback - do not stub it \
                 blind; find the call site.",
                u64::from_le_bytes(b)
            ));
        }
        Stub::StackChkFail => return Err("__stack_chk_fail".into()),
        Stub::AllocAligned => {
            let n = uc.reg_read(RegisterX86::RDI).map_err(e)?;
            let a = uc.reg_read(RegisterX86::RSI).map_err(e)?;
            let p = {
                let mut s = st.borrow_mut();
                s.guest_allocs += 1;
                s.alloc(n, a).map_err(|x| x.to_string())?
            };
            let len = bounded(uc, "alloc_aligned", n)?.max(1);
            uc.mem_write(p, &vec![0u8; len]).map_err(e)?;
            do_ret(uc, p).map_err(e)?;
        }
        Stub::MemCopy => {
            let dst = uc.reg_read(RegisterX86::RDI).map_err(e)?;
            let src = uc.reg_read(RegisterX86::RSI).map_err(e)?;
            let n = uc.reg_read(RegisterX86::RDX).map_err(e)?;
            let n = bounded(uc, "memcpy/memmove", n)?;
            let buf = uc.mem_read_as_vec(src, n).map_err(e)?;
            uc.mem_write(dst, &buf).map_err(e)?;
            do_ret(uc, dst).map_err(e)?;
        }
        Stub::MemSet => {
            let dst = uc.reg_read(RegisterX86::RDI).map_err(e)?;
            let val = (uc.reg_read(RegisterX86::RSI).map_err(e)? & 0xFF) as u8;
            let n = uc.reg_read(RegisterX86::RDX).map_err(e)?;
            bounded(uc, "memset", n)?;
            if dst < STACK + STACK_SIZE && dst + n > STACK {
                let rsp = uc.reg_read(RegisterX86::RSP).map_err(e)?;
                let mut b = [0u8; 8];
                uc.mem_read(rsp, &mut b).map_err(e)?;
                let ret = u64::from_le_bytes(b);
                if !st.borrow().memset_allow.contains(&ret) {
                    return Err(format!(
                        "memset would clobber the STACK: dst={dst:#x} n={n:#x} rsp={rsp:#x} \
                         return={ret:#x} - the destination pointer is unpopulated scaffolding, fix \
                         the caller's context (or register {ret:#x} as a known codec bitset site)"
                    ));
                }
            }
            uc.mem_write(dst, &vec![val; n.max(1) as usize]).map_err(e)?;
            do_ret(uc, dst).map_err(e)?;
        }
    }
    let _ = address;
    Ok(())
}

/// Emulate the handful of AVX encodings Destiny's codecs use.
///
/// ⚠ **FAIL CLOSED.** Anything not explicitly recognised returns `false` so Unicorn still raises,
/// rather than skipping an instruction and producing a plausible-but-wrong trace. Register contents
/// are never invented: a `vmovaps` from a YMM the shim never saw zeroed also returns `false`.
fn avx_shim(st: &Rc<RefCell<State>>, uc: &mut Unicorn<'_, ()>) -> bool {
    let Ok(rip) = uc.pc_read() else { return false };
    let Ok(b) = uc.mem_read_as_vec(rip, 16) else { return false };
    // VEX.256 `vxorps ymmN, ymmN, ymmN`  ->  zero ymmN      (c5 <vvvv> 57 /r)
    if b[0] == 0xC5 && b[2] == 0x57 {
        let reg = (b[3] >> 3) & 7;
        st.borrow_mut().ymm.insert(reg, [0u8; 32]);
        return uc.reg_write(RegisterX86::RIP, rip + 4).is_ok();
    }
    // VEX.256 `vmovaps [rsp+disp8], ymmN`                    (c5 fc 29 44 24 <disp8>)
    if b[..3] == [0xC5, 0xFC, 0x29] && b[3] == 0x44 && b[4] == 0x24 {
        let reg = (b[3] >> 3) & 7;
        let Some(val) = st.borrow().ymm.get(&reg).copied() else {
            return false; // never invent register contents
        };
        let Ok(rsp) = uc.reg_read(RegisterX86::RSP) else { return false };
        // ⚠ `disp8` is read UNSIGNED here, exactly as the Python shim does. Every observed site
        // has a positive displacement; a negative one must widen this deliberately, not by accident.
        if uc.mem_write(rsp + u64::from(b[5]), &val).is_err() {
            return false;
        }
        return uc.reg_write(RegisterX86::RIP, rip + 6).is_ok();
    }
    st.borrow_mut()
        .notes
        .push(format!("AVX shim: UNHANDLED encoding at {rip:#x}: {:02x?}", &b[..8]));
    false
}

/// Return-address set of every direct `call <target>` (`E8 rel32`) in `[lo, hi)`.
///
/// A **byte scan**, not a linear disassembly: capstone/iced desync on the first non-instruction
/// byte and then report nothing (`status/LESSONS.md`). Every candidate is confirmed by recomputing
/// the target from the displacement, so a coincidental `0xE8` byte cannot enter the set.
pub fn direct_call_sites(image: &Image, target: u64, lo: u64, hi: u64) -> Result<HashSet<u64>> {
    let span = image
        .bytes(lo, (hi - lo) as usize)
        .map_err(|e| OracleError::Eboot(e.to_string()))?;
    let mut out = HashSet::new();
    for i in 0..span.len().saturating_sub(5) {
        if span[i] != 0xE8 {
            continue;
        }
        let rel = i32::from_le_bytes(span[i + 1..i + 5].try_into().unwrap()) as i64;
        let va = lo + i as u64;
        if (va as i64 + 5 + rel) as u64 == target {
            out.insert(va + 5);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The arena primitive, on its own, with both directions of the guard.
    ///
    /// ⚠ The axis this tests is **"can the bump pointer be moved back, and does it refuse a mark
    /// it should not accept"** - not whether any codec is correct. It builds no image, so it runs
    /// in microseconds and cannot be confused with the leak control, which is the one that
    /// exercises real encodes.
    #[test]
    fn the_arena_rewinds_keeps_its_high_water_mark_and_refuses_a_bad_mark() {
        let mut s = State {
            heap_next: HEAP,
            heap_high_water: HEAP,
            guest_allocs: 0,
            steps: 0,
            step_limit: STEP_LIMIT,
            fault: None,
            stubs: HashMap::new(),
            memset_allow: HashSet::new(),
            ymm: HashMap::new(),
            notes: Vec::new(),
        };
        let scaffolding = s.alloc(0x1000, 16).unwrap();
        assert_eq!(scaffolding, HEAP);
        let mark = s.heap_next;

        // Two "calls" that each take a big scratch buffer.
        for _ in 0..2 {
            s.rewind(mark).unwrap();
            let a = s.alloc(0x1DF00, 16).unwrap();
            let b = s.alloc(0x100, 16).unwrap();
            assert_eq!(a, mark, "every call must be handed the SAME buffer back");
            assert!(b > a);
        }
        // ⭐ The number a leak control watches: monotonic, and unmoved by the second call.
        let hw = s.heap_high_water;
        s.rewind(mark).unwrap();
        for _ in 0..50 {
            s.rewind(mark).unwrap();
            s.alloc(0x1DF00, 16).unwrap();
            s.alloc(0x100, 16).unwrap();
        }
        assert_eq!(s.heap_high_water, hw, "50 more calls must not move the high-water mark");

        // NEGATIVE, both directions: a mark outside the live arena is an error, not a pointer
        // handed out twice.
        assert!(s.rewind(HEAP - 1).is_err(), "below the arena");
        assert!(s.rewind(s.heap_next + 1).is_err(), "above the bump pointer");
        // …and the positive control on the same call.
        assert!(s.rewind(mark).is_ok());
    }

    /// ⭐ Without a rewind the same loop runs out, and the error says what to do about it. This is
    /// the pre-2026-08-09 behaviour, asserted so nobody re-derives it from a live run.
    #[test]
    fn a_bump_arena_without_a_rewind_runs_out_and_the_error_names_the_fix() {
        let mut s = State {
            heap_next: HEAP,
            heap_high_water: HEAP,
            guest_allocs: 0,
            steps: 0,
            step_limit: STEP_LIMIT,
            fault: None,
            stubs: HashMap::new(),
            memset_allow: HashSet::new(),
            ymm: HashMap::new(),
            notes: Vec::new(),
        };
        let mut n = 0;
        let err = loop {
            match s.alloc(0x1DF00, 16) {
                Ok(_) => n += 1,
                Err(e) => break e.to_string(),
            }
        };
        assert!(n > 100 && n < 200, "{n} encodes' worth of 0x1DF00 buffers fit in 16 MiB");
        assert!(err.contains("scratch heap exhausted"), "{err}");
        assert!(err.contains("heap_rewind"), "the error must name the fix: {err}");
    }
}
