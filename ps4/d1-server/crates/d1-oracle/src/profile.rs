// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Build a **populated family-4 / type-1 character profile** - schema `0x80801743`, 28,792 bytes.
//!
//! `historicaldocs/design/THE-EMPTY-PROFILE.md` measured the fixture this project has been shipping since
//! 2026-08-08: it decodes to **exactly two non-zero fields out of 28,792 bytes** (the SOID and
//! `last_orbited_destination`). The blank Guardian on screen is a faithful rendering of that object.
//! This module is the fix: one place that knows where every region of the profile lives, and one
//! encode path that goes through the **client's own encoder** ([`crate::ObjectOracle`]) so a
//! hand-packing mistake becomes a refusal instead of a payload.
//!
//! ```text
//! d1-oracle profile layout                              # the derived offset table
//! d1-oracle profile controls                            # 12 checks, exit 2 on failure
//! d1-oracle profile build --out capture-rig/profiles/profile-populated.bin
//! ```
//!
//! # ⛔ THE OFFSET CORRECTION THAT THIS MODULE EXISTS TO PIN
//!
//! Two live documents place the character stat-override array at **`+0x6CE8` with 6 entries**
//! (`historicaldocs/design/light-and-gear.md` §3, `historicaldocs/design/level-light-customization.md` §4.3). **Both are wrong by
//! eight bytes.** It is at **`+0x6CE0` with 7 entries**, and three independent instruments agree:
//!
//! 1. **The schema.** `0x808018BB` @`+0x6CD0` → field `[3]` @`+0x00C` = `0x80801912` @`+0x6CDC`
//!    (`bool present`, 0-bit leaf) → field `[1]` @`+0x004` = `0x80804768` @`+0x6CE0`, whose
//!    descriptor header declares **element count 7, element size 8**, and `7 × 8 == 56 ==` the
//!    container's struct size. It closes exactly against the next sibling at `+0x6D18`.
//! 2. **The reader.** `0x265EF0`:
//!    ```text
//!    0x265f25  mov rax, [rbp+0x18]            ; rax = profile + 0x6CD0   (0x266144: lea r15,[r15+0x6cd0])
//!    0x265f2e  cmp byte [rax+0xc], 0 ; je     ; present  -> +0x6CDC
//!    0x265f40  cmp edi, 6 ; jg                ; i = 0..6                 -> SEVEN entries
//!    0x265f48  cmp [rax+rdi*8+0x10], bl       ; key[i]  -> +0x6CE0 + 8i
//!    0x265f4c  lea rdi, [rdi+1]               ; ⚠ incremented AFTER the key compare …
//!    0x265f52  mov eax, [rax+rdi*8+0xc]       ; … so value[i] -> +0x6CE4 + 8i, not +0xC + 8i
//!    ```
//!    Reading the `lea` as a *pre*-increment is what produced `+0x6CE8`; the increment only ever
//!    reaches the value load.
//! 3. **The codec.** Seven entries written at `+0x6CE0` round-trip `object IDENTICAL`; seven
//!    written at `+0x6CE8` are **refused** - `round trip differs at +0x6d18`, because the seventh
//!    lands on the *second* override block's `present` bool. Both are [`controls`].
//!
//! # ⚠ `d1-eboot descriptors` prints the ELEMENT SIZE in the offset column of an array field
//!
//! For a `kind 0x1` field the printed `@+0xNN` is the element **stride**, not a C offset - the array
//! begins at the *parent* field's offset. `0x80804768`'s single field prints `@+0x008`, which is the
//! 8-byte element, and reading it as an offset is the other half of the `+0x6CE8` error. Element
//! count and size are read here from the container's descriptor header (`anchor+0x20` / `anchor+0x28`)
//! rather than from that column, and [`derive_layout`] asserts `count * size == struct_size` on every
//! array it touches.
//!
//! # What is deliberately NOT written
//!
//! * **`+0x28`** is `last_orbited_destination`, *not* customization field `[10]`/`personality`
//!   (`THE-EMPTY-PROFILE.md` §3 - a live read beats the shape-match inference). Orbit depends on it.
//!   The other 11 customization fields are left alone: writing a hash-shaped `u32` we cannot resolve
//!   is a guess, not a fix.
//! * **Progressions** default to count 0. The `u16` ordinal for `character_level` is `[U]` - it is
//!   not the manifest hash `0x6650C0F9` and no content hash appears as an immediate anywhere in the
//!   eboot. [`ProfileSpec::progressions`] is wired end to end so that closing that `[U]` is one line.
//! * ~~**Item content indices.**~~ ⭐ **CLOSED 2026-08-09** (`historicaldocs/design/item-content-indices.md`): a
//!   populated inventory slot is `{u16 content_index, u64 soid}` and the `u16` is the element index
//!   of a 7,544-row table in `ps4_investment_globals_012f_3.pkg` entry 8126. [`ProfileSpec::item_slots`]
//!   writes them; unnamed slots stay **`0xFFFF`** rather than zero, which would claim 340 copies of
//!   content index 0 - see [`ProfileSpec::empty_item_slots`].

use std::collections::BTreeSet;
use std::path::Path;

use d1_eboot::descriptors::{self, Descriptor, Field, Index, LEAF};
use d1_eboot::image::Image;

use crate::error::{OracleError, Result};
use crate::object::Decoded;
use crate::ObjectOracle;

/// Family 4 / type 1 - the profile the in-world character reads.
pub const TAG: u32 = 0x8080_1743;
/// Family 4 / type 3 - one inventory item element, 184 bytes, SOID at `+0x00`.
pub const ITEM_TAG: u32 = 0x8080_1732;
pub const STRUCT_SIZE: usize = 28_792;

// ── C offsets. ABSOLUTE in the decoded struct. Every one is re-derived by `derive_layout`. ───────
pub const OFF_SOID: usize = 0x0000;
/// `{u8 race, u8 gender, u8 class}` - `0x808017FC`.
pub const OFF_IDENTITY: usize = 0x0008;
/// `0x8080181A`, 36 bytes, 12 fields.
pub const OFF_CUSTOMIZATION: usize = 0x000C;
/// Customization field `[10]` (block `+0x1C`). ⚠ **`last_orbited_destination`, measured live.**
pub const OFF_LAST_ORBITED_DESTINATION: usize = 0x0028;
/// `u32` preceding the inventory-slot array. Left zero: nothing is known to read it.
pub const OFF_ITEM_SLOT_COUNT: usize = 0x0030;
/// 340 × 32 B `0x808016E0` - `{u16 content_index @+0, u64 item_instance_soid @+8, 3 × u32}`.
pub const OFF_ITEM_SLOTS: usize = 0x0038;
pub const ITEM_SLOTS: usize = 340;
pub const ITEM_SLOT_STRIDE: usize = 32;
/// `0xFFFF` is "empty" everywhere in this client (`0x1056640: cmp word[rsi],0xFFFF; setne`).
/// `0x0000` is a valid index and is never "absent".
pub const EMPTY_INDEX: u16 = 0xFFFF;
/// 20 × `u64` `0x808045DA` - the equipped item-instance SOIDs the stat aggregator iterates.
pub const OFF_EQUIPPED: usize = 0x2B80;
pub const EQUIPPED_SLOTS: usize = 20;
/// `u32` serialised as **7 bits** - and it *gates* how many progression entries are encoded.
pub const OFF_PROGRESSION_COUNT: usize = 0x3660;
/// 127 × 16 B `0x8080176B` - `{u32 progress, u32 daily, u32 weekly, u16 index}`.
pub const OFF_PROGRESSIONS: usize = 0x3664;
pub const PROGRESSION_SLOTS: usize = 127;
pub const PROGRESSION_STRIDE: usize = 16;
/// `0x808018BB`, 404 bytes - the character stat block.
pub const OFF_STAT_BLOCK: usize = 0x6CD0;
/// `bool` - when set, `0x265EF0` returns an override verbatim and **never touches equipped gear**.
pub const OFF_STAT_OVERRIDE_PRESENT: usize = 0x6CDC;
/// ⭐ 7 × 8 B `{u8 stat_index @+0, u32 value @+4}`. **`+0x6CE0`, not `+0x6CE8`** - see the module docs.
pub const OFF_STAT_OVERRIDE: usize = 0x6CE0;
pub const STAT_OVERRIDE_SLOTS: usize = 7;
pub const STAT_OVERRIDE_STRIDE: usize = 8;
/// The *second*, identical override block. Writing a 7th entry at `+0x6CE8` lands here and the
/// codec refuses - which is the negative control in [`controls`].
pub const OFF_STAT_OVERRIDE_BLOCK_2: usize = 0x6D18;

