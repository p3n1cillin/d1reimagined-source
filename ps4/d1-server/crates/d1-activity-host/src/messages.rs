// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! The Activity-Host message **structs** - the client's in-memory images, not the wire.
//!
//! ⚠⚠ Read [`crate::codec`] first. Everything here produces a *struct image*, which the schema
//! codec then serialises. `struct_bytes()` is named that way on purpose; there is deliberately no
//! `to_wire_bytes` anywhere in this module.
//!
//! Field offsets are `[C]` from the client's own apply sites - the decompile of `0x29BD10` reads
//! `join_result` field by field with plain `mov`/`movzx` off the decoded struct, so the *struct*
//! is little-endian native, which is what we reproduce.

use crate::entity::EntitySlotMask;
use crate::protocol::{
    self, GLOBAL_ACTIVITY_STATE_BLOB_SIZE, JOIN_REQUEST_STRUCT_SIZE, JOIN_RESULT_STRUCT_SIZE,
    START_ACTIVITY_HOST_RESPONSE_SIZE,
};
use crate::AhError;

// ── AH → client ───────────────────────────────────────────────────────────────────────────────

/// `[C]` udt **4** `join_result`, `0x1A8` bytes. The richest message in the protocol, and the one
/// that hands the client its lease.
///
/// Struct offsets, all read out of `FUN_0029BD10` operating on the decoded struct:
///
/// ```text
/// +0x000  [u8;4] nonce        `if (*param_2 == (int)param_1[0x173])` - compared against ac+0xB98
/// +0x008  u64    ah_id        `param_1[0x193] = *(long *)(param_2 + 2)`
/// +0x100  i8     result       `(char)param_2[0x40]`, 0 = accepted; anything else logs `[reason ENUM(%d)]`
/// +0x101  char   session_id[] `(long)param_2 + 0x101`, NUL-terminated, printed with %s
/// +0x18C  u32                 `[U]` -> activity_host_changed
/// +0x190  u32                 `[U]` -> the deterministic simulation queue
/// +0x194  u8     epoch        `(char)param_2[0x65]` - the replication epoch
/// +0x196  u16    grace_ms     `*(ushort *)((long)param_2 + 0x196)` -> ac+0x6C70
/// +0x198  u16    keepalive_ms `*(ushort *)(param_2 + 0x66)`        -> ac+0x6C74
/// ```
///
/// ⭐ **`keepalive_ms` is the Activity Host's to choose and the client has no default** - the
/// constructor initialises `ac+0x6C74` to 0. Whatever we put at `+0x198` becomes the cadence at
/// which the client says something. The *eviction* deadline stays server-side; the client never
/// learns it.
///
/// ⛔ **The `result` value space is `[U]` and that is a real absence, not a failed search.** The
/// client formats it through the generic `ENUM(%d)` formatter at `0xE9FA0`; there is no
/// denial-reason string table in this build. Only `0` is known, and it means accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinResult {
    /// ⭐ **The whole `join_request` struct, echoed.** `join_result`'s descriptor field `[0]` is
    /// tag `0x80800E98` - the request - embedded at `+0x00`, which is *why* the nonce is at `+0x00`
    /// and the AH id at `+0x08`. They are the request's own first two fields.
    ///
    /// Empty means "send zeroes there", which encodes and decodes cleanly; echoing the client's
    /// own bytes is strictly more faithful and free, because the listener already has them.
    /// `[U]` whether the client reads anything from it beyond the two fields `0x29BD10` touches.
    pub request_image: Vec<u8>,
    /// Echoed verbatim from the join request. **Bytes, not a number** - the client compares it as
    /// a native `int` against a value it set itself, so echoing the bytes is correct regardless of
    /// how either side would have interpreted them. Same discipline as message 17's id echo.
    pub nonce: [u8; 4],
    /// The activity-host id, matching the one message 7 handed out.
    pub ah_id: u64,
    /// `0` = accepted.
    pub result: i8,
    /// NUL-terminated ASCII. ⚠ The field is **128** bytes (`0x101..0x181`) - see
    /// [`JOIN_RESULT_SESSION_ID_CAPACITY`], corrected 2026-08-08.
    pub session_id: Vec<u8>,
    /// `[U]` - `activity_host_changed`.
    pub field_18c: u32,
    /// `[U]` - feeds the deterministic simulation queue.
    pub field_190: u32,
    /// The replication epoch. A `u8`, monotonic, advanced later by udt 42.
    pub replication_epoch: u8,
    /// Milliseconds of grace. `[U]` semantics beyond "the client stores it at `ac+0x6C70`".
    pub grace_ms: u16,
    /// ⭐ Milliseconds between client keepalives. Ours to choose; the client has no default.
    pub keepalive_interval_ms: u16,
}

/// The `session_id` field's capacity, NUL included.
///
/// ⚠⚠ **CORRECTED 2026-08-08: 128, not 139.** This was `0x18C - 0x101 = 139`, inferred from the
/// next *known* reader's offset. The descriptor says the array is `0x80804056` - **128 bytes** - so
/// `+0x181..+0x184` is padding and `+0x184`/`+0x188` are two separate `u32` fields that a
/// 129-to-138-byte session id would have quietly overwritten. Derived, not transcribed:
/// [`d1_oracle::activity_host::join_result::SESSION_ID_CAPACITY`].
pub const JOIN_RESULT_SESSION_ID_CAPACITY: usize =
    d1_oracle::activity_host::join_result::SESSION_ID_CAPACITY;

impl JoinResult {
    /// The minimum viable acceptance.
    ///
    /// `keepalive_interval_ms` of 0 would mean "say something every 0 ms" - the keepalive tick at
    /// `0x298C40` fires whenever `now - last_send >= interval` - so callers that do not care
    /// should use [`Self::DEFAULT_KEEPALIVE_MS`].
    pub fn accepted(nonce: [u8; 4], ah_id: u64, session_id: &str, epoch: u8) -> Self {
        Self {
            request_image: Vec::new(),
            nonce,
            ah_id,
            result: 0,
            session_id: session_id.as_bytes().to_vec(),
            field_18c: 0,
            field_190: 0,
            replication_epoch: epoch,
            grace_ms: Self::DEFAULT_GRACE_MS,
            keepalive_interval_ms: Self::DEFAULT_KEEPALIVE_MS,
        }
    }

    /// `[U]`, a choice not a finding. 2 s of keepalive cadence is slow enough not to spam a run log
    /// and fast enough that a stall is visible within one screenshot.
    pub const DEFAULT_KEEPALIVE_MS: u16 = 2_000;
    /// `[U]`, a choice not a finding.
    pub const DEFAULT_GRACE_MS: u16 = 10_000;

    /// Accept a join, echoing the client's own request struct into field `[0]`.
    pub fn accepted_echoing(
        request_image: &[u8],
        ah_id: u64,
        session_id: &str,
        epoch: u8,
    ) -> Self {
        let nonce = request_image
            .get(..4)
            .and_then(|s| s.try_into().ok())
            .unwrap_or([0u8; 4]);
        let mut j = Self::accepted(nonce, ah_id, session_id, epoch);
        j.request_image = request_image.to_vec();
        j
    }

    /// The `0x1A8` struct image.
    pub fn struct_bytes(&self) -> Result<Vec<u8>, AhError> {
        if self.session_id.len() >= JOIN_RESULT_SESSION_ID_CAPACITY {
            return Err(AhError::FieldOverflow {
                field: "join_result.session_id",
                len: self.session_id.len(),
                max: JOIN_RESULT_SESSION_ID_CAPACITY - 1,
            });
        }
        let mut b = vec![0u8; JOIN_RESULT_STRUCT_SIZE];
        // Field [0] first, so the explicit nonce/ah_id below always win.
        if !self.request_image.is_empty() {
            let n = self.request_image.len().min(JOIN_REQUEST_STRUCT_SIZE);
            b[..n].copy_from_slice(&self.request_image[..n]);
        }
        b[0x000..0x004].copy_from_slice(&self.nonce);
        b[0x008..0x010].copy_from_slice(&self.ah_id.to_le_bytes());
        b[0x100] = self.result as u8;
        b[0x101..0x101 + self.session_id.len()].copy_from_slice(&self.session_id);
        b[0x18C..0x190].copy_from_slice(&self.field_18c.to_le_bytes());
        b[0x190..0x194].copy_from_slice(&self.field_190.to_le_bytes());
        b[0x194] = self.replication_epoch;
        b[0x196..0x198].copy_from_slice(&self.grace_ms.to_le_bytes());
        b[0x198..0x19A].copy_from_slice(&self.keepalive_interval_ms.to_le_bytes());
        Ok(b)
    }
}

/// `[C]` udt **10** `start_activity_host_response`, `0x88` bytes = `{u64 ah_id, char session_id[0x80]}`.
///
/// ⭐ **This never travels on the AH connection.** It is injected client-side by the BAP message-7
/// handler (`0x594C60`), which copies `{u64 at +1, 128 bytes at +9}` out of the manager response
/// into one contiguous `0x88` blob and delivers it as `user_defined_type 10`. So *this struct is
/// literally BAP message 7's payload*, and [`crate::registry`] builds it as such.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartActivityHostResponse {
    pub ah_id: u64,
    /// NUL-terminated ASCII, 128 bytes.
    pub session_id: Vec<u8>,
}

