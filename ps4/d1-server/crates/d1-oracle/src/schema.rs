// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! What both codecs need before they will run: a fake Investment Globals manager, a fake message
//! definition record, and the fail-closed package registry.
//!
//! Port of `rsat-schema-encode-oracle.py`'s `SchemaOracle.__init__`. The object oracle inherits the
//! same scaffolding in the Python and does the same here - the object codec does not read the
//! definition record, but sharing one construction path means the two oracles cannot drift in how
//! they build the registry, which is the part that decides correctness.

use d1_eboot::descriptors::{self, Index};
use d1_eboot::image::Image;

use crate::error::{OracleError, Result};
use crate::harness::{Harness, Stub};
use crate::registry;

/// `0x25C9A0` - the Investment Globals manager singleton.
pub const GET_MANAGER: u64 = 0x25_C9A0;
/// An address nothing occupies, used as the vtable slot `+0x4D0` target.
pub const STUB_ACCESSOR: u64 = 0x2FF_0000;
/// `0x27FDD0` - message id → definition index. Its table is populated at runtime from content; in
/// a bare harness it returns -1 and the decoder bails before touching the schema. Any non-(-1)
/// index works, because the accessor is stubbed to hand back our record regardless.
pub const ID_TO_INDEX: u64 = 0x27_FDD0;

pub struct SchemaHarness {
    pub image: Image,
    /// This image's codec addresses ([`crate::codec`]). Every codec call goes through these.
    pub c: &'static crate::codec::CodecAddrs,
    pub index: Index,
    pub h: Harness,
    pub tag: u32,
    pub struct_size: usize,
    /// The fake definition record: `+0x10` u16 message id, `+0x14` request tag, `+0x18` response
    /// tag. Only those three words are read.
    pub definition: u64,
    pub manager: u64,
    /// ⭐ The bump-arena mark taken at the END of construction.
    ///
    /// Below it is the **permanent scaffolding** - the definition record, the fake vtable and
    /// manager, and the whole fail-closed package registry - which every call reads and no call may
    /// release. Above it is **one codec call's scratch**, and [`SchemaHarness::begin_call`] gives
    /// that back.
    ///
    /// ⚠ This is what makes a long-lived oracle possible at all: the arena is a bump pointer whose
    /// `free` is a no-op, so without a rewind the 133rd `global_activity_state` encode dies with
    /// `scratch heap exhausted` - measured, run `20260809-120451-MOONBASE-FRESH`.
    transient_mark: u64,
    /// ⛔ Always `true` outside [`SchemaHarness::set_rewind_enabled`]'s one caller. See there.
    rewind_enabled: bool,
}

impl SchemaHarness {
    pub fn new(tag: u32, msgid: u16, request_tag: u32) -> Result<Self> {
        Self::new_with_types(tag, msgid, request_tag, &[])
    }

    /// [`Self::new`], with `value_types` also made resolvable in the registry: the types a
    /// kind-`0x22` typed value names by its handle (e.g. the Breach command `0x8080279A`). The
    /// schema never reaches them statically, so without this the handle resolves to NULL and the
    /// codec reads garbage (a `memset` of length `-0xA4`).
    pub fn new_with_types(
        tag: u32,
        msgid: u16,
        request_tag: u32,
        value_types: &[u32],
    ) -> Result<Self> {
        let image = Image::open_default().map_err(|e| OracleError::Eboot(e.to_string()))?;
        let index = descriptors::index(&image).map_err(|e| OracleError::Eboot(e.to_string()))?;
        let d = index.get(tag).ok_or(OracleError::UnknownTag(tag))?;
        let struct_size = d.struct_size as usize;
        let c = crate::codec::codec_for(&image.sha256()).ok_or_else(|| {
            OracleError::Eboot(format!(
                "no codec address set for image sha256 {} - the codecs would be called at the \
                 wrong addresses",
                image.sha256()
            ))
        })?;
        let mut h = Harness::new(&image)?;

        let definition = h.alloc(0x80)?;
        h.write_u16(definition + 0x10, msgid)?;
        h.write_u32(definition + 0x14, request_tag)?;
        h.write_u32(definition + 0x18, tag)?;

        let vt = h.alloc(0x600)?;
        h.write_u64(vt + 0x4D0, STUB_ACCESSOR)?;
        let manager = h.alloc(0x40)?;
        h.write_u64(manager, vt)?;

        h.stub(c.get_manager, Stub::Return(manager));
        h.stub(STUB_ACCESSOR, Stub::Return(definition));
        h.stub(c.id_to_index, Stub::Return(0));

        // Box fields encode their in-band tag through start-up type cells: seed them and make
        // their types resolvable (see `CodecAddrs::type_cells`).
        let mut extra: Vec<u32> = c.type_cells.iter().map(|&(_, _, t, _)| t).collect();
        extra.extend_from_slice(value_types);
        for &(slot, cell, t, desc) in c.type_cells {
            h.write_u64(slot, cell)?;
            h.write_u32(cell, t)?;
            h.write_u64(cell + 8, desc)?;
        }
        for &(table, guard, slots) in c.type_cell_caches {
            for (i, &slot) in slots.iter().enumerate() {
                let cell = h.read_u64(slot)?;
                let tag = h.read_u32(cell)?;
                h.write_u32(table + i as u64 * 4, tag)?;
            }
            h.write(guard, &[1])?;
        }
        registry::build(&mut h, &image, &index, tag, c.registry_global, &extra)?;

        // ⭐ Everything allocated from here on is one call's scratch. Nothing after this line may
        // allocate scaffolding that outlives a call - `begin_call` releases it.
        let transient_mark = h.heap_mark();

        Ok(Self {
            image,
            c,
            index,
            h,
            tag,
            struct_size,
            definition,
            manager,
            transient_mark,
            rewind_enabled: true,
        })
    }