/// ⭐ `0x808018BB` field `[7]` @`+0x17C` - a `kind 0x102` bool, i.e. **`+0x6CD0 + 0x17C = 0x6E4C`**.
///
/// **The investment "everything is unlocked" override.** 28 sites read this byte and every one of
/// them takes it as *"stop evaluating requirements, the answer is yes"*:
///
/// * `0x7A7420` (ladder rung 33) - `if (ctx->vfunc[0x58]() && ctx->vfunc[0x10]()[0x6E4C]) goto
///   Starting world-transition`, jumping **both** `activity_failed_to_load_missing_dlc` (`0x17`)
///   and `activity_failed_to_load_missing_permissions` (`0x18`).
/// * `0x546B30` - returns `0xFFFFFFFF` ("no unmet requirement") before it builds either requirement
///   iterator.
/// * `0x264620` (under `0x546E70`, the permissions predicate) - feeds the flag straight into
///   `0x267DB0`'s availability answer.
/// * `0x543CC0` / `0x543CB0` / `0x2671A0` / … - the `c_investment_*_requirements_client_iterator`
///   advance functions stop filtering.
///
/// ⛔ It does **not** touch offer-key ownership. `0x59F250` (`is offer key owned?`) and therefore
/// `cui::c_orbit_data_component`'s trial flag and `0x54F150`'s store upsell are unaffected - see
/// `historicaldocs/design/entitlement-and-content-rights.md`.
pub const OFF_ALL_CONTENT_OVERRIDE: usize = 0x6E4C;
/// `0x808018BB` field `[8]` @`+0x17D` - the sibling bool at **`+0x6E4D`**, read by `0x284E60`,
/// `0x285510` and `0x285E00` (the item/gear availability walk) with the same "answer yes" polarity.
/// Written by the same flag because the two are one feature; `[I]` that they are separable.
pub const OFF_ALL_CONTENT_OVERRIDE_ITEMS: usize = 0x6E4D;

/// Field `[11]`, the dense **completion flags** - one byte per element, `2` = set. This is the
/// home of unlock-flag storage kind 3: `0x28D000` reads a kind-3 flag at `profile + this + sub`
/// (`vfunc[0x90]`), where `sub` is the flag's row `+6` in the master unlock table.
pub const OFF_COMPLETIONS: usize = 0x52C0;
/// 01.29's `0x80804C21` array length.
pub const COMPLETION_SLOTS: usize = 2048;
/// The value the unlock evaluator `0x28CFB0` compares with (`cmp byte [..], 2 / sete`).
pub const COMPLETION_SET: u8 = 2;

/// The offsets that differ between builds. The `OFF_*` constants above are 01.29's, with their
/// derivations. [`offs`] picks this build's set by image hash, and [`controls`] checks the chosen
/// set against [`derive_layout`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Offs {
    pub struct_size: usize,
    pub off_stat_block: usize,
    pub off_stat_override_present: usize,
    pub off_stat_override: usize,
    pub off_stat_override_block_2: usize,
    pub off_all_content_override: usize,
    pub off_all_content_override_items: usize,
    pub off_completions: usize,
    pub completion_slots: usize,
    /// ⭐ The 64 pending-loot slots (`0x80801829`, 56 B each): field `[10]` `0x80801701` `+0x8`,
    /// `0x80804671`. `None` where unmeasured.
    pub off_pending_loot: Option<usize>,
}

/// Pending-loot slots and their stride.
pub const PENDING_LOOT_SLOTS: usize = 64;
pub const PENDING_LOOT_STRIDE: usize = 0x38;

/// ⭐ A pending world-loot record (`0x80801829`): an item the character is owed and will receive
/// when it picks up the matching world drop. The host announces the drop with incident 1504 (a
/// `0x80800CB6` body carrying the same nonce); walking onto it sends a service-601 claim naming
/// `source_kind` and the nonce. Layout: `+0` i16 content index, `+4` u32 quantity (1), `+8`
/// 4-bit source kind (3 = a world spawn), `+0x10` `{u32 key, u64 id}` nonce, `+0x20` u32 (1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingLoot {
    pub content_index: u16,
    pub source_kind: u8,
    pub key: u32,
    pub nonce: u64,
}

pub const OFFS_0129: Offs = Offs {
    struct_size: STRUCT_SIZE,
    off_stat_block: OFF_STAT_BLOCK,
    off_stat_override_present: OFF_STAT_OVERRIDE_PRESENT,
    off_stat_override: OFF_STAT_OVERRIDE,
    off_stat_override_block_2: OFF_STAT_OVERRIDE_BLOCK_2,
    off_all_content_override: OFF_ALL_CONTENT_OVERRIDE,
    off_all_content_override_items: OFF_ALL_CONTENT_OVERRIDE_ITEMS,
    off_completions: OFF_COMPLETIONS,
    completion_slots: COMPLETION_SLOTS,
    off_pending_loot: None,
};

/// How far 56578 moved everything from body field `[12]` onward.
///
/// Field `[10]` `0x80801701` grew `0x100` (5,168 → 5,424 B). Field `[11]`, the dense completion
/// flags, grew `0x200`: 01.29 `0x80804C21` (2,048 elements) is replaced by `0x8080165B` (2,560
/// elements of the same `kind 0x103` 2-bit leaf), and `0x80804C21` no longer exists in 56578.
/// Fields `[0]`..`[9]` (identity, customization, item slots, equipment, progressions) are unchanged.
pub const SHIFT_56578: usize = 0x300;

/// Build 56578 (01.33). Struct `0x7378` = 29,560 B.
pub const OFFS_56578: Offs = Offs {
    struct_size: 29_560,
    off_stat_block: OFF_STAT_BLOCK + SHIFT_56578,
    off_stat_override_present: OFF_STAT_OVERRIDE_PRESENT + SHIFT_56578,
    off_stat_override: OFF_STAT_OVERRIDE + SHIFT_56578,
    off_stat_override_block_2: OFF_STAT_OVERRIDE_BLOCK_2 + SHIFT_56578,
    off_all_content_override: OFF_ALL_CONTENT_OVERRIDE + SHIFT_56578,
    off_all_content_override_items: OFF_ALL_CONTENT_OVERRIDE_ITEMS + SHIFT_56578,
    // Field [10] grew 0x100, so [11] starts 0x100 later; [11] itself is 2,560 elements
    // (`0x8080165B`). `0x28D000` reads kind 3 at `+0x53C0 + sub` on this build.
    off_completions: OFF_COMPLETIONS + 0x100,
    completion_slots: 2560,
    off_pending_loot: Some(0x3E90 + 0x8),
};

/// The offset set for an image, by its SHA-256.
pub fn offs_for(sha256: &str) -> Option<Offs> {
    match sha256 {
        "7271fcb926401df8defb126cb8eb2b247134138b81e401bbacb2ab60791b7954" => Some(OFFS_0129),
        "672f03411c0503dfbcda804cbb9fb27cdd9a8d73a8bcb71bf0c4f688073b0833" => Some(OFFS_56578),
        _ => None,
    }
}

/// The selected build's offsets. Panics when the image has no measured set: a profile built at
/// another build's offsets decodes into the wrong fields with no error.
pub fn offs() -> Offs {
    static O: std::sync::OnceLock<std::result::Result<Offs, String>> = std::sync::OnceLock::new();
    O.get_or_init(|| {
        let image = Image::open_default().map_err(|e| e.to_string())?;
        offs_for(&image.sha256())
            .ok_or_else(|| format!("no profile offset set for image sha256 {}", image.sha256()))
    })
    .clone()
    .unwrap_or_else(|e| panic!("{e}"))
}

/// ⭐ The Light stat index is **0**. `THE-EMPTY-PROFILE.md` §4: `0x281F50` replayed on
/// `runs/snapshots/orbit-all.snap` returns `rax & 0xFF == 0x00`, with `0x276220` → `0x28` = the
/// level cap 40 as the positive control and `0x281F70` → `0x01` as the discriminator.
/// `[O]`, snapshot replay - not `observed-live`.
pub const LIGHT_STAT_INDEX: u8 = 0;

/// ⭐ The **Rise of Iron Light cap: 385.** This build is 01.29 (RoI-era); no legitimate Guardian
/// ever reads above it. ⛔ Every fixture before 2026-08-10 shipped **400** - a value that is
/// IMPOSSIBLE in the real game, which makes the fixture provably invented on sight and taints any
/// screenshot made with it. `[R]` era knowledge, not a client measurement - the client-side cap
/// (if one is enforced at all) is `[U]`; this constant exists so the DEFAULT is a value that could
/// have come from a real service.
pub const LIGHT_ROI_CAP: u32 = 385;