impl StartActivityHostResponse {
    pub fn struct_bytes(&self) -> Result<Vec<u8>, AhError> {
        let cap = d1_bap::activity_host::AH_HOST_TEXT_LEN;
        if self.session_id.len() >= cap {
            return Err(AhError::FieldOverflow {
                field: "start_activity_host_response.session_id",
                len: self.session_id.len(),
                max: cap - 1,
            });
        }
        let mut b = vec![0u8; START_ACTIVITY_HOST_RESPONSE_SIZE];
        b[0..8].copy_from_slice(&self.ah_id.to_le_bytes());
        b[8..8 + self.session_id.len()].copy_from_slice(&self.session_id);
        Ok(b)
    }
}

/// `[C]` udt **1** `global_activity_state` - ⭐ the authoritative Activity State.
///
/// ⚠⚠ **CORRECTED 2026-08-08.** This used to be modelled as *"8 bytes of prefix and then exactly
/// 1,712 bytes of blob"*, from `0xBAFB50`'s `memcpy(world+0x50B0, payload+8, 0x6B0)`. The
/// **descriptor** for this message (tag `0x80800E76`, derived in
/// [`d1_oracle::activity_host`]) says the struct is **`0x6B0` = 1,712 bytes in total**, one
/// composite field. Both readings are true of different objects: `0xBAFB50` is unpacking a
/// *simulation-queue element*, which wraps the decoded struct behind an 8-byte key. The AH wire
/// struct is 1,712 bytes and the schema codec refuses anything else, so the old model would have
/// produced a hard, named error at the first push rather than a silent 8-byte shift.
///
/// ⛔ **The contents are `[U]` and this is the ceiling in one field.** 1,712 bytes is not a world;
/// it is the Mission Script's global state. A stub sends zeros, which is a *valid* activity state
/// describing an activity in which nothing has happened and nothing is scheduled to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalActivityState {
    /// The whole 1,712-byte struct.
    pub blob: Vec<u8>,
}

impl Default for GlobalActivityState {
    fn default() -> Self {
        Self::empty()
    }
}

impl GlobalActivityState {
    /// ⚠ `[R]` from another session's rung-31..36 trace, **not verified at a consumer in this
    /// crate**: the client requires blob byte `+0x00` to be **non-zero**. It is field `[0]` of
    /// `0x80800B8F`, `kind 0x2` (a bool), and two more bools follow at `+0x01`/`+0x02` whose
    /// semantics are `[U]`. [`Self::empty`] leaves it zero on purpose so the two cases can be
    /// bisected in one run.
    pub const FLAG_0: usize = d1_oracle::activity_host::global_activity_state::FLAG_0;
    /// ⭐⭐ The **activity INDEX** - a `u16` at blob `+0x680`, the field **rung 33 gates on**.
    ///
    /// `FUN_0078C770` reads it (`*(u16*)(world + 0x50B0 + 0x680)`) and rung 33's `FUN_007A7420`
    /// passes it to `FUN_0054F410` and to `activity_table[idx]`. Leaving it 0 makes the client
    /// evaluate `activity_table[0]` = `HASH(0x8AE8DA69)` = **"Tower"**, whose prerequisites are
    /// unmet on a fresh account - which is `activity_failed_to_load_missing_dlc`, in full.
    pub const ACTIVITY_INDEX: usize =
        d1_oracle::activity_host::global_activity_state::ACTIVITY_INDEX;
    /// ⭐⭐ The activity CHECKSUM, a `u32` at blob `+0x5C`. Read by `FUN_00299100` as
    /// `*(int*)(FUN_002A1F50(world) + 0x5C)`, where `FUN_002A1F50` is the one-line leaf
    /// `return p + 0x50B0` - i.e. this blob. See [`Self::set_activity_checksum`].
    pub const CHECKSUM: usize = 0x5C;
    /// ⭐ The START REGION, a `u32` at blob `+0x54` (10 bits used), and its name hash at `+0x58`.
    /// 56578 `0x77FBF0` reads both from this blob and hands them to the slice-set transition
    /// `0x77FD50` (kind 2), which stores the region as PENDING (`sstm+0x138`). The region id is the
    /// SLICE-SET id, `8 * bubble + n` for bubble `b`'s n-th slice set in the scenario's per-bubble
    /// list (measured on A Guardian Rises: 113 = `cosmo_m0_phase00_highway_intro`, bubble 14's
    /// second set, prints `PRV113.1` and plays the opening; 112 is the highway gameplay set).
    ///
    /// Left 0, every world starts in bubble 0. That is right for `act_cosmo_m0_highway`, but on
    /// `act_cosmo_m1_dome_a` (The Dark Within) bubble 0 is a PUBLIC patrol bubble and rung 34 stops
    /// on *"Region 'PUB00.00' is PUBLIC but not yet connected"*. The mission's own first phase,
    /// `cosmo_m1_phase00_dome_a`, is in bubble 8 = region 8 (run `2026-10-14-dw-domea4`).
    pub const START_REGION: usize = 0x54;
    /// The start region's name hash; `0x811C9DC5` (FNV-1 of "") is what the client itself passes
    /// when it has no name.
    pub const START_REGION_HASH: usize = 0x58;
    /// The world CLOCK base, a `u64` of milliseconds at blob `+0x520`. 56578 `0x2A19D0` returns
    /// `base + sim_ticks * 1000 / ticks_per_second`; `0x6E89E0(period)` turns that into a cycle
    /// phase `fmod(clock / period, 1)` for content. Left 0, every served world's clock starts at 0.
    pub const TIME_BASE_MS: usize = 0x520;
    /// ⭐⭐ The **bubble count**, a `u32` at blob `+0x10` (field `[4]` of `0x80800B8F`, `kind 0x5`
    /// = a 32-bit leaf). `FUN_002A0320` - the check `0x75A9E0` runs *immediately after*
    /// `FUN_00299100` passes - requires `*(int*)(blob+0x10) == *(long*)(scenario_record+0x48)`.
    /// On a mismatch it calls `(*cb)(6, 0, …)`: `"invalid host activity state"` when ours is **0**
    /// (every run to date), `"activity client bubble count mismatch"` otherwise. The scenario's
    /// count is a property of the WORLD - **16 for `orbit`, measured live** (`activity-checksum.md`
    /// §9). See [`Self::set_bubble_count`].
    pub const BUBBLE_COUNT: usize = 0x10;
    /// ⭐⭐ The per-bubble **chosen-state array** at blob `+0x14` - field `[5]`, a nested struct
    /// `0x80804050` of **64 bytes**. `FUN_002A0320` reads the signed byte `blob+0x14+i` for
    /// `i in [0, bubble_count)`:
    /// * **`-1` (`0xFF`)** = "bubble not chosen" - benign. `FUN_00343020(ctx, id, 0xFFFFFFFF, 0)`
    ///   returns 1, and the inner sub-bubble loop runs only when the scenario bubble's valid-flag
    ///   is `> 1`; every `orbit` bubble has valid-flag `<= 1` (measured), so it never runs.
    /// * **`0`** = "state 0 chosen" - then the scenario's own state-0 flag must be set, which is
    ///   FALSE for several `orbit` bubbles ⇒ `"activity client bubble state index was valid, but
    ///   state itself was invalid"`.
    /// * **`>= 1`** = `"activity client bubble state > 1 chosen"` / `"…mismatch"`.
    ///
    /// ⛔⛔ **RETRACTED, 2026-08-09: "all-`0xFF` is the world-agnostic pass" IS TRUE FOR RUNG 33
    /// AND IS EXACTLY WHAT BREAKS RUNG 34.** The `-1` branch above is not merely "benign" - its
    /// `FUN_00343020(ctx, id, 0xFFFFFFFF, 0)` call **UNREGISTERS the region**: it writes `-1` into
    /// `slice_set_registry + 0x10 + id*0x10` and clears the slot. Rung 34 then asks
    /// `FUN_00344a50(ctx, region_id)` for that same slot, reads `-1`, returns state **5**, and the
    /// slice-set transition manager prints
    /// `Region 'PRV00.00 (HASH: …)' ('PENDING') cannot be found!` ⇒ `error.sstm_slice_not_found`.
    /// The `0` branch is the one that **REGISTERS** the region (`param_3` = the scenario tag).
    ///
    /// ⇒ **A bubble must be marked `0x00` (chosen) for its region to exist**, and `0x00` is legal
    /// exactly when that scenario bubble has a sub-bubble whose flag byte is non-zero - equivalently
    /// (measured 16/16 live on `orbit`) when its **sub-bubble count is non-zero**. `orbit`'s
    /// bubbles 5-8 and 15 have count 0, which is why blanket `0x00` failed there. Use
    /// [`Self::set_bubble_chosen`] / `D1_AH_BUBBLE_CHOSEN`. ⇒ `historicaldocs/design/slice-set-residency.md`.
    ///
    /// ⚠ Whether the codec actually carries these 64 bytes is asserted by
    /// [`crate::controls`]' bubble round-trip control, not assumed - the descriptor only *names* a
    /// single 8-bit field inside the struct.
    pub const BUBBLE_STATE: usize = 0x14;
    /// `0x80804050` is 64 bytes; that bounds how many bubbles can be marked unchosen in one blob.
    pub const BUBBLE_STATE_CAPACITY: usize = 64;
    /// ⛔⛔ **THE ACTIVITY-SLOT ARRAY'S ELEMENT COUNT, at blob `+0x64` - and the reason this crate
    /// will NOT publish a launch slot.** Six bits (`0x8080533D` field `[0] @+0x000`, `kind 0x5`,
    /// **6 bits** ⇒ 0..63 against a capacity of 50).
    ///
    /// ## What was asked, and what was measured
    ///
    /// The standing candidate was: the 50 × 24-byte activity slots at [`Self::LAUNCH_SLOTS`] are
    /// published all-zero, unset slots read `{0x811C9DC5, 0xFFFFFFFF}` where `0x811C9DC5` is
    /// `fnv1("")`, so populating slot 0 might make the launch real. **Three findings, in the order
    /// they close it:**
    ///
    /// 1. ⛔ **It cannot be encoded, by the client's own encoder.** With this count at 0 the blob
    ///    encodes fine and the slot bytes **do not reach the wire at all** (byte-identical output
    ///    for slot-0 hashes `0x00000000`, `0x811C9DC5` and `0xDEADBEEF` - measured). With the count
    ///    at **any** non-zero value the encoder walks into the element, reaches its second field
    ///    (`0x80800D17` field `[1] @+0x004`, **`kind 0x29`, a DYNAMIC-WIDTH leaf**), and derefs the
    ///    **content registry** `*DAT_01A0B420` at `0x105B74A` to size it - a runtime structure a
    ///    statically mapped image does not have. `d1-oracle activity-host encode --udt 1` returns
    ///    `unicorn: READ_UNMAPPED calling 0x2c8ff0, RIP 0x105b74a` for counts 1, 63 and every hash
    ///    tried. This is the same class as [`crate::codec::warm_undecodable_set`]'s `kind 0x22`.
    /// 2. ⛔⛔ **And it would fault the LIVE Activity Host, not just an offline control.** This
    ///    crate encodes through that same oracle at push time, so a non-zero count would turn every
    ///    `global_activity_state` push into an `AhError` - the failure that already killed a run
    ///    once as `error.private_activity_host_host_crashed`.
    /// 3. ⛔ **The premise was a conflation anyway.** The KTOBUCKET line
    ///    `ACTIVITY ACTNAME:[%s] SLICESET HASH:[0x%08x]` is emitted by `FUN_00782720`, which renders
    ///    ACTNAME from `s_investment_utility_context::vfunc[3](FUN_0033FF30())` - the **activity
    ///    loader's** current name, with `NONE` a literal string at `0x16076E5` - and SLICESET from
    ///    the SSTM's resolved hash. **Neither reads this array.** The two objects merely share the
    ///    FNV-1 basis `0x811C9DC5` as a sentinel. ⇒ populating a launch slot **cannot** make ACTNAME
    ///    real. `historicaldocs/design/ACTIVITY-LAUNCH-SOLVED.md` §2 and `historicaldocs/status/aug10-startup-context.md` already
    ///    refuted the slot array as the selector, on the change-world command buffer.
    ///
    /// ⇒ [`Self::struct_bytes`] **refuses** a blob with a non-zero count rather than letting the
    /// Unicorn fault surface at push time, and there is deliberately **no env knob**. Reopening
    /// this needs a live instrument, not a bolder encoder.
    pub const LAUNCH_SLOT_COUNT: usize = 0x64;
    /// The 50 × 24-byte activity-slot array at blob `+0x68` (`0x8080522B`, 1200 bytes; element
    /// `0x80800D17`, 24 bytes). Derived from the descriptor tree, and cross-checked twice by the
    /// two constants either side of it: the same nested block's `[7] @+0x620` is
    /// [`Self::ACTIVITY_INDEX`] (`0x680`) and `[10] @+0x628` is [`Self::NAME`] (`0x688`).
    /// ⛔ Read [`Self::LAUNCH_SLOT_COUNT`] before touching it.
    pub const LAUNCH_SLOTS: usize = 0x68;
    /// 24 bytes. ⚠ `d1-eboot descriptors` prints an array's ELEMENT SIZE in the offset column:
    /// `0x8080522B`'s single field reads `@+0x018`, which is the stride, not a displacement.
    pub const LAUNCH_SLOT_STRIDE: usize = 0x18;
    /// 50. `1200 / 24`.
    pub const LAUNCH_SLOT_CAPACITY: usize = 50;