    /// ⭐⭐ **Start one codec call: give back the previous call's scratch.**
    ///
    /// The arena is a bump pointer and [`crate::harness::Stub::Free`] is a no-op, so without this
    /// every call leaks its whole working set and a long-lived oracle dies on wall-clock time. The
    /// boundary is the *call*, not the process, because a codec call's allocations - the object
    /// image, the wire buffer, the bitstream, the flush out-param - are all read back into host
    /// `Vec`s before it returns. Nothing guest-side survives it.
    ///
    /// ⚠ That last sentence is a claim, so it is **measured**: `guest_allocs()` counts allocations
    /// made by guest code through the `alloc_aligned` import, and the leak control asserts it stays
    /// at zero across the whole message set. If a future descriptor makes the client's codec
    /// allocate, that assertion goes red *before* a rewind hands the same bytes out twice.
    ///
    /// ⚠ Not usable by [`crate::MessageOracle`] as written: its `make_handle` returns a guest
    /// pointer the caller passes to a *later* `encode`, so its transaction spans two methods. That
    /// oracle is one-shot tooling and is bounded by process lifetime; a long-lived caller must
    /// bracket the pair itself with [`crate::harness::Harness::heap_mark`] /
    /// [`crate::harness::Harness::heap_rewind`].
    pub fn begin_call(&mut self) -> Result<()> {
        if !self.rewind_enabled {
            return Ok(());
        }
        self.h.heap_rewind(self.transient_mark)
    }

    /// ⛔⛔ **THE LEAK CONTROL'S INVERSION KNOB. Never call this in a server.**
    ///
    /// Setting `false` restores the pre-2026-08-09 behaviour exactly: [`SchemaHarness::begin_call`]
    /// becomes a no-op and every codec call leaks its whole working set into a bump arena whose
    /// `free` does nothing.
    ///
    /// It exists because *a control that only ever passes is decoration*. The leak control
    /// ([`crate::activity_host::leak_control`]) runs **both arms every time it runs**: with the
    /// rewind on the arena high-water mark must be flat over thousands of encodes, and with it off
    /// the same loop must run the arena out and reproduce the observed failure. Without the second
    /// arm, "the high-water mark was flat" would be a claim about a loop nobody proved could grow.
    pub fn set_rewind_enabled(&mut self, on: bool) {
        self.rewind_enabled = on;
    }

    /// The mark [`SchemaHarness::begin_call`] rewinds to - the size of the permanent scaffolding.
    pub fn transient_mark(&self) -> u64 {
        self.transient_mark
    }

    /// Allocate a zeroed C object of the schema's declared size and seed it with `bytes`.
    pub fn make_object(&mut self, bytes: &[u8]) -> Result<u64> {
        let size = self.struct_size.max(16) as u64;
        let obj = self.h.alloc(size)?;
        if !bytes.is_empty() {
            let n = bytes.len().min(self.struct_size);
            self.h.write(obj, &bytes[..n])?;
        }
        Ok(obj)
    }
}