/// The SOID `d1-responder` keys the profile by (`d1_oracle::PROFILE_SOID`).
pub const DEFAULT_SOID: u64 = crate::PROFILE_SOID;
/// Old Russia. The client compares this field against `0x811C9DC5` (the FNV-1a basis = the hash of
/// the empty string = "unset") at `0x752AE0`; a real value here is what makes orbit render.
pub const DESTINATION_OLD_RUSSIA: u32 = 0x9320_15BB;
/// `{race, gender, class}` as carried by every fixture in `capture-rig/characters/`
/// (decoded `+0x08 = 0x10000`).
pub const DEFAULT_IDENTITY: [u8; 3] = [0, 0, 1];
/// `d1-responder`'s `D1_ITEM_SOID_BASE` default; staged elements are `base + i`.
pub const ITEM_SOID_BASE: u64 = 0xD100_0000_0000_0301;

/// One entry of the character progression table at `+0x3664`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progression {
    /// Raw progress. **The client derives the level by walking the definition's steps** - it never
    /// stores a level.
    pub progress: u32,
    /// Periodic bucket A ("daily"); zeroed by `0x265C20`.
    pub daily: u32,
    /// Periodic bucket B ("weekly"); zeroed by `0x265D80`.
    pub weekly: u32,
    /// The `u16` runtime ordinal. `[U]` for `character_level` - see the module docs.
    pub index: u16,
}

/// One slot of the 340-entry inventory array at `+0x38`.
///
/// ⭐ **This is the array the client actually walks to draw gear.** `0x1087660` reads the `u16` at
/// element `+0x00` and the SOID at element `+0x08` for every one of the 340 slots, accepts the slot
/// iff the `u16` is not `0xFFFF` (`0x1056640`), resolves the SOID in family 4/3, and hands the
/// `u16` to `0x555730`, which turns it into a world tag via `0x54C090` and asks the resourcerer to
/// load it. A profile whose 340 slots are all `0xFFFF` has an empty Guardian by construction -
/// `historicaldocs/design/item-render-path.md` §0 link 3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemSlot {
    /// Element `+0x00`. The **content index**, measured - never invented.
    pub content_index: u16,
    /// Element `+0x08`. Must resolve in family 4/3 or the walker passes `NULL` for the element.
    pub item_soid: u64,
    /// ⭐⭐ **WHICH of the 340 slots this item occupies - and it is the ONLY thing that decides
    /// which TAB the item appears under.**
    ///
    /// `cui::c_investment_backpack_item::ctor` (`0x9F2CC0`) at `0x9F2D60` passes the **slot index**
    /// and the scope to `FUN_0026DB60`, which returns the bucket id from a 39-record content table.
    /// **The item is not an input.** Writing item *n* at slot *n* - which every builder before
    /// 2026-08-10 did - puts all of them in slots 0-19, i.e. **Materials**.
    ///
    /// Resolve it with [`crate::inventory::slot_for_bucket`], which returns `None` rather than
    /// inventing a range for a bucket nobody has read.
    pub slot: u16,
}

/// Everything a populated profile needs, and nothing that is a guess.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileSpec {
    pub soid: u64,
    /// `{race, gender, class}` - must agree with the family-3 character object.
    pub identity: [u8; 3],
    pub last_orbited_destination: u32,
    /// Up to [`STAT_OVERRIDE_SLOTS`] `{stat_index, value}` pairs. Non-empty sets `+0x6CDC`.
    ///
    /// ⚠ `0x265EF0` returns the **first** entry whose key matches, then stops. Unused entries stay
    /// `{0, 0}`, which is unreachable for index 0 (entry 0 matches first) and inert for every other
    /// index, because a miss falls through to the ordinary gear summation.
    pub stat_overrides: Vec<(u8, u32)>,
    /// Up to [`EQUIPPED_SLOTS`] item-instance SOIDs. Each must resolve in family 4/3 -
    /// [`inventory_soids`] reads them out of the staged elements so they cannot drift.
    pub equipped: Vec<u64>,
    /// Up to [`PROGRESSION_SLOTS`] entries. The count word at `+0x3660` gates serialisation, so an
    /// empty vector leaves the whole 2 KB table off the wire.
    pub progressions: Vec<Progression>,
    /// ⭐ The 340-slot inventory array at `+0x38`, in slot order. Written **over** the `0xFFFF`
    /// fill, so a short list still leaves the rest explicitly empty.
    pub item_slots: Vec<ItemSlot>,
    /// Mark all 340 inventory slots empty (`0xFFFF`) instead of leaving them zero.
    ///
    /// ⭐ Zero is **not** empty: `0x1056640` tests `!= 0xFFFF`, so a zero-filled profile claims the
    /// Guardian owns 340 copies of content index 0. Every profile this project shipped before
    /// 2026-08-08 said exactly that. Set `false` to reproduce the old shape when bisecting.
    pub empty_item_slots: bool,
    /// Set [`OFF_ALL_CONTENT_OVERRIDE`] and [`OFF_ALL_CONTENT_OVERRIDE_ITEMS`] - the client's own
    /// "every investment requirement is satisfied" bit.
    ///
    /// **Default `false`, deliberately.** It is a behaviour switch inside the client, so a run made
    /// with it on is a *different experiment* from one made without: it suppresses the two context
    /// strings (`"World-transition requires DLC."` / `"…requires permissions."`) that are the only
    /// evidence rung 33 ever produces about content gating. Turn it on to answer *"is content
    /// gating what is stopping us?"*, and turn it back off to keep the diagnostic.
    pub all_content: bool,
    /// ⭐ Field `[11]` completion-flag subindices to set (unlock-flag storage kind 3). Each is the
    /// `sub` of a kind-3 row in the master unlock table, NOT the flag's index - the index space and
    /// the storage space differ. A kind-3 flag served here reaches the client's derived plane the
    /// way a real account's completions did; it is how the Director's destination gates are served.
    pub completions: Vec<u16>,
    /// Pending world-loot records (see [`PendingLoot`]). Non-empty marks every other slot empty.
    pub pending_loot: Vec<PendingLoot>,
}

impl Default for ProfileSpec {
    /// The shipped profile: identity, Old Russia, Light 400 via the override, the staged equipment
    /// if it is present, empty inventory slots, no progressions.
    fn default() -> Self {
        ProfileSpec {
            soid: DEFAULT_SOID,
            identity: DEFAULT_IDENTITY,
            last_orbited_destination: DESTINATION_OLD_RUSSIA,
            stat_overrides: vec![(LIGHT_STAT_INDEX, LIGHT_ROI_CAP)],
            equipped: Vec::new(),
            progressions: Vec::new(),
            item_slots: Vec::new(),
            empty_item_slots: true,
            all_content: false,
            completions: Vec::new(),
            pending_loot: Vec::new(),
        }
    }
}

impl ProfileSpec {
    /// Lay the spec out as the decoded C struct of the selected build (28,792 B on 01.29, 29,560 B
    /// on 56578).
    ///
    /// This never touches the wire - [`build`] hands the result to the client's own encoder, which
    /// is the only thing allowed to turn it into bytes.
    pub fn to_object(&self) -> Result<Vec<u8>> {
        self.to_object_for(offs())
    }