    /// ⭐ The activity name - `char[0x28]` at blob `+0x688`, **the struct's last field**, and
    /// `0x688 + 0x28 == 0x6B0` exactly. Derived from the descriptor tree, never transcribed.
    pub const NAME: usize = d1_oracle::activity_host::global_activity_state::NAME;
    /// `0x28`, NUL included.
    pub const NAME_CAPACITY: usize = d1_oracle::activity_host::global_activity_state::NAME_CAPACITY;

    /// All zeroes. ⚠ **This is the bisection control, not the thing to send.** `FLAG_0` is zero
    /// and there is no activity name, and `[R]` says the client needs both. Use
    /// [`Self::for_activity`] for a run you expect to work.
    pub fn empty() -> Self {
        Self { blob: vec![0u8; GLOBAL_ACTIVITY_STATE_BLOB_SIZE] }
    }

    /// ⭐ The state a stub Activity Host should actually publish: `FLAG_0` set, and the activity
    /// name written at `+0x688`.
    ///
    /// `FUN_01057B80` sets `ac+0x130 |= 0x40` **only on a successful decode of this message** (the
    /// `or` is inside its body, on the `decode != 0` branch - read in the decompile, not reported),
    /// and rung 31 reads that bit. Everything else in the 1,712 bytes stays zero, which is a valid
    /// description of an activity in which nothing has been scheduled - the ceiling, unchanged.
    pub fn for_activity(name: &str) -> Result<Self, AhError> {
        let mut s = Self::empty();
        s.blob[Self::FLAG_0] = 1;
        s.set_name(name)?;
        Ok(s)
    }

    /// Write the activity name at `+0x688`. Refused rather than truncated: a truncated activity
    /// name is a *different* activity name, and the client would look it up and find nothing.
    pub fn set_name(&mut self, name: &str) -> Result<(), AhError> {
        let b = name.as_bytes();
        if b.len() >= Self::NAME_CAPACITY {
            return Err(AhError::FieldOverflow {
                field: "global_activity_state.activity_name",
                len: b.len(),
                max: Self::NAME_CAPACITY - 1,
            });
        }
        self.blob[Self::NAME..Self::NAME + Self::NAME_CAPACITY].fill(0);
        self.blob[Self::NAME..Self::NAME + b.len()].copy_from_slice(b);
        Ok(())
    }

    /// ⭐⭐ Write the activity index at `+0x680`, **little-endian** - this is a decoded struct
    /// image on x86, not the wire; the schema encoder byte-orders it.
    ///
    /// ⚠ It is *not* derived from the activity name. `rungs-31-to-36-preflight.md` established
    /// that the two are adjacent fields of this same blob and that **the name is nowhere derived
    /// from the index**; the descriptor agrees (`[7] @+0x620` u16 and `[10] @+0x628` char[40] are
    /// two independent fields). So this crate refuses to invent one from the other.
    pub fn set_activity_index(&mut self, index: u16) {
        self.blob[Self::ACTIVITY_INDEX..Self::ACTIVITY_INDEX + 2]
            .copy_from_slice(&index.to_le_bytes());
    }

    /// ⭐⭐ The **activity CHECKSUM** at `blob+0x5C` - the field that aborts rung 33's world
    /// transition when it disagrees with the client's own content.
    ///
    /// `FUN_00299100` (its **only** caller is `0x75A9E0` at `0x75AA56`) compares
    /// `*(int*)(FUN_002A1F50(world) + 0x5C)` - i.e. **this field of this blob** - against a value it
    /// resolves out of the content registry `*DAT_01A0B420`, bucketed by `param_3 >> 13` and indexed
    /// by `param_3 & 0x1FFF`, then read at `record + 0xC + ac[3]*4`. On disagreement it logs
    /// `"Activity checksum mismatch!"` and calls `(*param_2)(5, 0, "activity checksum mismatch")`,
    /// which surfaces as `world_controller:transition_error` and **aborts the transition** - after
    /// which the world change completes to the EMPTY STRING and rung 33 reports *"we don't have an
    /// active activity"*. Measured 3/3 in `runs/20260809-084932-RUNG33-PERMS6`.
    ///
    /// ⛔ **We cannot take the skip branch.** `FUN_00299100` returns success early when its
    /// `param_3 == 0xFFFFFFFF`, but `param_3` is `*(u32*)(manager + 0x278)` - **client-side state**,
    /// not a field of ours. The value must therefore MATCH, not be suppressed.
    ///
    /// ⚠ `rungs-31-to-36-preflight.md` names this field and calls a mismatch *"LOUD but **not** on
    /// the status-6 path"* - **true for rung 30 and wrong for rung 33**, where it is fatal.
    ///
    /// ⚠ The correct value is `[U]`. `D1_AH_ACTIVITY_CHECKSUM` exists so a candidate can be tried
    /// per-poke (the AH hot-swaps) without a rebuild. Default `0` reproduces every run to date.
    /// Set the world clock base (see [`Self::TIME_BASE_MS`]).
    pub fn set_time_base_ms(&mut self, ms: u64) {
        self.blob[Self::TIME_BASE_MS..Self::TIME_BASE_MS + 8].copy_from_slice(&ms.to_le_bytes());
    }

    /// Set the start region (see [`Self::START_REGION`]).
    pub fn set_start_region(&mut self, region: u32, hash: u32) {
        self.blob[Self::START_REGION..Self::START_REGION + 4].copy_from_slice(&region.to_le_bytes());
        self.blob[Self::START_REGION_HASH..Self::START_REGION_HASH + 4].copy_from_slice(&hash.to_le_bytes());
    }

    /// The start region currently in the blob.
    pub fn start_region(&self) -> u32 {
        u32::from_le_bytes(self.blob[Self::START_REGION..Self::START_REGION + 4].try_into().expect("4 bytes"))
    }

    pub fn set_activity_checksum(&mut self, checksum: u32) {
        self.blob[Self::CHECKSUM..Self::CHECKSUM + 4].copy_from_slice(&checksum.to_le_bytes());
    }

    /// The activity checksum currently in the blob, for the run log and for controls.
    pub fn activity_checksum(&self) -> u32 {
        u32::from_le_bytes(
            self.blob[Self::CHECKSUM..Self::CHECKSUM + 4].try_into().expect("4 bytes"),
        )
    }

    /// ⭐⭐ Set the **bubble count** at `blob+0x10` AND mark that many bubbles UNCHOSEN (`0xFF`) at
    /// `blob+0x14`. These are two fields but one fact: `FUN_002A0320` first checks the count equals
    /// the scenario's `+0x48`, then walks exactly `count` bubble-state bytes. Publishing the count
    /// without the `0xFF` bytes would enter the loop with every bubble reading `0` ("state 0
    /// chosen") and fail on the first bubble whose scenario state-0 is invalid. So they move
    /// together. `count` must equal the world's scenario bubble count (`orbit` = 16); a wrong count
    /// is the loud `"activity client bubble count mismatch"`, never a silent pass.
    pub fn set_bubble_count(&mut self, count: u32) -> Result<(), AhError> {
        let n = count as usize;
        if n > Self::BUBBLE_STATE_CAPACITY {
            return Err(AhError::FieldOverflow {
                field: "global_activity_state.bubble_count",
                len: n,
                max: Self::BUBBLE_STATE_CAPACITY,
            });
        }
        self.blob[Self::BUBBLE_COUNT..Self::BUBBLE_COUNT + 4].copy_from_slice(&count.to_le_bytes());
        // Clear the whole 64-byte field, then mark the first `count` bubbles unchosen.
        self.blob[Self::BUBBLE_STATE..Self::BUBBLE_STATE + Self::BUBBLE_STATE_CAPACITY].fill(0);
        self.blob[Self::BUBBLE_STATE..Self::BUBBLE_STATE + n].fill(0xFF);
        Ok(())
    }

    /// ⭐⭐ Mark the listed bubbles **CHOSEN** (`0x00`) at `blob+0x14`, leaving the rest `0xFF`.
    ///
    /// This is the rung-34 knob. `FUN_002A0320`'s per-bubble loop is the **only** registrar of
    /// slice-set regions on a non-`orbit` world (`FUN_007528B0`, the other one, is reached only
    /// through `world_controller:orbit_map_loader`). Its `-1`/"unchosen" branch calls
    /// `FUN_00343020(ctx, bubble*8, 0xFFFFFFFF, 0)`, which writes `-1` into
    /// `slice_set_registry + 0x10 + (bubble*8)*0x10`; the `0`/"chosen" branch calls it with the
    /// scenario tag and **registers** the region. Rung 34's `FUN_0077E440` then starts a
    /// `transitioning:initial_slice_set` on a region id read from content, and
    /// `FUN_007812D0`/`FUN_00344A50` fails it with state 5 unless that slot is registered.
    ///
    /// ⚠ **`0x00` is not universally legal.** `FUN_002A0320` rejects a chosen bubble whose
    /// scenario sub-bubble flag is clear, with `"activity client bubble state index was valid, but
    /// state itself was invalid"`. Measured live on `orbit` (16/16): the flag is set exactly for
    /// the bubbles whose sub-bubble count is non-zero - 0-4 and 9-14, the same 11 the live registry
    /// has registered. So choose only bubbles the world actually has.
    ///
    /// Must be called **after** [`Self::set_bubble_count`], which rewrites the whole 64-byte field.
    /// Indices at or beyond the published count are a config error, not a silent no-op: the client
    /// never reads them, so accepting them would let a wrong knob look like a working one.
    pub fn set_bubble_chosen(&mut self, chosen: &[usize]) -> Result<(), AhError> {
        let n = self.bubble_count() as usize;
        for &i in chosen {
            if i >= n {
                return Err(AhError::FieldOverflow {
                    field: "global_activity_state.bubble_chosen",
                    len: i + 1,
                    max: n,
                });
            }
            self.blob[Self::BUBBLE_STATE + i] = 0x00;
        }
        Ok(())
    }

    /// The bubble count currently in the blob, for the run log and for controls.
    pub fn bubble_count(&self) -> u32 {
        u32::from_le_bytes(
            self.blob[Self::BUBBLE_COUNT..Self::BUBBLE_COUNT + 4].try_into().expect("4 bytes"),
        )
    }

    /// The per-bubble state byte at `blob+0x14+i`, for controls.
    pub fn bubble_state(&self, i: usize) -> u8 {
        self.blob[Self::BUBBLE_STATE + i]
    }

    /// The activity index currently in the blob, for the run log and for controls.
    pub fn activity_index(&self) -> u16 {
        u16::from_le_bytes(
            self.blob[Self::ACTIVITY_INDEX..Self::ACTIVITY_INDEX + 2].try_into().expect("2 bytes"),
        )
    }

    /// The activity name currently in the blob, for the run log.
    pub fn name(&self) -> String {
        let field = &self.blob[Self::NAME..Self::NAME + Self::NAME_CAPACITY];
        let end = field.iter().position(|b| *b == 0).unwrap_or(field.len());
        String::from_utf8_lossy(&field[..end]).to_string()
    }

    /// The activity-slot element count currently in the blob. Always 0 in anything this crate
    /// builds; exposed so [`Self::struct_bytes`]' refusal and its control read the same field.
    pub fn launch_slot_count(&self) -> u32 {
        u32::from_le_bytes(
            self.blob[Self::LAUNCH_SLOT_COUNT..Self::LAUNCH_SLOT_COUNT + 4]
                .try_into()
                .unwrap_or([0; 4]),
        )
    }

    /// ⛔ **Only for the refusal control.** Populates activity slot `index` and sets the element
    /// count - which makes the blob **unencodable**, on purpose, so the control can prove it.
    /// See [`Self::LAUNCH_SLOT_COUNT`] for the three measurements that closed this candidate.
    /// There is no env knob and there must not be one.
    #[doc(hidden)]
    pub fn set_launch_slot_unencodable(&mut self, index: usize, hash: u32) -> Result<(), AhError> {
        if index >= Self::LAUNCH_SLOT_CAPACITY {
            return Err(AhError::FieldOverflow {
                field: "global_activity_state.launch_slot",
                len: index + 1,
                max: Self::LAUNCH_SLOT_CAPACITY,
            });
        }
        let at = Self::LAUNCH_SLOTS + index * Self::LAUNCH_SLOT_STRIDE;
        self.blob[at..at + 4].copy_from_slice(&hash.to_le_bytes());
        let count = (index as u32) + 1;
        self.blob[Self::LAUNCH_SLOT_COUNT..Self::LAUNCH_SLOT_COUNT + 4]
            .copy_from_slice(&count.to_le_bytes());
        Ok(())
    }

    pub fn struct_bytes(&self) -> Result<Vec<u8>, AhError> {
        if self.blob.len() != GLOBAL_ACTIVITY_STATE_BLOB_SIZE {
            return Err(AhError::BadStructSize {
                user_defined_type: protocol::UDT_GLOBAL_ACTIVITY_STATE,
                got: self.blob.len(),
                want: GLOBAL_ACTIVITY_STATE_BLOB_SIZE,
            });
        }
        // ⛔⛔ REFUSED HERE so the failure is a named error at build time rather than a Unicorn
        // fault inside the codec at PUSH time, mid-session, on the live wire. A non-zero
        // activity-slot count makes the client's own encoder deref the content registry
        // (*DAT_01A0B420) to size a dynamic-width leaf, which a static image does not carry.
        // ⇒ Self::LAUNCH_SLOT_COUNT.
        if self.launch_slot_count() != 0 {
            return Err(AhError::FieldOverflow {
                field: "global_activity_state.launch_slot_count is non-zero: the CLIENT'S OWN \
                        encoder faults on the content registry (READ_UNMAPPED at 0x105B74A via \
                        0x2C8FF0) while sizing the slot element's kind-0x29 dynamic-width leaf, \
                        so this blob cannot be put on the wire by any offline encoder - and the \
                        live Activity Host uses that same encoder",
                len: self.launch_slot_count() as usize,
                max: 0,
            });
        }
        Ok(self.blob.clone())
    }
}