    /// [`ProfileSpec::to_object`] at an explicit build's offsets. Needs no image.
    pub fn to_object_for(&self, o: Offs) -> Result<Vec<u8>> {
        let Offs { off_stat_override_present, off_stat_override, off_all_content_override_items, off_all_content_override, struct_size, off_completions, completion_slots, off_pending_loot, .. } = o;
        let too_many = |what: &str, got: usize, cap: usize| {
            Err(OracleError::Shape(format!(
                "{got} {what} exceeds the schema's {cap} slots - the extra entries would land in \
                 the NEXT field and the codec would silently reshape them"
            )))
        };
        if self.stat_overrides.len() > STAT_OVERRIDE_SLOTS {
            return too_many("stat overrides", self.stat_overrides.len(), STAT_OVERRIDE_SLOTS);
        }
        if self.equipped.len() > EQUIPPED_SLOTS {
            return too_many("equipped items", self.equipped.len(), EQUIPPED_SLOTS);
        }
        if self.progressions.len() > PROGRESSION_SLOTS {
            return too_many("progressions", self.progressions.len(), PROGRESSION_SLOTS);
        }
        if self.item_slots.len() > ITEM_SLOTS {
            return too_many("inventory slots", self.item_slots.len(), ITEM_SLOTS);
        }

        let mut o = vec![0u8; struct_size];
        o[OFF_SOID..OFF_SOID + 8].copy_from_slice(&self.soid.to_le_bytes());
        o[OFF_IDENTITY..OFF_IDENTITY + 3].copy_from_slice(&self.identity);
        o[OFF_LAST_ORBITED_DESTINATION..OFF_LAST_ORBITED_DESTINATION + 4]
            .copy_from_slice(&self.last_orbited_destination.to_le_bytes());

        if self.empty_item_slots {
            for i in 0..ITEM_SLOTS {
                let at = OFF_ITEM_SLOTS + i * ITEM_SLOT_STRIDE;
                o[at..at + 2].copy_from_slice(&EMPTY_INDEX.to_le_bytes());
            }
        }

        if !self.pending_loot.is_empty() {
            let Some(base) = off_pending_loot else {
                return Err(OracleError::Shape("pending loot: no measured offset on this build".into()));
            };
            if self.pending_loot.len() > PENDING_LOOT_SLOTS {
                return too_many("pending loot records", self.pending_loot.len(), PENDING_LOOT_SLOTS);
            }
            for k in 0..PENDING_LOOT_SLOTS {
                let at = base + k * PENDING_LOOT_STRIDE;
                match self.pending_loot.get(k) {
                    // Zero is item 0, not empty (as for the item slots): mark unused slots -1.
                    None => o[at..at + 2].copy_from_slice(&EMPTY_INDEX.to_le_bytes()),
                    Some(p) => {
                        o[at..at + 2].copy_from_slice(&p.content_index.to_le_bytes());
                        o[at + 4..at + 8].copy_from_slice(&1u32.to_le_bytes());
                        o[at + 8] = p.source_kind;
                        o[at + 0x10..at + 0x14].copy_from_slice(&p.key.to_le_bytes());
                        o[at + 0x18..at + 0x20].copy_from_slice(&p.nonce.to_le_bytes());
                        o[at + 0x20..at + 0x24].copy_from_slice(&1u32.to_le_bytes());
                    }
                }
            }
        }

        for it in self.item_slots.iter() {
            let at = OFF_ITEM_SLOTS + it.slot as usize * ITEM_SLOT_STRIDE;
            o[at..at + 2].copy_from_slice(&it.content_index.to_le_bytes());
            o[at + 8..at + 16].copy_from_slice(&it.item_soid.to_le_bytes());
        }

        for (i, soid) in self.equipped.iter().enumerate() {
            let at = OFF_EQUIPPED + i * 8;
            o[at..at + 8].copy_from_slice(&soid.to_le_bytes());
        }

        let n = self.progressions.len() as u32;
        o[OFF_PROGRESSION_COUNT..OFF_PROGRESSION_COUNT + 4].copy_from_slice(&n.to_le_bytes());
        for (i, p) in self.progressions.iter().enumerate() {
            let at = OFF_PROGRESSIONS + i * PROGRESSION_STRIDE;
            o[at..at + 4].copy_from_slice(&p.progress.to_le_bytes());
            o[at + 4..at + 8].copy_from_slice(&p.daily.to_le_bytes());
            o[at + 8..at + 12].copy_from_slice(&p.weekly.to_le_bytes());
            o[at + 12..at + 14].copy_from_slice(&p.index.to_le_bytes());
        }

        if self.all_content {
            o[off_all_content_override] = 1;
            o[off_all_content_override_items] = 1;
        }

        for &sub in &self.completions {
            if sub as usize >= completion_slots {
                return Err(OracleError::Shape(format!(
                    "completion subindex {sub} is past field [11]'s {completion_slots} elements")));
            }
            o[off_completions + sub as usize] = COMPLETION_SET;
        }

        if !self.stat_overrides.is_empty() {
            o[off_stat_override_present] = 1;
            for (i, (key, value)) in self.stat_overrides.iter().enumerate() {
                let at = off_stat_override + i * STAT_OVERRIDE_STRIDE;
                o[at] = *key;
                o[at + 4..at + 8].copy_from_slice(&value.to_le_bytes());
            }
        }
        Ok(o)
    }

    /// The `(offset, value)` pairs this spec *claims* it wrote, as qwords, for the round-trip check.
    fn claimed_qwords(&self) -> Vec<(usize, &'static str)> {
        let Offs { off_stat_override_present, off_stat_override, off_all_content_override, .. } = offs();
        let mut v = vec![(OFF_SOID, "SOID"), (OFF_IDENTITY, "identity {race,gender,class}")];
        // +0x28 shares a qword with +0x2C; it is asserted as a u32 by `verify`, not here.
        if !self.equipped.is_empty() {
            v.push((OFF_EQUIPPED, "equipped[0] item-instance SOID"));
        }
        if !self.progressions.is_empty() {
            v.push((OFF_PROGRESSIONS, "progression[0]"));
        }
        if !self.stat_overrides.is_empty() {
            v.push((off_stat_override_present & !7, "stat-override present flag"));
            v.push((off_stat_override, "stat-override[0] {index,value}"));
        }
        if self.all_content {
            v.push((off_all_content_override & !7, "all-content override bools"));
        }
        v
    }
}

/// A built profile: the wire payload plus the decode that proves it.
#[derive(Debug, Clone)]
pub struct BuiltProfile {
    pub payload: Vec<u8>,
    pub bits: u32,
    pub decoded: Decoded,
}

impl BuiltProfile {
    /// Every non-zero qword of the decode, `(offset, value)` - the shape check people skip.
    pub fn nonzero(&self) -> Vec<(usize, u64)> {
        self.decoded.nonzero_qwords()
    }
}

/// Encode a spec through the **client's own encoder**, then decode it back and require the C object
/// to be identical.
///
/// The round trip is the proof: [`ObjectOracle::round_trip`] refuses on the first differing byte, so
/// a value written into a byte no field covers is an error rather than a payload.
pub fn build(oracle: &mut ObjectOracle, spec: &ProfileSpec) -> Result<BuiltProfile> {
    if oracle.tag() != TAG {
        return Err(OracleError::Shape(format!(
            "oracle is for tag {:#010x}, not the profile tag {TAG:#010x}",
            oracle.tag()
        )));
    }
    let object = spec.to_object()?;
    let (enc, dec) = oracle.round_trip(&object, 0x9000)?;
    verify(spec, &dec)?;
    Ok(BuiltProfile { payload: enc.payload, bits: enc.bits, decoded: dec })
}