/// `[C]` udt **11** `replicate_membership` - ⭐ the fourth and last message `FUN_00298EF0` waits
/// on, and the one that sets `ac+0x131 & 1`.
///
/// ## It is schema-coded after all - the retraction
///
/// > ⛔ **RETRACTED 2026-08-09.** This crate previously refused udt 11 and named it a gap:
/// > *"its RX handler `0x29CDE0` never calls `FUN_002A4090`; it calls `FUN_002D2090` (0x2800 buf,
/// > seed `0x811C9DC5`)."* **That is the handler for udt 7 `sensor_message`, not udt 11.** The
/// > receive dispatch table `0x18F84A0` is `{i32 type; i32 pad; fnptr}` and is **not** sorted by
/// > id - udt 11 sits at index 4, *before* udt 10 - so the index-to-udt mapping that produced the
/// > claim was invalid. udt 11's handler is `0x29CF50 → 0x29C730`, which is the same object codec
/// > with one extra leading presence bit. See [`d1_oracle::activity_host::replicate_membership`].
///
/// ## What we send, and why every byte of it is the client's
///
/// A **solo fireteam is one member: the client itself**, and the client just told us who it is.
/// `join_request`'s 40-byte identity at `+0x10` is the same struct (`0x80800C2D`, also udt 22
/// `client_identity_update`) as a membership record's identity at `+0x00`. Its `0x80800B1F`
/// properties at `+0x38` are likewise the first 184 bytes of the member's `0x80800C56` properties
/// field at member `+0x28`. So we echo both shared structures.
///
/// The other non-zero bytes are fields the client's own code reads back:
/// * `MEMBER_COUNT` `+0x00` - what `0x29C730` prints as `MEM-%d`: the membership UPDATE NUMBER
///   (56578 `0x778D60` logs it as "membership update '%d'" and re-reads the PAH region record
///   only when it changes). 1 at the join, advanced on every changed re-publish;
/// * `SIMULATION_OCCUPANCY_MASK` `+0x5028` bit 0 - the bit `FUN_00BA0B10` tests before copying
///   member 0's machine id into the simulation-machine cache;
/// * `OCCUPANCY_MASK` `+0x502C` bit 0 - the bit `FUN_0029FE70` tests when it looks for our slot.
/// * `ACKNOWLEDGED_SESSION_HOST` `+0x5020` - the player identifier the AH acknowledges as the
///   peer-session host. `FUN_0029FE40 -> FUN_002A4970` compares this qword with the local host
///   identifier stored immediately before the received membership image. Leaving it zero is the
///   exact predicate behind *"We are being identified as the host of this session, but the AH has
///   not acknowledged this"*.
/// * member-0 `FIRETEAM_ID` at image `+0xD0` - echoed from join-request `+0xD8`.
///   `fireteam_from_player()` (`FUN_0075DB30`) returns this qword, and the simulation-membership
///   diff (`FUN_00BA3D60`) refuses to enqueue a player-add when it is zero.
///
/// `GENERATION` `+0x04` stays **0** deliberately: `0x29C730` compares it with the client's current
/// value and **zeroes the peer table count `ac+0x64B4` on a difference**, and a fresh client holds
/// 0.
///
/// ⚠ `[U]` - the fields outside the echoed identity/properties and the four asserted fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplicateMembership {
    /// The whole `0x5038` struct image.
    pub image: Vec<u8>,
}

impl ReplicateMembership {
    pub const STRUCT_SIZE: usize = d1_oracle::activity_host::replicate_membership::STRUCT_SIZE;
    /// Where a membership record's identity lives, and where the *same* 40 bytes live inside a
    /// `join_request`.
    pub const MEMBERS: usize = d1_oracle::activity_host::replicate_membership::MEMBERS;
    pub const IDENTITY_SIZE: usize =
        d1_oracle::activity_host::replicate_membership::IDENTITY_SIZE;
    pub const JOIN_REQUEST_IDENTITY: usize =
        d1_oracle::activity_host::replicate_membership::JOIN_REQUEST_IDENTITY;
    pub const MEMBER_PROPERTIES: usize =
        d1_oracle::activity_host::replicate_membership::MEMBER_PROPERTIES;
    pub const PROPERTIES_PREFIX_SIZE: usize =
        d1_oracle::activity_host::replicate_membership::PROPERTIES_PREFIX_SIZE;
    pub const JOIN_REQUEST_PROPERTIES: usize =
        d1_oracle::activity_host::replicate_membership::JOIN_REQUEST_PROPERTIES;
    pub const MEMBER_FIRETEAM_ID: usize =
        d1_oracle::activity_host::replicate_membership::MEMBER_FIRETEAM_ID;
    pub const JOIN_REQUEST_FIRETEAM_ID: usize =
        d1_oracle::activity_host::replicate_membership::JOIN_REQUEST_FIRETEAM_ID;
    pub const SIMULATION_OCCUPANCY_MASK: usize =
        d1_oracle::activity_host::replicate_membership::SIMULATION_OCCUPANCY_MASK;
    pub const OCCUPANCY_MASK: usize =
        d1_oracle::activity_host::replicate_membership::OCCUPANCY_MASK;

    /// ⭐⭐ The peer-session host identity acknowledged by the Activity Host.
    ///
    /// Descriptor derivation: membership field `[3]` begins at `+0x2E48`; its final field is
    /// tag `0x8080405E` at nested `+0x21D8`, hence overall `+0x5020`. Runtime derivation:
    /// `FUN_0029FE40` passes the membership image to `FUN_002A4970`, which adds `0x5020` and
    /// compares that qword with the local host identifier. The setter `FUN_002A4960` copies the
    /// selected identifier to the same offset.
    pub const ACKNOWLEDGED_SESSION_HOST: usize = 0x5020;
    /// One membership record is `0x250` bytes (`0x808052FE`).
    pub const MEMBER_STRIDE: usize = 0x250;

    /// Acknowledge `peer_index` as the host of the peer session.
    ///
    /// The value is not fabricated: it is the first qword of that member's 40-byte identity,
    /// which was echoed from the client's join request. The current server is deliberately solo,
    /// so callers use peer 0; keeping the index explicit prevents this from becoming an accidental
    /// slot-0 assumption if multi-peer membership is added later.
    pub fn acknowledge_session_host(&mut self, peer_index: usize) -> Result<(), AhError> {
        if peer_index >= Self::PAH_PEER_TOKEN_CAPACITY {
            return Err(AhError::FieldOverflow {
                field: "replicate_membership.acknowledged_session_host.peer_index",
                len: peer_index + 1,
                max: Self::PAH_PEER_TOKEN_CAPACITY,
            });
        }
        let source = Self::MEMBERS + peer_index * Self::MEMBER_STRIDE;
        let target = Self::ACKNOWLEDGED_SESSION_HOST;
        if self.image.len() < source + 8 || self.image.len() < target + 8 {
            return Err(AhError::BadStructSize {
                user_defined_type: protocol::UDT_REPLICATE_MEMBERSHIP,
                got: self.image.len(),
                want: Self::STRUCT_SIZE,
            });
        }
        let host = self.image[source..source + 8].to_vec();
        self.image[target..target + 8].copy_from_slice(&host);
        Ok(())
    }

    /// The acknowledged host identity, for controls and run provenance.
    pub fn acknowledged_session_host(&self) -> &[u8] {
        &self.image[Self::ACKNOWLEDGED_SESSION_HOST..Self::ACKNOWLEDGED_SESSION_HOST + 8]
    }

    /// ⭐⭐ Field `[3]` of the membership struct → tag `0x80800BD7`, 8,672 bytes: the **PAH
    /// tabulated region data**. This is the rung-34 knob.
    ///
    /// `FUN_007775B0` - *"Processing new PAH tabulated region data in membership update '%d'."* -
    /// reads it out of the membership object at `ac+0xE68 + 0x2E50`, which is image `+0x2E48`
    /// (`MEMBERS 0x08 + 20 * 0x250 = 0x2E48`, and the descriptor tree says field `[3] @+0x2e48`:
    /// two derivations, one answer). It walks up to 40 records of `0xD8` bytes, **breaking on
    /// `region_id == -1`**, and copies the record whose
    ///
    /// * `region_id == sstm+0x138` (the PENDING region) **and**
    /// * `peer_tokens[sstm+0x3B4] == sstm+0x1D0` (our peer slot's byte == the transition token)
    ///
    /// into `sstm+0x490`. `FUN_0077F6B0` then requires `sstm+0x490 != -1` **and** that same byte
    /// equality, or it prints *"Region '%s' is ready for switch, but we do not yet have valid
    /// host-tabulated data for it. Waiting."* forever - which is where every run has stopped.
    /// ⇒ `historicaldocs/design/rung34-task7.md`.
    pub const PAH_REGIONS: usize = 0x2E48;
    /// `0x808051FE` is 8,640 bytes with a single field record at `+0xD8` ⇒ stride `0xD8`,
    /// capacity 40. `FUN_007775B0`'s own loop is `piVar13 += 0x36` ints (= `0xD8`) `while (i < 0x28)`.
    pub const PAH_REGION_STRIDE: usize = 0xD8;
    /// 40. `8640 / 216`.
    pub const PAH_REGION_CAPACITY: usize = 40;
    /// Record `+0x00`, a `u32`: the region id. `0xFFFFFFFF` **terminates the scan**.
    pub const PAH_REGION_ID: usize = 0x00;
    /// Record `+0x38`: `0x80805491`, a **20-byte array with stride 1** - one token byte per peer
    /// slot, indexed by `FUN_0029FE70`'s local peer index. Derived twice: from the descriptor
    /// (`0x80800C9F` field `[2] @+0x010`, and `0x28 + 0x10 = 0x38`) and from the decompiler
    /// (`*(char *)((long)piVar13 + local_peer_index + 0x38)`).
    pub const PAH_PEER_TOKENS: usize = 0x38;
    /// 20 - the same cap as the member array.
    pub const PAH_PEER_TOKEN_CAPACITY: usize = 20;
    /// Region record `+0x09`: ambassadorship, 1 = assigned (2 = revoking). The client takes the
    /// ambassador's path for a PUBLIC region (`0x7791C7`: this == 1 and `+0x0A` == the local peer;
    /// "PAH assigned ambassadorship to us"), then creates the public session. 0 leaves it waiting
    /// to join a session nobody advertises (notes/ps4-56578-public-sessions-2026-11-02.md).
    pub const PAH_AMBASSADORSHIP: usize = 0x09;
    /// Region record `+0x0A`: the ambassador's peer index.
    pub const PAH_AMBASSADOR_PEER: usize = 0x0A;
    /// Region record `+0x04`: 1 for a PUBLIC region. The live host set it for Restoration's public
    /// regions 56 and 48 and left it 0 for A Guardian Rises' private ones
    /// (notes/d1-live-captures-2026-11-05.md, "Restoration's public start").
    pub const PAH_REGION_PUBLIC: usize = 0x04;
    /// Region record `+0x08`: 1 alongside the ambassadorship in every live record, private regions
    /// included. `[U]` meaning.
    pub const PAH_REGION_08: usize = 0x08;