/// Assert that the decode carries every field the spec asked for, **at the offset it asked for**.
///
/// ⚠ `round_trip` already proves the whole struct survived; this exists so a *silently empty* spec
/// (all defaults dropped, an empty vector) cannot pass as a populated profile.
pub fn verify(spec: &ProfileSpec, dec: &Decoded) -> Result<()> {
    let Offs { off_stat_override_present, off_stat_override, off_all_content_override_items, off_all_content_override, struct_size, off_completions, completion_slots, .. } = offs();
    let u32_at = |off: usize| -> u32 {
        u32::from_le_bytes(dec.object[off..off + 4].try_into().unwrap_or([0; 4]))
    };
    let bad = |m: String| Err(OracleError::Shape(m));

    if dec.object.len() != struct_size {
        return bad(format!("decoded {} bytes, expected {struct_size}", dec.object.len()));
    }
    if dec.qword(OFF_SOID) != spec.soid {
        return bad(format!("+{OFF_SOID:#06x} SOID is {:#x}, expected {:#x}", dec.qword(OFF_SOID), spec.soid));
    }
    if dec.object[OFF_IDENTITY..OFF_IDENTITY + 3] != spec.identity {
        return bad(format!("+{OFF_IDENTITY:#06x} identity is {:?}, expected {:?}",
                           &dec.object[OFF_IDENTITY..OFF_IDENTITY + 3], spec.identity));
    }
    if u32_at(OFF_LAST_ORBITED_DESTINATION) != spec.last_orbited_destination {
        return bad(format!(
            "+{OFF_LAST_ORBITED_DESTINATION:#06x} last_orbited_destination is {:#010x}, expected \
             {:#010x} - orbit depends on this field",
            u32_at(OFF_LAST_ORBITED_DESTINATION), spec.last_orbited_destination));
    }
    for it in spec.item_slots.iter() {
        let i = it.slot as usize;
        let at = OFF_ITEM_SLOTS + i * ITEM_SLOT_STRIDE;
        let got = u16::from_le_bytes(dec.object[at..at + 2].try_into().unwrap());
        if got != it.content_index || dec.qword(at + 8) != it.item_soid {
            return bad(format!(
                "item slot {i} @+{at:#06x} is {{index {got:#06x}, soid {:#x}}}, expected \
                 {{index {:#06x}, soid {:#x}}}",
                dec.qword(at + 8), it.content_index, it.item_soid));
        }
    }
    if spec.empty_item_slots {
        let used: std::collections::BTreeSet<usize> =
            spec.item_slots.iter().map(|it| it.slot as usize).collect();
        for i in (0..ITEM_SLOTS).filter(|i| !used.contains(i)) {
            let at = OFF_ITEM_SLOTS + i * ITEM_SLOT_STRIDE;
            let got = u16::from_le_bytes(dec.object[at..at + 2].try_into().unwrap());
            if got != EMPTY_INDEX {
                return bad(format!("item slot {i} @+{at:#06x} is {got:#06x}, expected {EMPTY_INDEX:#06x}"));
            }
        }
    }
    // Both directions: every requested completion reads 2 back, and nothing else in field [11] is
    // set - a shifted offset would decode cleanly and set the wrong flags.
    let want: std::collections::BTreeSet<usize> = spec.completions.iter().map(|&s| s as usize).collect();
    for i in 0..completion_slots {
        let got = dec.object[off_completions + i];
        let expect = if want.contains(&i) { COMPLETION_SET } else { 0 };
        if got != expect {
            return bad(format!("completion [{i}] @+{:#06x} is {got}, expected {expect}", off_completions + i));
        }
    }
    for (i, soid) in spec.equipped.iter().enumerate() {
        let at = OFF_EQUIPPED + i * 8;
        if dec.qword(at) != *soid {
            return bad(format!("equipped[{i}] @+{at:#06x} is {:#x}, expected {soid:#x}", dec.qword(at)));
        }
    }
    if u32_at(OFF_PROGRESSION_COUNT) != spec.progressions.len() as u32 {
        return bad(format!("+{OFF_PROGRESSION_COUNT:#06x} progression count is {}, expected {}",
                           u32_at(OFF_PROGRESSION_COUNT), spec.progressions.len()));
    }
    for (i, p) in spec.progressions.iter().enumerate() {
        let at = OFF_PROGRESSIONS + i * PROGRESSION_STRIDE;
        let idx = u16::from_le_bytes(dec.object[at + 12..at + 14].try_into().unwrap());
        if u32_at(at) != p.progress || idx != p.index {
            return bad(format!(
                "progression[{i}] @+{at:#06x} is {{progress {}, index {idx:#06x}}}, expected \
                 {{progress {}, index {:#06x}}}", u32_at(at), p.progress, p.index));
        }
    }
    let present = dec.object[off_stat_override_present];
    let want_present = u8::from(!spec.stat_overrides.is_empty());
    if present != want_present {
        return bad(format!(
            "+{off_stat_override_present:#06x} override-present is {present}, expected {want_present} \
             - without it 0x265EF0 skips the table entirely and sums over gear"));
    }
    for (i, (key, value)) in spec.stat_overrides.iter().enumerate() {
        let at = off_stat_override + i * STAT_OVERRIDE_STRIDE;
        if dec.object[at] != *key || u32_at(at + 4) != *value {
            return bad(format!(
                "stat override[{i}] @+{at:#06x} is {{index {}, value {}}}, expected {{index {key}, \
                 value {value}}}", dec.object[at], u32_at(at + 4)));
        }
    }
    // ⚠ Two bits, asserted in BOTH directions: an off-by-one that landed the flag on a neighbouring
    // field would still decode, and would still look like a populated profile.
    let want = u8::from(spec.all_content);
    for (off, what) in [
        (off_all_content_override, "all-content override"),
        (off_all_content_override_items, "all-content override (items)"),
    ] {
        if dec.object[off] != want {
            return bad(format!(
                "+{off:#06x} {what} is {}, expected {want}", dec.object[off]));
        }
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Layout, re-derived from the eboot rather than transcribed
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// One array container's shape, read from its **descriptor header** and not from the offset column
/// that `d1-eboot descriptors` prints for a `kind 0x1` field (that column is the element stride).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArrayShape {
    pub tag: u32,
    /// Absolute C offset of element 0 = the offset of the *parent* field pointing at the container.
    pub base: usize,
    pub count: u32,
    pub elem_size: u32,
}

impl ArrayShape {
    fn check(&self, container: &Descriptor) -> Result<()> {
        if u64::from(self.count) * u64::from(self.elem_size) != u64::from(container.struct_size) {
            return Err(OracleError::Shape(format!(
                "{:#010x}: count {} × element {} != struct size {} - the array does not tile its \
                 container, so one of the three numbers is misread",
                self.tag, self.count, self.elem_size, container.struct_size
            )));
        }
        Ok(())
    }
}

/// The whole profile layout, derived from the descriptor tree in this eboot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub struct_size: u32,
    pub soid: usize,
    pub identity: usize,
    pub customization: usize,
    pub last_orbited_destination: usize,
    pub item_slots: ArrayShape,
    pub equipped: ArrayShape,
    pub progression_count: usize,
    pub progressions: ArrayShape,
    pub stat_block: usize,
    pub stat_override_present: usize,
    pub stat_overrides: ArrayShape,
    /// `0x808018BB` field `[7]`, the investment all-content override bool.
    pub all_content_override: usize,
    /// `0x808018BB` field `[8]`, its item-side sibling.
    pub all_content_override_items: usize,
}

fn eb(e: impl std::fmt::Display) -> OracleError {
    OracleError::Eboot(e.to_string())
}

fn desc<'a>(idx: &'a Index, tag: u32) -> Result<&'a Descriptor> {
    idx.get(tag).ok_or(OracleError::UnknownTag(tag))
}

/// Field `i` of `tag`, or a typed error naming what was being looked for.
fn field(image: &Image, idx: &Index, tag: u32, i: usize) -> Result<Field> {
    let d = desc(idx, tag)?;
    descriptors::fields(image, d.anchor, d.field_count)
        .get(i)
        .and_then(|(_, f)| *f)
        .ok_or_else(|| OracleError::Shape(format!("{tag:#010x} has no field [{i}]")))
}

/// A composite field's child tag, checked.
fn child(image: &Image, idx: &Index, tag: u32, i: usize) -> Result<(Field, u32)> {
    let f = field(image, idx, tag, i)?;
    if f.child_tag == LEAF {
        return Err(OracleError::Shape(format!("{tag:#010x} field [{i}] is a leaf, expected a struct")));
    }
    Ok((f, f.child_tag))
}

/// The container's own element count and element size.
///
/// ⚠ Read from `anchor+0x20` / `anchor+0x28` - i.e. record `+0x10` / `+0x18`. The field record's
/// offset column carries the **element stride** for `kind 0x1`, and reading it as a C offset is
/// exactly the eight-byte error this module documents.
fn array_at(image: &Image, idx: &Index, tag: u32, base: usize) -> Result<ArrayShape> {
    let d = desc(idx, tag)?;
    let count = image.u32(d.anchor + 0x20).map_err(eb)?;
    let elem_size = image.u32(d.anchor + 0x28).map_err(eb)?;
    let shape = ArrayShape { tag, base, count, elem_size };
    shape.check(d)?;
    Ok(shape)
}

/// Walk `0x80801743`'s tree and produce the offsets this module writes to.
///
/// **This is the control on every constant above.** Nothing here is transcribed from a document;
/// if the eboot disagrees, [`controls`] fails and exits 2.
pub fn derive_layout(image: &Image, idx: &Index) -> Result<Layout> {
    // 0x80801743 is a one-field wrapper over 0x80801714 at offset 0.
    let (wrap, inner_tag) = child(image, idx, TAG, 0)?;
    if wrap.offset != 0 {
        return Err(OracleError::Shape(format!("{TAG:#010x} field [0] is at +{:#x}, expected 0", wrap.offset)));
    }
    let inner = desc(idx, inner_tag)?.clone();

    let soid = field(image, idx, inner_tag, 0)?.offset as usize;
    let identity = child(image, idx, inner_tag, 1)?.0.offset as usize;

    let (cust_f, cust_tag) = child(image, idx, inner_tag, 2)?;
    let customization = cust_f.offset as usize;
    let dest = customization + field(image, idx, cust_tag, 10)?.offset as usize;

    // [3] +0x30 -> 0x80800F9F {u32 count @+0, 0x80800FA1 @+8}; the 340-slot array is 0x80800FA1.
    let (inv_f, inv_tag) = child(image, idx, inner_tag, 3)?;
    let item_slot_count = inv_f.offset as usize + field(image, idx, inv_tag, 0)?.offset as usize;
    if item_slot_count != OFF_ITEM_SLOT_COUNT {
        return Err(OracleError::Shape(format!(
            "inventory count word is at +{item_slot_count:#x}, code says +{OFF_ITEM_SLOT_COUNT:#x}"
        )));
    }
    let (slots_f, slots_tag) = child(image, idx, inv_tag, 1)?;
    let item_slots = array_at(image, idx, slots_tag, inv_f.offset as usize + slots_f.offset as usize)?;

    let (eq_f, eq_tag) = child(image, idx, inner_tag, 5)?;
    let equipped = array_at(image, idx, eq_tag, eq_f.offset as usize)?;

    // [8] +0x3660 -> 0x808047A7 {u32 count(7 bits) @+0, 0x80804745 @+4}.
    let (prog_f, prog_tag) = child(image, idx, inner_tag, 8)?;
    let progression_count = prog_f.offset as usize + field(image, idx, prog_tag, 0)?.offset as usize;
    let (ptab_f, ptab_tag) = child(image, idx, prog_tag, 1)?;
    let progressions = array_at(image, idx, ptab_tag, prog_f.offset as usize + ptab_f.offset as usize)?;

    // [15] +0x6CD0 -> 0x808018BB; [3] +0x0C -> 0x80801912 {bool present @+0, 0x80804768 @+4}.
    let (stat_f, stat_tag) = child(image, idx, inner_tag, 15)?;
    let (ovr_f, ovr_tag) = child(image, idx, stat_tag, 3)?;
    let ovr_base = stat_f.offset as usize + ovr_f.offset as usize;
    let present_f = field(image, idx, ovr_tag, 0)?;
    if present_f.kind != 0x102 {
        return Err(OracleError::Shape(format!(
            "{ovr_tag:#010x} field [0] is kind {:#x}, expected the 0x102 bool `present`", present_f.kind)));
    }
    let (tab_f, tab_tag) = child(image, idx, ovr_tag, 1)?;
    let stat_overrides = array_at(image, idx, tab_tag, ovr_base + tab_f.offset as usize)?;

    // [15] +0x6CD0 -> 0x808018BB fields [7] and [8]: two adjacent `kind 0x102` bools. The kind check
    // is what makes an off-by-one in the field index a refusal rather than a plausible offset.
    let mut overrides = [0usize; 2];
    for (slot, i) in [(0usize, 7usize), (1, 8)] {
        let f = field(image, idx, stat_tag, i)?;
        if f.kind != 0x102 {
            return Err(OracleError::Shape(format!(
                "{stat_tag:#010x} field [{i}] is kind {:#x}, expected the 0x102 bool", f.kind)));
        }
        overrides[slot] = stat_f.offset as usize + f.offset as usize;
    }

    Ok(Layout {
        struct_size: inner.struct_size,
        soid,
        identity,
        customization,
        last_orbited_destination: dest,
        item_slots,
        equipped,
        progression_count,
        progressions,
        stat_block: stat_f.offset as usize,
        stat_override_present: ovr_base + present_f.offset as usize,
        stat_overrides,
        all_content_override: overrides[0],
        all_content_override_items: overrides[1],
    })
}

/// The layout this module declares for one build - what [`derive_layout`] must reproduce.
pub fn declared_layout_for(o: Offs) -> Layout {
    Layout {
        struct_size: o.struct_size as u32,
        soid: OFF_SOID,
        identity: OFF_IDENTITY,
        customization: OFF_CUSTOMIZATION,
        last_orbited_destination: OFF_LAST_ORBITED_DESTINATION,
        item_slots: ArrayShape {
            tag: 0x8080_0FA1,
            base: OFF_ITEM_SLOTS,
            count: ITEM_SLOTS as u32,
            elem_size: ITEM_SLOT_STRIDE as u32,
        },
        equipped: ArrayShape {
            tag: 0x8080_45DA,
            base: OFF_EQUIPPED,
            count: EQUIPPED_SLOTS as u32,
            elem_size: 8,
        },
        progression_count: OFF_PROGRESSION_COUNT,
        progressions: ArrayShape {
            tag: 0x8080_4745,
            base: OFF_PROGRESSIONS,
            count: PROGRESSION_SLOTS as u32,
            elem_size: PROGRESSION_STRIDE as u32,
        },
        stat_block: o.off_stat_block,
        stat_override_present: o.off_stat_override_present,
        stat_overrides: ArrayShape {
            tag: 0x8080_4768,
            base: o.off_stat_override,
            count: STAT_OVERRIDE_SLOTS as u32,
            elem_size: STAT_OVERRIDE_STRIDE as u32,
        },
        all_content_override: o.off_all_content_override,
        all_content_override_items: o.off_all_content_override_items,
    }
}

/// The layout this module declares for the selected build.
pub fn declared_layout() -> Layout {
    declared_layout_for(offs())
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Inventory
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// Read the item-instance SOIDs out of the **staged family-4/3 elements** the responder will serve.
///
/// ⭐ Deriving them beats writing `ITEM_SOID_BASE + i`: an equipped SOID that does not resolve in
/// 4/3 names nothing, and `0x265EF0` skips it silently (`test rbx,rbx; je`). Decoding each staged
/// element makes the profile and the inventory provably agree.
///
/// `dir` may be a directory of `*.bin` or a single file. Returns them in filename order, which is
/// the order `stage_loadout` writes and therefore slot order.
pub fn inventory_soids(dir: &Path) -> Result<Vec<u64>> {
    Ok(inventory_items(dir)?.into_iter().map(|i| i.soid).collect())
}

/// One staged family-4/3 element, as far as this crate needs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedItem {
    /// `C+0x00` - **the lookup key**. `0x56CA70` compares element[0] to the key it is given.
    pub soid: u64,
    /// `C+0x18` - the item's **content hash**, as `d1-forge` stages it.
    ///
    /// ⚠ This is *our* convention, not the client's: `historicaldocs/design/item-render-path.md` §3 shows nothing
    /// on the render path reads `C+0x18`. It is here because it is the only way to name which item
    /// a staged element *is* without a second file that can drift from it - and
    /// [`crate::item_index::IndexTable::require`] refuses a hash the content does not know, so a
    /// convention change fails loudly instead of producing a wrong index.
    pub item_hash: u32,
    /// `C+0x1C` - the bucket hash, same convention. Unused here; carried so a caller can report it.
    pub bucket_hash: u32,
    /// `C+0x24` - the `u16` the client's own definition lookup takes
    /// (`0x265EF0`: `movzx esi, word [rbx+0x24]` → `investment_globals->vtbl[0x410]`).
    pub content_index_field: u16,
}

/// The staged elements, decoded - SOID **and** the item hash that names each one.
///
/// ⭐ Deriving these beats writing `ITEM_SOID_BASE + i`: an equipped SOID that does not resolve in
/// 4/3 names nothing, and `0x265EF0` skips it silently (`test rbx,rbx; je`). Decoding each staged
/// element makes the profile, the card and the inventory provably agree.
pub fn inventory_items(dir: &Path) -> Result<Vec<StagedItem>> {
    let mut files: Vec<std::path::PathBuf> = if dir.is_dir() {
        std::fs::read_dir(dir)
            .map_err(|e| eb(format!("{}: {e}", dir.display())))?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().map(|x| x == "bin").unwrap_or(false))
            .collect()
    } else {
        vec![dir.to_path_buf()]
    };
    files.sort();
    if files.is_empty() {
        return Ok(Vec::new());
    }
    let mut o = ObjectOracle::new(ITEM_TAG)?;
    let mut out = Vec::new();
    for f in &files {
        let raw = std::fs::read(f).map_err(|e| eb(format!("{}: {e}", f.display())))?;
        let d = o.decode(&raw)?;
        let soid = d.qword(0);
        if soid == 0 {
            return Err(OracleError::Shape(format!(
                "{}: family-4/3 element has SOID 0 at +0x00 - nothing can be equipped to it",
                f.display()
            )));
        }
        let u32_at = |o: usize| u32::from_le_bytes(d.object[o..o + 4].try_into().unwrap());
        out.push(StagedItem {
            soid,
            item_hash: u32_at(0x18),
            bucket_hash: u32_at(0x1C),
            content_index_field: u16::from_le_bytes(d.object[0x24..0x26].try_into().unwrap()),
        });
    }
    Ok(out)
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Controls
// ─────────────────────────────────────────────────────────────────────────────────────────────────

pub struct Control {
    pub name: &'static str,
    pub pass: bool,
    pub detail: String,
}

fn ok(name: &'static str, detail: impl Into<String>) -> Control {
    Control { name, pass: true, detail: detail.into() }
}
fn no(name: &'static str, detail: impl Into<String>) -> Control {
    Control { name, pass: false, detail: detail.into() }
}