    /// The live host's flags for one region record: with `ambassador`, `+0x08 = 1`, `+0x09 = 1`
    /// and `+0x0A = peer_index` (every live session); with `public`, also `+0x04 = 1`.
    pub fn apply_region_flags(record: &mut [u8], peer_index: usize, ambassador: bool, public: bool) {
        if ambassador {
            record[Self::PAH_REGION_08] = 1;
            record[Self::PAH_AMBASSADORSHIP] = 1;
            record[Self::PAH_AMBASSADOR_PEER] = peer_index as u8;
        }
        if public {
            record[Self::PAH_REGION_PUBLIC] = 1;
        }
    }

    /// ⭐⭐ Publish **one** PAH-tabulated region record, in slot 0, and terminate the array.
    ///
    /// `region_id` is the region the client is transitioning **to** (`sstm+0x138`; measured `0`
    /// for a one-bubble PvP map - `PRV00.00` is `sprintf("%03s%02d.%02d", "PRV", id, id)`).
    /// `peer_index` is our slot in the membership (`0` for a solo fireteam, measured live).
    /// `token` is the transition token: `FUN_0077E5A0` does `token = sstm[0x1D0] + 1; if (token ==
    /// 0) token = 1;` on **every** transition, so it is `1` on the first slice-set transition after
    /// a boot and increments thereafter. It is **not** derivable from anything we send.
    ///
    /// ⚠ Slot 1's region id is set to `-1` deliberately. Left at `0` it would be a *valid duplicate*
    /// of region 0, and `FUN_007775B0`'s **CURRENT**-region copy (`if (id == sstm+0x98) memcpy(
    /// sstm+0x3B8, …)`) has **no token condition** - so once the region became CURRENT, 39 zeroed
    /// duplicates would each overwrite the good record and the last (empty) one would win. The
    /// terminator is not tidiness; it is the difference between one record and forty.
    /// ⭐ The host's slice-set REQUEST for the whole group (the host teleport): group block
    /// `+0x2E48` field `+0x21C8`, type `0x80800BF9` = {u3 state, {u8, i10 slice-set index, u32
    /// name hash}}. Same offsets as 01.24, where it moved A Guardian Rises from the intro slice
    /// set 113 to gameplay 112: state 1 asks, the client switches, state 3 finishes.
    pub const GROUP_SLICE_REQUEST: usize = 0x2E48 + 0x21C8;

    /// Write the group slice-set request (see [`Self::GROUP_SLICE_REQUEST`]).
    pub fn set_slice_request(&mut self, state: u8, index: u32, name_hash: u32) -> Result<(), String> {
        let o = Self::GROUP_SLICE_REQUEST;
        if self.image.len() < o + 0x10 {
            return Err(format!("membership image is {} bytes, too short for +0x{o:X}", self.image.len()));
        }
        self.image[o] = state;
        self.image[o + 4] = 0;
        self.image[o + 8..o + 12].copy_from_slice(&index.to_le_bytes());
        self.image[o + 12..o + 16].copy_from_slice(&name_hash.to_le_bytes());
        Ok(())
    }

    /// The group slice-set request as (state, index).
    pub fn slice_request(&self) -> Option<(u8, u32)> {
        let o = Self::GROUP_SLICE_REQUEST;
        let b = self.image.get(o..o + 12)?;
        Some((b[0], u32::from_le_bytes(b[8..12].try_into().ok()?)))
    }

    pub fn set_pah_region(
        &mut self,
        region_id: u32,
        peer_index: usize,
        token: u8,
    ) -> Result<(), AhError> {
        if peer_index >= Self::PAH_PEER_TOKEN_CAPACITY {
            return Err(AhError::FieldOverflow {
                field: "replicate_membership.pah_region.peer_index",
                len: peer_index + 1,
                max: Self::PAH_PEER_TOKEN_CAPACITY,
            });
        }
        if self.image.len() < Self::PAH_REGIONS + 2 * Self::PAH_REGION_STRIDE {
            return Err(AhError::BadStructSize {
                user_defined_type: protocol::UDT_REPLICATE_MEMBERSHIP,
                got: self.image.len(),
                want: Self::STRUCT_SIZE,
            });
        }
        let slot0 = Self::PAH_REGIONS;
        self.image[slot0 + Self::PAH_REGION_ID..slot0 + Self::PAH_REGION_ID + 4]
            .copy_from_slice(&region_id.to_le_bytes());
        self.image[slot0 + Self::PAH_PEER_TOKENS + peer_index] = token;
        // D1_AH_PUBLIC_AMBASSADOR=1: assign the region's ambassadorship to this peer (step 2 of
        // the public-session plan), as the live host did for every region; D1_AH_PUBLIC_REGIONS
        // (comma-separated region ids) marks those regions public. Set here so every re-publish
        // path re-applies them.
        let ambassador = std::env::var("D1_AH_PUBLIC_AMBASSADOR").is_ok_and(|v| v == "1");
        let public = std::env::var("D1_AH_PUBLIC_REGIONS").is_ok_and(|v| {
            v.split(',').any(|r| r.trim().parse::<u32>().is_ok_and(|r| r == region_id))
        });
        Self::apply_region_flags(&mut self.image[slot0..slot0 + Self::PAH_REGION_STRIDE], peer_index, ambassador, public);
        // The terminator. See the doc comment - this is load-bearing, not hygiene.
        let slot1 = Self::PAH_REGIONS + Self::PAH_REGION_STRIDE;
        self.image[slot1 + Self::PAH_REGION_ID..slot1 + Self::PAH_REGION_ID + 4]
            .copy_from_slice(&u32::MAX.to_le_bytes());
        Ok(())
    }

    /// Region record `+0x4C`: the advertised public session, `0x84` bytes up to `+0xD0`.
    pub const PAH_ADVERTISED_SESSION: usize = 0x4C;
    pub const PAH_ADVERTISED_SESSION_LEN: usize = 0x84;
    /// Region record `+0xD0`: the advertised public session's activity-host id.
    pub const PAH_ADVERTISED_HOST: usize = 0xD0;

    /// Advertise a public session in slot 0's region record (plan step 5 in
    /// notes/ps4-56578-public-sessions-2026-11-02.md). The live PAH copied both from the client's
    /// own udt 21 slice-set record once the client had joined its new public instance
    /// (notes/d1-live-captures-2026-11-05.md): the session at `+0x4C` and that instance's host
    /// id at `+0xD0`. The session is the client's platform session, echoed, never built here.
    pub fn set_region_advertisement(&mut self, session: &[u8], host: u64) -> Result<(), AhError> {
        if session.len() != Self::PAH_ADVERTISED_SESSION_LEN {
            return Err(AhError::FieldOverflow {
                field: "replicate_membership.pah_region.advertised_session",
                len: session.len(),
                max: Self::PAH_ADVERTISED_SESSION_LEN,
            });
        }
        let slot0 = Self::PAH_REGIONS;
        if self.image.len() < slot0 + Self::PAH_REGION_STRIDE {
            return Err(AhError::BadStructSize {
                user_defined_type: protocol::UDT_REPLICATE_MEMBERSHIP,
                got: self.image.len(),
                want: Self::STRUCT_SIZE,
            });
        }
        let s = slot0 + Self::PAH_ADVERTISED_SESSION;
        self.image[s..s + Self::PAH_ADVERTISED_SESSION_LEN].copy_from_slice(session);
        let h = slot0 + Self::PAH_ADVERTISED_HOST;
        self.image[h..h + 8].copy_from_slice(&host.to_le_bytes());
        Ok(())
    }

    /// The region id currently in slot 0 of the tabulated array, for the run log and the controls.
    pub fn pah_region_id(&self) -> u32 {
        let o = Self::PAH_REGIONS + Self::PAH_REGION_ID;
        u32::from_le_bytes(self.image[o..o + 4].try_into().unwrap_or([0; 4]))
    }

    /// Slot 0's token byte for `peer_index`, for the run log and the controls.
    pub fn pah_peer_token(&self, peer_index: usize) -> u8 {
        self.image
            .get(Self::PAH_REGIONS + Self::PAH_PEER_TOKENS + peer_index)
            .copied()
            .unwrap_or(0)
    }

    /// ⭐ One member, the client itself, echoed out of its own `join_request` struct image.
    ///
    /// Refused rather than improvised if the request is too short - a membership with a zero
    /// identity would make `FUN_0029FE70` fail to find our slot, while truncated properties would
    /// silently recreate the zero-fireteam suppression in `FUN_00BA3D60`.
    pub fn solo(join_request_image: &[u8]) -> Result<Self, AhError> {
        d1_oracle::activity_host::replicate_membership::solo(join_request_image)
            .map(|image| Self { image })
            .ok_or(AhError::BadStructSize {
                user_defined_type: protocol::UDT_JOIN_REQUEST,
                got: join_request_image.len(),
                want: Self::JOIN_REQUEST_PROPERTIES + Self::PROPERTIES_PREFIX_SIZE,
            })
    }

    /// The identity we echoed, for the run log.
    pub fn member_identity(&self) -> &[u8] {
        &self.image[Self::MEMBERS..Self::MEMBERS + Self::IDENTITY_SIZE]
    }

    /// The common properties prefix echoed from the join request.
    pub fn member_properties_prefix(&self) -> &[u8] {
        let start = Self::MEMBERS + Self::MEMBER_PROPERTIES;
        &self.image[start..start + Self::PROPERTIES_PREFIX_SIZE]
    }

    /// The fireteam identifier consumed by `FUN_0075DB30`.
    pub fn member_fireteam_id(&self) -> u64 {
        u64::from_le_bytes(
            self.image[Self::MEMBER_FIRETEAM_ID..Self::MEMBER_FIRETEAM_ID + 8]
                .try_into()
                .unwrap_or([0; 8]),
        )
    }

    pub fn struct_bytes(&self) -> Result<Vec<u8>, AhError> {
        if self.image.len() != Self::STRUCT_SIZE {
            return Err(AhError::BadStructSize {
                user_defined_type: protocol::UDT_REPLICATE_MEMBERSHIP,
                got: self.image.len(),
                want: Self::STRUCT_SIZE,
            });
        }
        Ok(self.image.clone())
    }
}

/// Every AH → client message this crate can build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AhMessage {
    /// udt 0 - the granted entity-index mask.
    EntitySlotsAllocated(EntitySlotMask),
    /// udt 1 - the authoritative Activity State.
    GlobalActivityState(GlobalActivityState),
    /// udt 4 - the join lease.
    JoinResult(JoinResult),
    /// udt 11 - who is in this activity. ⭐ The last of `FUN_00298EF0`'s four.
    ReplicateMembership(ReplicateMembership),
    /// ⭐⭐ udt 5 - `sensor_auth_update`, the wire message that writes the activity lifetime phase.
    /// Its wire form is a bespoke replicated frame, not a single-descriptor struct, so it is
    /// **pre-built once** by [`d1_oracle::sensor`] (at instance setup, off the send path) and
    /// carried here as finished wire bytes. The codec sends them verbatim - see
    /// [`crate::codec::SchemaCodec::build_sensor`].
    SensorAuthUpdate { payload: Vec<u8> },
    /// udt 42 - `{u8 epoch, u8 suppress}`.
    AdvanceReplicationEpoch { epoch: u8, suppress: u8 },
    /// udt 43 - `{i32 count, u64 machine_ids[]}`.
    ReservationsFailed { machine_ids: Vec<u64> },
    /// Anything else, as bytes. The direction check still applies.
    Opaque { user_defined_type: u32, bytes: Vec<u8> },
}

impl AhMessage {
    pub fn user_defined_type(&self) -> u32 {
        match self {
            Self::EntitySlotsAllocated(_) => protocol::UDT_ENTITY_SLOTS_ALLOCATED,
            Self::GlobalActivityState(_) => protocol::UDT_GLOBAL_ACTIVITY_STATE,
            Self::JoinResult(_) => protocol::UDT_JOIN_RESULT,
            Self::ReplicateMembership(_) => protocol::UDT_REPLICATE_MEMBERSHIP,
            Self::SensorAuthUpdate { .. } => protocol::UDT_SENSOR_AUTH_UPDATE,
            Self::AdvanceReplicationEpoch { .. } => protocol::UDT_ADVANCE_REPLICATION_EPOCH,
            Self::ReservationsFailed { .. } => protocol::UDT_RESERVATIONS_FAILED,
            Self::Opaque { user_defined_type, .. } => *user_defined_type,
        }
    }

    /// The client's in-memory struct image. **Not the wire** - see [`crate::codec`].
    ///
    /// Infallible by construction for the variants that can overflow: those are validated when
    /// built, and [`Self::try_struct_bytes`] is the checked form.
    pub fn struct_bytes(&self) -> Vec<u8> {
        self.try_struct_bytes().unwrap_or_default()
    }

    /// [`Self::struct_bytes`] with the field-capacity checks surfaced.
    pub fn try_struct_bytes(&self) -> Result<Vec<u8>, AhError> {
        match self {
            Self::EntitySlotsAllocated(m) => Ok(m.to_struct_bytes()),
            Self::GlobalActivityState(s) => s.struct_bytes(),
            Self::JoinResult(j) => j.struct_bytes(),
            Self::ReplicateMembership(m) => m.struct_bytes(),
            // ⭐ `sensor_auth_update` carries finished wire bytes, not a struct image. The codec
            // special-cases it *before* this method (returning the bytes verbatim), so reaching
            // here means a caller tried to treat it as a plain struct.
            Self::SensorAuthUpdate { .. } => Err(AhError::SchemaCodec {
                user_defined_type: protocol::UDT_SENSOR_AUTH_UPDATE,
                detail: "sensor_auth_update carries finished wire bytes, not a struct image; the \
                         codec sends them verbatim through its special case"
                    .to_string(),
            }),
            Self::AdvanceReplicationEpoch { epoch, suppress } => Ok(vec![*epoch, *suppress]),
            Self::ReservationsFailed { machine_ids } => {
                let mut b = Vec::with_capacity(4 + 8 * machine_ids.len());
                b.extend_from_slice(&(machine_ids.len() as i32).to_le_bytes());
                for id in machine_ids {
                    b.extend_from_slice(&id.to_le_bytes());
                }
                Ok(b)
            }
            Self::Opaque { bytes, .. } => Ok(bytes.clone()),
        }
    }
}

// ── client → AH ───────────────────────────────────────────────────────────────────────────────

/// A decoded client → AH message.
///
/// ⚠ These are built from a **decoded struct image**, which today means [`crate::codec`] has to
/// have produced one. With the default [`crate::codec::SchemaCodec`] that never happens, so the
/// listener reports the gate rather than pretending to have parsed anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientMessage {
    /// udt 3, `0x100` struct. `[U]` internally; the one field we need is the nonce, and we echo it
    /// rather than interpret it.
    JoinRequest { struct_image: Vec<u8> },
    /// udt 19, `u32 count`.
    AllocateEntityIndices { count: u32 },
    /// udt 20, the `0x600` mask.
    FreeEntityIndices(EntitySlotMask),
    /// udt 14, `0x14` struct. ⛔ The client **giving up** - a consequence, never a cause.
    PeerLeaveRequest { struct_image: Vec<u8> },
    /// udt 15, one byte. ⚠ The byte is an *uninitialised stack local* in `FUN_00298C40`; there is
    /// nothing in it to read, which is why this variant carries no data.
    Keepalive,
    /// udt 17, `0x008` struct. ⭐ The client asking the Activity Host to re-publish state.
    RequestStateRefresh { struct_image: Vec<u8> },
    /// udt 21, `0x220` struct. ⭐ The client's **own** authoritative state. Fire-and-forget: there
    /// is no reply message for it and no client handler that could receive one.
    ClientAuthoritativeDataUpdate { struct_image: Vec<u8> },
    /// udt 36, `u32`.
    MembershipAcknowledgement { value: u32 },
    /// udt 22.
    ClientIdentityUpdate { struct_image: Vec<u8> },
    /// udt 35.
    ConnectivityFailure { struct_image: Vec<u8> },
    /// Anything else in the 49, unparsed.
    Other { user_defined_type: u32, struct_image: Vec<u8> },
}

/// `[I]` - the join nonce is the first four bytes of the request, because it is the first four
/// bytes of the *result* and the client compares the two against one value it holds itself. There
/// is no second candidate in the struct and no cost to being wrong about its *type*, because we
/// echo the bytes rather than read them.
pub const JOIN_REQUEST_NONCE_OFFSET: usize = 0;

impl ClientMessage {
    pub fn user_defined_type(&self) -> u32 {
        match self {
            Self::JoinRequest { .. } => protocol::UDT_JOIN_REQUEST,
            Self::AllocateEntityIndices { .. } => protocol::UDT_ALLOCATE_ENTITY_INDICES,
            Self::FreeEntityIndices(_) => protocol::UDT_FREE_ENTITY_INDICES,
            Self::PeerLeaveRequest { .. } => protocol::UDT_PEER_LEAVE_REQUEST,
            Self::Keepalive => protocol::UDT_KEEPALIVE_REQUEST,
            Self::RequestStateRefresh { .. } => protocol::UDT_REQUEST_STATE_REFRESH,
            Self::ClientAuthoritativeDataUpdate { .. } => {
                protocol::UDT_CLIENT_AUTHORITATIVE_DATA_UPDATE
            }
            Self::MembershipAcknowledgement { .. } => protocol::UDT_MEMBERSHIP_ACKNOWLEDGEMENT,
            Self::ClientIdentityUpdate { .. } => protocol::UDT_CLIENT_IDENTITY_UPDATE,
            Self::ConnectivityFailure { .. } => protocol::UDT_CONNECTIVITY_FAILURE,
            Self::Other { user_defined_type, .. } => *user_defined_type,
        }
    }