/// Every control this module ships. **Exit 2 if any `pass` is false.**
///
/// The set is chosen so that each one can fail in the direction we are worried about:
///
/// | control | the axis it tests |
/// |---|---|
/// | `layout` | a constant here drifting from the eboot |
/// | `stat-array-base` | the `+0x6CE0` vs `+0x6CE8` reading - **positive** |
/// | `stat-array-base-NEGATIVE` | ⭐ the same reading from the other side. `+0x6CE8` **must be refused**; a control that can only pass is decoration |
/// | `stat-array-full` | all 7 entries, not just the first |
/// | `equipped-base` | `+0x2B80` element 0 is a real field and not padding |
/// | `item-slot-base` | `+0x0038` element 0, ditto |
/// | `progression-count-gates` | the 7-bit count word decides how much of the 2 KB table is encoded |
/// | `baseline-is-empty` | the fixture we are replacing really does decode to two fields |
/// | `built-profile` | the shipped spec round-trips and `verify` accepts it |
/// | `strictly-more-than-baseline` | ⭐ a *monotonicity* check: the new profile's non-zero set must be a strict superset of the baseline's. Catches a rewrite that quietly drops the SOID or the destination |
pub fn controls(baseline: Option<&Path>) -> Result<Vec<Control>> {
    let Offs { off_stat_override_present, off_stat_override_block_2, off_stat_override, off_all_content_override_items, off_all_content_override, struct_size, .. } = offs();
    let mut out = Vec::new();

    // 1. Layout, re-derived.
    let image = Image::open_default().map_err(eb)?;
    let idx = descriptors::index(&image).map_err(eb)?;
    let derived = derive_layout(&image, &idx)?;
    let declared = declared_layout();
    out.push(if derived == declared {
        ok("layout", format!(
            "{} B; identity +{:#x}; destination +{:#x}; items {}×{} @+{:#x}; equipped {}×{} @+{:#x}; \
             progressions {}×{} @+{:#x} (count +{:#x}); overrides {}×{} @+{:#x} (present +{:#x})",
            derived.struct_size, derived.identity, derived.last_orbited_destination,
            derived.item_slots.count, derived.item_slots.elem_size, derived.item_slots.base,
            derived.equipped.count, derived.equipped.elem_size, derived.equipped.base,
            derived.progressions.count, derived.progressions.elem_size, derived.progressions.base,
            derived.progression_count,
            derived.stat_overrides.count, derived.stat_overrides.elem_size, derived.stat_overrides.base,
            derived.stat_override_present))
    } else {
        no("layout", format!("eboot says {derived:#?}\ncode says {declared:#?}"))
    });

    let mut oracle = ObjectOracle::new(TAG)?;
    let base = ProfileSpec {
        stat_overrides: Vec::new(),
        empty_item_slots: false,
        ..ProfileSpec::default()
    };

    // 2. The stat-override array base, positive: all 7 entries at +0x6CE0.
    let mut o = base.to_object()?;
    o[off_stat_override_present] = 1;
    for i in 0..STAT_OVERRIDE_SLOTS {
        let at = off_stat_override + i * STAT_OVERRIDE_STRIDE;
        o[at] = i as u8;
        o[at + 4..at + 8].copy_from_slice(&(100u32 * (i as u32 + 1)).to_le_bytes());
    }
    out.push(match oracle.round_trip(&o, 0x9000) {
        Ok(_) => ok("stat-array-full", format!(
            "{STAT_OVERRIDE_SLOTS} entries at +{off_stat_override:#x} round-trip IDENTICAL")),
        Err(e) => no("stat-array-full", e.to_string()),
    });

    // 3. ⭐ The SAME array, based eight bytes later - the reading two live documents publish.
    //    It MUST be refused, at +0x6D18, which is the second override block's `present` bool.
    let mut o = base.to_object()?;
    o[off_stat_override_present] = 1;
    for i in 0..STAT_OVERRIDE_SLOTS {
        let at = off_stat_override + 8 + i * STAT_OVERRIDE_STRIDE;
        o[at] = i as u8;
        o[at + 4..at + 8].copy_from_slice(&(100u32 * (i as u32 + 1)).to_le_bytes());
    }
    out.push(match oracle.round_trip(&o, 0x9000) {
        Err(OracleError::Shape(m)) if m.contains(&format!("+{off_stat_override_block_2:#x}:")) => ok(
            "stat-array-base-NEGATIVE",
            format!("+{:#x} correctly REFUSED - {m}", off_stat_override + 8)),
        Ok(_) => no("stat-array-base-NEGATIVE", format!(
            "a 7-entry array at +{:#x} round-tripped. It must not: the 7th entry lands on \
             +{off_stat_override_block_2:#x}, the SECOND override block's present bool. Either the \
             schema moved or this control no longer tests anything.", off_stat_override + 8)),
        Err(e) => no("stat-array-base-NEGATIVE", format!("refused, but not where expected: {e}")),
    });

    // 4/5. Element 0 of the two other arrays is a real field, not padding a zero would hide.
    for (name, off, val) in [
        ("equipped-base", OFF_EQUIPPED, ITEM_SOID_BASE),
        ("item-slot-base", OFF_ITEM_SLOTS, u64::from(EMPTY_INDEX)),
    ] {
        let mut o = base.to_object()?;
        o[off..off + 8].copy_from_slice(&val.to_le_bytes());
        out.push(match oracle.round_trip(&o, 0x9000) {
            Ok((_, d)) if d.qword(off) == val => ok(name, format!(
                "element 0 @+{off:#x} survives the codec - the array starts here")),
            Ok((_, d)) => no(name, format!("+{off:#x} decoded {:#x}, wrote {val:#x}", d.qword(off))),
            Err(e) => no(name, e.to_string()),
        });
    }

    // 6. The 7-bit count word gates serialisation: with count 1, entry 1 must NOT survive.
    let mut o = base.to_object()?;
    o[OFF_PROGRESSION_COUNT..OFF_PROGRESSION_COUNT + 4].copy_from_slice(&1u32.to_le_bytes());
    o[OFF_PROGRESSIONS..OFF_PROGRESSIONS + 4].copy_from_slice(&1234u32.to_le_bytes());
    let at1 = OFF_PROGRESSIONS + PROGRESSION_STRIDE;
    o[at1..at1 + 4].copy_from_slice(&5678u32.to_le_bytes());
    out.push(match oracle.round_trip(&o, 0x9000) {
        Err(OracleError::Shape(m)) if m.contains(&format!("+{at1:#x}:")) => ok(
            "progression-count-gates",
            format!("count=1 encodes entry 0 and DROPS entry 1 - {m}")),
        Ok(_) => no("progression-count-gates", format!(
            "count=1 encoded entry 1 at +{at1:#x} as well. The count word no longer gates the table, \
             so a level entry written past the count would silently reach the wire.")),
        Err(e) => no("progression-count-gates", format!("refused, but not where expected: {e}")),
    });

    // 7. The baseline fixture really is empty - the claim this whole module is answering.
    if let Some(p) = baseline {
        match std::fs::read(p).map_err(|e| eb(format!("{}: {e}", p.display()))) {
            Ok(raw) => match oracle.decode(&raw) {
                Ok(d) => {
                    let nz = d.nonzero_offsets();
                    out.push(if nz == vec![OFF_SOID, OFF_LAST_ORBITED_DESTINATION] {
                        ok("baseline-is-empty", format!(
                            "{} decodes to EXACTLY 2 non-zero qwords of {struct_size}: +0x00 SOID \
                             {:#018x}, +0x28 destination {:#010x}",
                            p.display(), d.qword(OFF_SOID), d.qword(OFF_LAST_ORBITED_DESTINATION) as u32))
                    } else {
                        no("baseline-is-empty", format!(
                            "{} has non-zero qwords {nz:?}, expected [0, 0x28]", p.display()))
                    });
                }
                Err(e) => out.push(no("baseline-is-empty", e.to_string())),
            },
            Err(e) => out.push(no("baseline-is-empty", e.to_string())),
        }
    }

    // 8/9. The shipped spec, and the monotonicity check against the baseline.
    let spec = ProfileSpec::default();
    match build(&mut oracle, &spec) {
        Ok(b) => {
            out.push(ok("built-profile", format!(
                "{} B / {} bits; {} non-zero qwords; {}",
                b.payload.len(), b.bits, b.nonzero().len(),
                spec.claimed_qwords().iter()
                    .map(|(o, w)| format!("+{o:#x} {w}")).collect::<Vec<_>>().join(", "))));
            let got: BTreeSet<usize> = b.decoded.nonzero_offsets().into_iter().collect();
            let want: BTreeSet<usize> = [OFF_SOID, OFF_LAST_ORBITED_DESTINATION].into_iter().collect();
            out.push(if want.is_subset(&got) && got.len() > want.len() {
                ok("strictly-more-than-baseline", format!(
                    "{} non-zero qwords vs the baseline's {} - and BOTH baseline fields survive",
                    got.len(), want.len()))
            } else {
                no("strictly-more-than-baseline", format!(
                    "non-zero qwords {got:?} is not a strict superset of the baseline's {want:?} - a \
                     populated profile that drops the SOID or the destination is a regression, not a fix"))
            });
        }
        Err(e) => out.push(no("built-profile", e.to_string())),
    }

    // 10/11. ⭐ The all-content override, in BOTH directions.
    //
    // Positive: the two bools survive the codec at +0x6E4C/+0x6E4D, i.e. the schema really carries a
    // field there and the byte is not landing in padding the encoder drops.
    // Negative: the *neighbouring* byte +0x6E4E is NOT a field - if it ever round-trips, fields [7]
    // and [8] have moved and the offsets above are naming something else.
    let mut o = base.to_object()?;
    o[off_all_content_override] = 1;
    o[off_all_content_override_items] = 1;
    out.push(match oracle.round_trip(&o, 0x9000) {
        Ok((_, d))
            if d.object[off_all_content_override] == 1
                && d.object[off_all_content_override_items] == 1 =>
        {
            ok("all-content-override", format!(
                "+{off_all_content_override:#x}/+{off_all_content_override_items:#x} survive the \
                 codec - 0x808018BB fields [7]/[8], the bools 28 investment predicates read"))
        }
        Ok((_, d)) => no("all-content-override", format!(
            "wrote 1/1, decoded {}/{}",
            d.object[off_all_content_override], d.object[off_all_content_override_items])),
        Err(e) => no("all-content-override", e.to_string()),
    });

    let neighbour = off_all_content_override_items + 1; // +0x6E4E - between [8] and [9] @+0x180
    let mut o = base.to_object()?;
    o[neighbour] = 1;
    out.push(match oracle.round_trip(&o, 0x9000) {
        Err(OracleError::Shape(m)) if m.contains(&format!("+{neighbour:#x}:")) => ok(
            "all-content-override-NEGATIVE",
            format!("+{neighbour:#x} is correctly NOT a field - {m}")),
        Ok(_) => no("all-content-override-NEGATIVE", format!(
            "+{neighbour:#x} round-tripped. 0x808018BB's bools are not where this module says they \
             are, so +{off_all_content_override:#x} is naming the wrong field.")),
        Err(e) => no("all-content-override-NEGATIVE", format!("refused, but not where expected: {e}")),
    });

    // ⭐ The 340-slot array with real content, and the padding refusal that pins the element layout.
    //
    // `item-slots-populated` writes `{u16 index, u64 soid}` into the first three slots and requires
    // both fields back plus `0xFFFF` in slot 3 - i.e. that populating the array does not erase the
    // sentinel fill. `item-slot-padding-NEGATIVE` writes a byte into element `+0x02`, which
    // `0x808016E0` leaves uncovered between the `u16` and the `u64`; if that ever round-trips, the
    // SOID is not at `+0x08` and every populated slot names the wrong instance.
    //
    // ⭐⭐ 2026-08-10: the slots are the REAL bucket slots, not 0/1/2. The tab is POSITIONAL
    // (`FUN_0026DB60`), so a control that writes to slots 0-2 asserts nothing about placement and
    // would pass unchanged if the old item-`n`-at-slot-`n` behaviour came back. It now writes
    // Subclass/Heavy/Helmet at their measured slots and requires **slot 0 (Materials) to be EMPTY**
    // - which is exactly the assertion the old fixture would fail.
    let populated = ProfileSpec {
        item_slots: vec![
            ItemSlot { content_index: 710, item_soid: ITEM_SOID_BASE, slot: 40 },
            ItemSlot { content_index: 6299, item_soid: ITEM_SOID_BASE + 1, slot: 113 },
            ItemSlot { content_index: 1512, item_soid: ITEM_SOID_BASE + 2, slot: 43 },
        ],
        ..ProfileSpec::default()
    };
    out.push(match oracle.round_trip(&populated.to_object()?, 0x9000) {
        Ok((_, d)) => {
            let u16_at = |o: usize| u16::from_le_bytes(d.object[o..o + 2].try_into().unwrap());
            let good = populated.item_slots.iter().all(|it| {
                let at = OFF_ITEM_SLOTS + it.slot as usize * ITEM_SLOT_STRIDE;
                u16_at(at) == it.content_index && d.qword(at + 8) == it.item_soid
            }) && u16_at(OFF_ITEM_SLOTS) == EMPTY_INDEX
                && u16_at(OFF_ITEM_SLOTS + 3 * ITEM_SLOT_STRIDE) == EMPTY_INDEX;
            if good {
                ok("item-slots-populated", format!(
                    "slots {:?} at +{OFF_ITEM_SLOTS:#x} carry {{index, soid}} through the codec, and \
                     ⭐ slot 0 (MATERIALS) is {EMPTY_INDEX:#06x} - placement is by BUCKET, not by \
                     enumeration order. This is the array 0x1087660 walks",
                    populated.item_slots.iter().map(|i| i.slot).collect::<Vec<_>>()))
            } else {
                no("item-slots-populated", format!(
                    "slot 0 decoded {{index {:#06x}, soid {:#x}}}", u16_at(OFF_ITEM_SLOTS),
                    d.qword(OFF_ITEM_SLOTS + 8)))
            }
        }
        Err(e) => no("item-slots-populated", e.to_string()),
    });

    let pad = OFF_ITEM_SLOTS + 2;
    let mut o = ProfileSpec::default().to_object()?;
    o[pad] = 1;
    out.push(match oracle.round_trip(&o, 0x9000) {
        Err(OracleError::Shape(m)) if m.contains(&format!("+{pad:#05x}:")) => ok(
            "item-slot-padding-NEGATIVE",
            format!("+{pad:#x} is correctly NOT a field - {m}. The SOID really is at element +0x08")),
        Ok(_) => no("item-slot-padding-NEGATIVE", format!(
            "+{pad:#x} round-tripped, so 0x808016E0 is not {{u16 @+0, u64 @+8}} and every populated \
             slot may be naming the wrong instance")),
        Err(e) => no("item-slot-padding-NEGATIVE", format!("refused, but not where expected: {e}")),
    });

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Completions land at the build's field-[11] base plus the subindex, as `2`, and nowhere else;
    /// a subindex past the field is refused instead of spilling into field [12].
    #[test]
    fn completions_are_written_at_the_field_11_base_plus_sub() {
        let spec = ProfileSpec { completions: vec![56, 62, 936], ..ProfileSpec::default() };
        let plain = ProfileSpec::default().to_object_for(OFFS_56578).unwrap();
        let o = spec.to_object_for(OFFS_56578).unwrap();
        let diff: Vec<usize> = (0..o.len()).filter(|&i| o[i] != plain[i]).collect();
        assert_eq!(diff, vec![0x53C0 + 56, 0x53C0 + 62, 0x53C0 + 936]);
        assert!(diff.iter().all(|&i| o[i] == COMPLETION_SET));
        // 01.29 keeps the documented `+0x52C0` base.
        let o129 = spec.to_object_for(OFFS_0129).unwrap();
        assert_eq!(o129[0x52C0 + 56], COMPLETION_SET);
        let past = ProfileSpec { completions: vec![2560], ..ProfileSpec::default() };
        assert!(past.to_object_for(OFFS_56578).is_err());
        assert!(ProfileSpec { completions: vec![2559], ..ProfileSpec::default() }.to_object_for(OFFS_56578).is_ok());
    }

    #[test]
    fn layout_is_derived_from_the_eboot_not_transcribed() {
        if !crate::test_inputs::reference_eboot() {
            return;
        }
        let image = Image::open_reference().expect("pinned eboot");
        let idx = descriptors::index(&image).expect("descriptor index");
        assert_eq!(derive_layout(&image, &idx).expect("walk"), declared_layout_for(OFFS_0129));
    }

    /// Build 56578's profile moved `+0x300` from field `[12]` on; the walk must agree. Skips when the
    /// 56578 image is absent.
    #[test]
    fn the_56578_layout_is_derived_from_its_eboot() {
        let Some(b) = d1_build::builds().ok().and_then(|b| b.all.iter().find(|x| x.id == "later-56578"))
        else {
            return;
        };
        if !b.eboot_path().exists() {
            return;
        }
        let image = Image::open(b.eboot_path()).unwrap();
        let idx = descriptors::index(&image).unwrap();
        assert_eq!(derive_layout(&image, &idx).unwrap(), declared_layout_for(OFFS_56578));
    }

    #[test]
    fn the_spec_refuses_to_overflow_an_array() {
        let spec = ProfileSpec {
            stat_overrides: (0..8).map(|i| (i as u8, 1)).collect(),
            ..Default::default()
        };
        assert!(matches!(spec.to_object_for(OFFS_0129), Err(OracleError::Shape(_))));
    }

    #[test]
    fn an_empty_spec_still_carries_the_two_fields_orbit_needs() {
        let o = ProfileSpec {
            stat_overrides: Vec::new(),
            empty_item_slots: false,
            ..Default::default()
        }
        .to_object_for(OFFS_0129)
        .unwrap();
        assert_eq!(u64::from_le_bytes(o[0..8].try_into().unwrap()), DEFAULT_SOID);
        assert_eq!(u32::from_le_bytes(o[0x28..0x2c].try_into().unwrap()), DESTINATION_OLD_RUSSIA);
        assert_eq!(o[OFFS_0129.off_stat_override_present], 0);
    }

    /// The whole control set, so `cargo test` and `d1-oracle profile controls` cannot drift.
    #[test]
    fn controls_pass() {
        if !crate::test_inputs::selected_eboot() {
            return;
        }
        let baseline = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../capture-rig/profiles/profile-old-russia.bin");
        let cs = controls(baseline.exists().then_some(baseline.as_path())).expect("controls ran");
        let failed: Vec<_> = cs.iter().filter(|c| !c.pass).map(|c| format!("{}: {}", c.name, c.detail)).collect();
        assert!(failed.is_empty(), "{failed:#?}");
    }
}