    /// Interpret a **decoded struct image** as one of the messages we act on.
    ///
    /// Anything we do not model becomes [`Self::Other`] rather than an error: an Activity Host
    /// that fatals on an unmodelled notification would be worse than one that logs it.
    pub fn from_struct_image(user_defined_type: u32, image: &[u8]) -> Result<Self, AhError> {
        protocol::check_receivable(user_defined_type)?;
        Ok(match user_defined_type {
            protocol::UDT_JOIN_REQUEST => Self::JoinRequest { struct_image: image.to_vec() },
            protocol::UDT_ALLOCATE_ENTITY_INDICES => {
                if image.len() < 4 {
                    return Err(AhError::BadStructSize { user_defined_type, got: image.len(), want: 4 });
                }
                Self::AllocateEntityIndices {
                    count: u32::from_le_bytes([image[0], image[1], image[2], image[3]]),
                }
            }
            protocol::UDT_FREE_ENTITY_INDICES => EntitySlotMask::from_struct_bytes(image)
                .map(Self::FreeEntityIndices)
                .ok_or(AhError::BadStructSize {
                    user_defined_type,
                    got: image.len(),
                    want: crate::entity::ENTITY_MASK_BYTES,
                })?,
            protocol::UDT_PEER_LEAVE_REQUEST => {
                Self::PeerLeaveRequest { struct_image: image.to_vec() }
            }
            protocol::UDT_KEEPALIVE_REQUEST => Self::Keepalive,
            protocol::UDT_REQUEST_STATE_REFRESH => {
                Self::RequestStateRefresh { struct_image: image.to_vec() }
            }
            protocol::UDT_CLIENT_AUTHORITATIVE_DATA_UPDATE => {
                Self::ClientAuthoritativeDataUpdate { struct_image: image.to_vec() }
            }
            protocol::UDT_MEMBERSHIP_ACKNOWLEDGEMENT => {
                if image.len() < 4 {
                    return Err(AhError::BadStructSize { user_defined_type, got: image.len(), want: 4 });
                }
                Self::MembershipAcknowledgement {
                    value: u32::from_le_bytes([image[0], image[1], image[2], image[3]]),
                }
            }
            protocol::UDT_CLIENT_IDENTITY_UPDATE => {
                Self::ClientIdentityUpdate { struct_image: image.to_vec() }
            }
            protocol::UDT_CONNECTIVITY_FAILURE => {
                Self::ConnectivityFailure { struct_image: image.to_vec() }
            }
            other => Self::Other { user_defined_type: other, struct_image: image.to_vec() },
        })
    }

    /// The four bytes to echo back in `join_result`, if this is a join request.
    pub fn join_nonce(&self) -> Option<[u8; 4]> {
        match self {
            Self::JoinRequest { struct_image } => struct_image
                .get(JOIN_REQUEST_NONCE_OFFSET..JOIN_REQUEST_NONCE_OFFSET + 4)
                .and_then(|s| s.try_into().ok()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The region record's flags as the live host set them.
    #[test]
    fn region_flags_match_the_live_records() {
        // notes/d1-live-captures-2026-11-05.md: private `01 01 00 00` at +0x08, public also +0x04 = 1.
        let mut private = vec![0u8; ReplicateMembership::PAH_REGION_STRIDE];
        ReplicateMembership::apply_region_flags(&mut private, 0, true, false);
        assert_eq!(&private[0x04..0x0C], &[0, 0, 0, 0, 1, 1, 0, 0]);
        let mut public = vec![0u8; ReplicateMembership::PAH_REGION_STRIDE];
        ReplicateMembership::apply_region_flags(&mut public, 0, true, true);
        assert_eq!(&public[0x04..0x0C], &[1, 0, 0, 0, 1, 1, 0, 0]);
        let mut off = vec![0u8; ReplicateMembership::PAH_REGION_STRIDE];
        ReplicateMembership::apply_region_flags(&mut off, 3, false, false);
        assert!(off.iter().all(|b| *b == 0));
    }

    /// Every offset in the `join_result` doc-comment, asserted at the byte. If one of these ever
    /// moves, the client silently reads a different field - there is no error path for it.
    #[test]
    fn join_result_lands_every_field_on_the_offset_the_client_reads() {
        let mut j = JoinResult::accepted([0xDE, 0xAD, 0xBE, 0xEF], 0x0102_0304_0506_0708, "S1", 7);
        j.field_18c = 0x1111_1111;
        j.field_190 = 0x2222_2222;
        j.grace_ms = 0x3344;
        j.keepalive_interval_ms = 0x5566;
        let b = j.struct_bytes().unwrap();

        assert_eq!(b.len(), 0x1A8);
        assert_eq!(&b[0x000..0x004], &[0xDE, 0xAD, 0xBE, 0xEF]);
        assert_eq!(&b[0x008..0x010], &0x0102_0304_0506_0708u64.to_le_bytes());
        assert_eq!(b[0x100], 0, "0 = accepted");
        assert_eq!(&b[0x101..0x103], b"S1");
        assert_eq!(b[0x103], 0, "the session id is NUL-terminated");
        assert_eq!(&b[0x18C..0x190], &0x1111_1111u32.to_le_bytes());
        assert_eq!(&b[0x190..0x194], &0x2222_2222u32.to_le_bytes());
        assert_eq!(b[0x194], 7, "the replication epoch is a u8 at +0x194");
        assert_eq!(&b[0x196..0x198], &0x3344u16.to_le_bytes());
        assert_eq!(&b[0x198..0x19A], &0x5566u16.to_le_bytes());
    }

    #[test]
    fn a_session_id_that_would_run_past_the_field_is_refused_not_truncated() {
        let mut j = JoinResult::accepted([0; 4], 1, "x", 0);
        j.session_id = vec![b'A'; JOIN_RESULT_SESSION_ID_CAPACITY];
        assert!(matches!(j.struct_bytes(), Err(AhError::FieldOverflow { .. })));
        // Positive control: one byte shorter fits, leaving room for the NUL.
        j.session_id = vec![b'A'; JOIN_RESULT_SESSION_ID_CAPACITY - 1];
        assert!(j.struct_bytes().is_ok());
    }

    /// The size identity that ties AH udt 10 to BAP message 7. If this breaks, the two are not the
    /// same thing after all and `registry` is wrong.
    #[test]
    fn start_activity_host_response_is_exactly_bap_message_sevens_payload() {
        let s = StartActivityHostResponse { ah_id: 0xAABB, session_id: b"sess".to_vec() };
        let b = s.struct_bytes().unwrap();
        assert_eq!(b.len(), 0x88);
        assert_eq!(&b[0..8], &0xAABBu64.to_le_bytes());
        assert_eq!(&b[8..12], b"sess");
    }

    #[test]
    fn global_activity_state_is_exactly_1712_bytes_and_not_8_more() {
        if !crate::eboot_or_skip("global_activity_state_is_exactly_1712_bytes_and_not_8_more") {
            return;
        }
        let s = GlobalActivityState::empty();
        let b = s.struct_bytes().unwrap();
        assert_eq!(b.len(), 1712, "the descriptor's struct_size, NOT 8 + 1712");
        assert_eq!(GLOBAL_ACTIVITY_STATE_BLOB_SIZE, 0x6B0);
        assert_eq!(
            b.len(),
            d1_oracle::activity_host::AhCodec::new().struct_size(1).unwrap(),
            "two instruments: our constant and the eboot's own descriptor"
        );
        // A wrong-sized blob is refused rather than padded.
        let bad = GlobalActivityState { blob: vec![0; 10] };
        assert!(matches!(bad.struct_bytes(), Err(AhError::BadStructSize { .. })));
    }

    #[test]
    fn advance_replication_epoch_is_two_bytes() {
        let m = AhMessage::AdvanceReplicationEpoch { epoch: 3, suppress: 1 };
        assert_eq!(m.try_struct_bytes().unwrap(), vec![3, 1]);
        assert_eq!(m.user_defined_type(), 42);
    }

    #[test]
    fn client_messages_decode_the_two_shapes_the_state_machine_needs() {
        let a = ClientMessage::from_struct_image(19, &7u32.to_le_bytes()).unwrap();
        assert_eq!(a, ClientMessage::AllocateEntityIndices { count: 7 });
        let k = ClientMessage::from_struct_image(15, &[0]).unwrap();
        assert_eq!(k, ClientMessage::Keepalive);
        // Positive control that the direction gate is live on this path too.
        assert!(matches!(
            ClientMessage::from_struct_image(4, &[0; 0x1A8]),
            Err(AhError::WrongDirection { .. })
        ));
    }

    #[test]
    fn an_unmodelled_but_legal_client_message_is_logged_not_fatal() {
        let m = ClientMessage::from_struct_image(45, &[1, 2, 3]).unwrap();
        assert_eq!(m, ClientMessage::Other { user_defined_type: 45, struct_image: vec![1, 2, 3] });
    }

    #[test]
    fn the_join_nonce_is_echoed_as_bytes_not_reinterpreted() {
        let mut img = vec![0u8; 0x100];
        img[..4].copy_from_slice(&[1, 2, 3, 4]);
        let m = ClientMessage::from_struct_image(3, &img).unwrap();
        assert_eq!(m.join_nonce(), Some([1, 2, 3, 4]));
        assert_eq!(ClientMessage::Keepalive.join_nonce(), None);
    }

    #[test]
    fn allocate_entity_indices_refuses_a_short_struct() {
        assert!(matches!(
            ClientMessage::from_struct_image(19, &[1, 2]),
            Err(AhError::BadStructSize { want: 4, .. })
        ));
    }
}
