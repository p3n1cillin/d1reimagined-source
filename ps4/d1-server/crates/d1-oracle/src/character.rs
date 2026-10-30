// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Build a populated **family-3 / type-1 character-SELECT CARD** - schema `0x80801954`, 2,688 bytes.
//!
//! # Why this exists, alongside [`crate::profile`]
//!
//! On 2026-08-08 a populated family-4/1 **profile** was built and confirmed by live memory read: the
//! client stores every field we sent (`+0x2B80` 13 equipped SOIDs, `+0x3660` one progression with
//! index 117 and progress 400,000, `+0x6CDC`/`+0x6CE0` the Light override = 400). **And the screen
//! still showed level 0, light 0, a black emblem and no equipment.**
//!
//! ⇒ the profile is the **in-world** source. What is on screen in orbit is the **character-select
//! card**, a *different object* - family 3 / type 1, `0x80801954`, 2,688 bytes - and we serve an
//! identity-only one (`run-boot.sh` leaves `D1_CHARACTER_FILE` unset; its own comment says *"without
//! it the character is identity-only - no level, no light, no gear"*).
//!
//! That diagnosis is `[I]`. This module is what makes it *testable*: one place that knows where the
//! card's level, its Light stat and its equipment summary live, and one encode path through the
//! **client's own encoder** ([`crate::ObjectOracle`]) so a layout mistake becomes a refusal rather
//! than a payload.
//!
//! ```text
//! d1-oracle character layout      # the offset table, RE-DERIVED from the eboot
//! d1-oracle character controls    # 13 checks; exit 2 = nothing it produced may be cited
//! d1-oracle character build --out capture-rig/characters/char-lvl40-geared.bin
//! ```
//!
//! # The card, as the client's own builder `0xA110F0` reads it  `[C]`
//!
//! ```text
//! 0xa114fa  lea  rbx, [r13+0x68]              ; rbx = card + 0x68  (r13 = the 3/1 object)
//! 0xa114fe  call 0x2666f0                     ; light, scaled float   -> the 16-entry table @ C+0x870
//! 0xa11511  call 0x266670                     ; light, raw int        -> same table
//! 0xa1151c  call 0x161550                     ; = `vmovss xmm0,[rdi+8]` -> the FLOAT at C+0x70
//! 0xa11521  call 0x589b10                     ;   floor()
//! 0xa11555  mov  eax, [r13+0x68]              ; ⭐ LEVEL - a plain u32 the server states
//! 0xa11561  cmp  eax, [rbp-0x73c]  ; sete     ; "at level cap"  (cap = 0x276220() = 40)
//! 0xa11570  cmp  [floor(C+0x70)], eax ; setg  ; "the derived number is greater than the level"
//! 0xa11602  call 0x25a320 ; call [rcx+0x160]  ; investment globals -> the EMBLEM SLOT byte at +0x18
//! 0xa11627  mov  ax, [r13+rax+0x258]          ; slot<<6 + 0x258 = equipment[slot].content_index
//! 0xa11642  cmp  di, 0xffff ; je              ; 0xFFFF = no emblem
//! ```
//!
//! # ⭐ THE FINDING THIS MODULE EXISTS TO PIN: an unused stat entry is `0xFF`, not `0`
//!
//! There are **two** readers of the 16-entry stat table at `C+0x870`, and they disagree about a
//! zero-filled table:
//!
//! ```text
//! 0x2666f0   cmp [rbx+rcx*8+0x808], al ; lea rcx,[rcx+1] ; jne loop   ; FIRST match wins, then stops
//!            mov edi, [rbx+rcx*8+0x804]                               ; = +0x80C+8i (the same
//!                                                                     ;   after-the-compare `lea`
//!                                                                     ;   that mis-shifted the
//!                                                                     ;   PROFILE's override array)
//! 0x266740   memset(out, 0, 0x100)                                    ; 64 u32 slots
//!            movsx rcx, byte [rbx+rax*8+0x808] ; cmp rcx,-1 ; je skip  ; ⭐ 0xFF == ABSENT
//!            mov edx, [rbx+rax*8+0x80c] ; mov [r14+rcx*4], edx         ; out[key] = value, i = 0..15
//! ```
//!
//! `0x2666F0` breaks on the first match, so `{0, 400}` in entry 0 returns 400 whatever follows it.
//! `0x266740` does **not** break - it writes every non-`0xFF` entry into a key-addressed array, in
//! order. With entries 1..15 left zero they all read as *"stat index 0 = 0"* and **overwrite Light
//! with zero, fifteen times.** So a zero-filled stat table is not "empty", it is a claim of sixteen
//! copies of stat index 0 - the same trap as `0xFFFF` vs `0x0000` for content indices, in a second
//! place and with a different sentinel. [`CharacterSpec::sentinel_stat_keys`] fills the unused
//! entries with `0xFF`.
//!
//! On build 56578 the three sibling tables at `C+0x8F0`, `C+0x970`, `C+0x9F0` supply primary,
//! special and heavy weapon investment stats. The peer carries them at `+0x8B8/+0x938/+0x9B8`.
//! A native pickup trace on 27 Oct followed primary Magazine through `0x910C60` and `0x28A2F0`;
//! setting its content-derived value changed the HUD magazine from 12 to 25. See
//! [`crate::weapon_stats`]. This builder keeps its historical defaults for existing callers;
//! `character weapon-stats` fills a selected peer table from its equipped owned definition.
//!
//! # ⚠ `d1-eboot descriptors` prints the ELEMENT SIZE in the offset column of an array field
//!
//! Same trap as [`crate::profile`], and it bites twice here: `0x808045D9`'s single field prints
//! `[0] @+0x040` (the 64-byte equipment element) and `0x80804703`'s prints `[0] @+0x008` (the 8-byte
//! stat element). **Neither is a displacement** - each array begins at its *parent* field's offset.
//! [`derive_layout`] therefore reads element count and size from the container's descriptor header
//! (`anchor+0x20` / `anchor+0x28`) and asserts `count * size == struct_size`, and [`controls`] pins
//! both readings from the other side with a **negative** control that must keep failing.
//!
//! # What is deliberately NOT written
//!
//! * ⭐ **The emblem - CLOSED 2026-08-09, both halves measured** (`historicaldocs/design/item-content-indices.md`).
//!   `0xA11627` hands `equipment[slot].content_index` to the emblem colour resolver `0xA81B00`.
//!   The **slot** is [`EMBLEM_SLOT`] = 12 (`investment_globals->vtbl[0x160]()[0x18]`, replayed on
//!   `runs/snapshots/orbit-all.snap` = `0x0C`); the **index** comes from
//!   [`crate::item_index::IndexTable`], a 7,544-row table read out of
//!   `ps4_investment_globals_012f_3.pkg` entry 8126 whose element `+0x00` is the item's content
//!   hash. *The Reflective Proof* `0x5508AC2A` → **1512**. Nothing is fabricated: `require()`
//!   refuses a hash 01.29 content does not know, and slot 12 is left free of staged inventory
//!   because an item parked there would render as the player's emblem.
//! * **Customization** (`C+0x0C`, 12 fields) - left zero, exactly as [`crate::profile`] leaves it.
//! * **`C+0x30`/`C+0x34`/`C+0x38`/`C+0x3C`** - the four `u32`s the older fixtures filled with the
//!   ruler values 11/22/33/44. Those were *probes*, not content; `0xA115D2` reads `C+0x38` into the
//!   card, so a leftover `33` there is a lie the card would repeat.

use std::collections::BTreeSet;
use std::path::Path;

use d1_eboot::descriptors::{self, Descriptor, Field, Index, LEAF};
use d1_eboot::image::Image;

use crate::error::{OracleError, Result};
use crate::object::Decoded;
use crate::profile::ArrayShape;
use crate::ObjectOracle;

/// Family 3 / type 1 - the character-select card.
pub const TAG: u32 = 0x8080_1954;
pub const STRUCT_SIZE: usize = 2_688;

// ── C offsets. ABSOLUTE in the decoded struct. Every one is re-derived by `derive_layout`. ───────
pub const OFF_SOID: usize = 0x0000;
/// `{u8 race @+0x08, u8 gender @+0x09, u8 class @+0x0A}` - `0x808017FC`. The card builder reads them
/// individually at `0xA115DE`..: `[r13+0xA]` class, `[r13+8]` race, `[r13+9]` gender.
pub const OFF_IDENTITY: usize = 0x0008;
/// `0x8080181A`, 36 bytes, 12 fields - the same block the profile carries at *its* `+0x0C`.
pub const OFF_CUSTOMIZATION: usize = 0x000C;
/// `0x80801855`, 2,568 bytes, 16 fields. Every interesting card field is inside it.
pub const OFF_STATS_BLOCK: usize = 0x0068;
/// ⭐ **LEVEL** - a plain `u32` the server states. `0xA11555: mov eax,[r13+0x68]`.
pub const OFF_LEVEL: usize = 0x0068;
/// `f32`, `[U]` in meaning. `0xA1151C` calls `0x161550` (= `vmovss xmm0,[rdi+8]`, `rdi = card+0x68`)
/// and floors it; `0xA11570` then does `setg` against the level. Reads as a *derived* level (D1's
/// Light-derived level), `[I]`.
pub const OFF_DERIVED_LEVEL: usize = 0x0070;
/// ⭐ 20 × 64 B `0x808045D9`/`0x8080183C` - the **equipment summary**.
pub const OFF_EQUIPMENT: usize = 0x0250;
pub const EQUIPMENT_SLOTS: usize = 20;
pub const EQUIPMENT_STRIDE: usize = 64;
/// Element `+0x00`, `u64`. Item-instance SOID by analogy with the profile's `+0x2B80` list - `[I]`;
/// `[C]` only that it is a `u64` field.
pub const EQUIP_ITEM_SOID: usize = 0x00;
/// ⭐ Element `+0x08`, `u16` - the **content index**, and the emblem. `0xA11627` reads
/// `[card + 0x258 + 64*slot]`, i.e. exactly this field. `[C]`
pub const EQUIP_CONTENT_INDEX: usize = 0x08;
/// ⭐ Element `+0x0A`, `u16` - the **weapon object index**. Reader found 2026-10-14 on build 56578:
/// `0x910C60` (one call per weapon slot from the player-configuration builder `0x912240`) indexes
/// the `investment_globals+0x350` table with it and resolves the weapon's object-definition tag.
/// `0` names record 0, a type-8 object: the loadout spawned as non-weapons. Source:
/// [`crate::art_arrangement::weapon_object`] (`def_client+0x1C8`), written for [`WEAPON_ORDINALS`].
pub const EQUIP_FIELD_0A: usize = 0x0A;
/// The equipped ordinals (`def_client+0x18`) of the primary, special and heavy weapon - measured on
/// build 56578 from the staged weapons' definitions (6, 7, 8). Element `+0x0A` is written only for
/// these; every other slot keeps the `0` all fixtures have always shipped.
pub const WEAPON_ORDINALS: [usize; 3] = [6, 7, 8];
/// ⭐⭐ Element `+0x0C`, `u16` - **THE ART-ARRANGEMENT INDEX. This is what makes gear DRAW.**
///
/// `FUN_0054CF70` (the render-description builder; callers `0x54CF40` ← `0x58D350` the world-tag
/// loader on the family-0/1 block, `0x54EB40` ← `0xA98550` the preview widget on the family-3/1
/// card's block at `+0x68` or family-1/1 at `+0xB60`) reads it at `block + 0x1F4 + slot*0x40` and
/// resolves it with `FUN_0054B530` - a bounds-checked `index < count` lookup in a 24-byte-stride
/// content table. On success `FUN_0054E6C0` fills the slot's four u64 geometry handles at
/// `out + 0x188 + slot*0x20` (pre-initialised to `-1` = "no geometry").
///
/// ⭐ [`crate::item_index::EMPTY_ARRANGEMENT`] (`0xFFFF`) sign-extends to `-1`, fails the bound and
/// leaves the slot at "no geometry" - the honest empty. ⛔ **`0` is IN RANGE** and silently names
/// arrangement #0; every card before 2026-08-10 shipped `0` in all 20 slots.
///
/// The class-correct value is content data - `FUN_00289E80(def+0x60 gear record, class)` - measured
/// by `capture-rig/oodle/art-arrangement.py` and consumed via [`crate::item_index::ArtTable`].
pub const EQUIP_ART_ARRANGEMENT: usize = 0x0C;
/// ⭐ Element `+0x0E` - **8 × u16 extra arrangement indices** (`0x80800F9D`, structurally the twin
/// of the inventory element's `+0x26` list). `FUN_0054E6C0`'s tail walks all eight with `0xFFFF` as
/// a per-entry SKIP - so ⛔ a `0` here is a live reference to arrangement #0, and every fixture
/// before 2026-08-10 shipped 8 zeros. Sentinel-filled by [`CharacterSpec::to_object`].
pub const EQUIP_EXTRA_ARRANGEMENTS: usize = 0x0E;
pub const EXTRA_ARRANGEMENT_SLOTS: usize = 8;
/// ⭐ 16 × 8 B `0x80804703`/`0x808017E3` - `{u8 stat_index @+0, u32 value @+4}`. **Light lives here.**
pub const OFF_STATS: usize = 0x0870;
pub const STAT_SLOTS: usize = 16;
pub const STAT_STRIDE: usize = 8;
/// Primary, special and heavy weapon investment tables (56578). See [`crate::weapon_stats`].
pub const OFF_STATS_SIBLINGS: [usize; 3] = [0x08F0, 0x0970, 0x09F0];

/// `0xFFFF` is "absent" for a **content index** (`0xA11642: cmp di,0xffff; je`). `0x0000` is a valid
/// index everywhere in this client and is never "absent".
pub const EMPTY_CONTENT_INDEX: u16 = 0xFFFF;
/// ⭐ `0xFF` is "absent" for a **stat key** - a *different* sentinel in a *different* width.
/// `0x266740: movsx rcx, byte [rbx+rax*8+0x808]; cmp rcx, -1; je`.
pub const EMPTY_STAT_KEY: u8 = 0xFF;

/// The Light stat index is **0** - `[C]` two ways: a snapshot replay of `0x281F50`
/// (`THE-EMPTY-PROFILE.md` §4, with `0x276220` → 40 as the positive control), and content-side from
/// the stat definition table (`historicaldocs/design/progression-table.md`: block 515, entry 580, 36 × 16 B;
/// `Defense` = 1, `Attack` = 11).
pub const LIGHT_STAT_INDEX: u8 = crate::profile::LIGHT_STAT_INDEX;

/// The SOID `d1-responder` keys the family-3 character by, and the SOID every fixture in
/// `capture-rig/characters/` carries at `+0x00`.
pub const DEFAULT_SOID: u64 = crate::CHARACTER_SOID;
/// `{race, gender, class}`. ⚠ **Must agree with the profile** - [`controls`] asserts it.
pub const DEFAULT_IDENTITY: [u8; 3] = crate::profile::DEFAULT_IDENTITY;
/// The level cap this content ships: `0x276220` = `investment_globals->vtbl[0x68]()[0x170]`, replayed
/// on `runs/snapshots/orbit-all.snap` and returning `0x28`. `[O]`
pub const LEVEL_CAP: u32 = 40;

/// ⭐ The **emblem's equipment slot**, re-exported from [`crate::item_index`]: **12**, read live off
/// `investment_globals->vtbl[0x160]()[0x18]`. `equipment[12]`'s content index is the only one this
/// client is *proven* to consume, so this module keeps that slot free for the emblem and lets the
/// staged inventory skip over it. Every other slot's meaning is `[U]`.
pub const EMBLEM_SLOT: usize = crate::item_index::EMBLEM_SLOT;

/// ⛔ Content index **1** - *'Bronze Tier'* (`stats:{}`, `equippable:false`, `hasGeometry:false`).
/// The literal `stage_loadout()` hardcoded into `element+0x24` for every item in every run this
/// project ever made. Named so that "is this element legacy?" is a comparison against a constant
/// with a reason attached, not a bare `!= 1`.
pub const LEGACY_BRONZE_TIER_INDEX: u16 = 1;

/// One slot of the 20-entry equipment summary at `C+0x250`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EquipSlot {
    /// Element `+0x00`. `0` leaves the slot's SOID unset.
    pub item_soid: u64,
    /// Element `+0x08`. ⭐ [`EMPTY_CONTENT_INDEX`] means *absent*; the emblem resolver skips it.
    pub content_index: u16,
    /// ⭐ Element `+0x0C` - the **art-arrangement index** ([`EQUIP_ART_ARRANGEMENT`]).
    /// [`crate::item_index::EMPTY_ARRANGEMENT`] = no geometry, honestly; resolve real values with
    /// [`crate::item_index::ArtTable`], never invent one - a wrong index renders SOMETHING and
    /// reads as success.
    pub art_arrangement: u16,
    /// Element `+0x0A` - the **weapon object index** ([`EQUIP_FIELD_0A`]); `0` for non-weapons.
    pub weapon_object: u16,
}

impl EquipSlot {
    pub const EMPTY: EquipSlot = EquipSlot {
        item_soid: 0,
        content_index: EMPTY_CONTENT_INDEX,
        art_arrangement: crate::item_index::EMPTY_ARRANGEMENT,
        weapon_object: 0,
    };
}

/// Everything a populated card needs, and nothing that is a guess.
#[derive(Debug, Clone, PartialEq)]
pub struct CharacterSpec {
    pub soid: u64,
    /// `{race, gender, class}` - **must** match the profile's `+0x08`.
    pub identity: [u8; 3],
    /// ⭐ `C+0x68`. The server states it outright; the client does not derive it here.
    pub level: u32,
    /// `C+0x70`, floored and compared `setg` against `level`. Writing `level as f32` makes the two
    /// agree and the flag false, which is the only self-consistent choice available - `[I]`.
    pub derived_level: f32,
    /// `{stat_index, value}` pairs written from entry 0 up. `0x2666F0` returns the **first** key
    /// match, so Light belongs at entry 0.
    pub stats: Vec<(u8, u32)>,
    /// Up to [`EQUIPMENT_SLOTS`] slots, in slot order.
    pub equipment: Vec<EquipSlot>,
    /// Fill unused entries of the `C+0x870` table with [`EMPTY_STAT_KEY`]. ⭐ Leaving them zero
    /// makes `0x266740` overwrite Light with 0 - see the module docs. Default **true**.
    pub sentinel_stat_keys: bool,
    /// Also sentinel-fill the three weapon tables at [`OFF_STATS_SIBLINGS`]. Default **false**
    /// preserves existing fixtures. This alone supplies no weapon stats; use `character
    /// weapon-stats` on the peer output to populate them from the equipped definition.
    pub sentinel_all_stat_tables: bool,
    /// Mark unused equipment slots' content index [`EMPTY_CONTENT_INDEX`] instead of leaving them
    /// zero (which claims 20 copies of content index 0). Default **true**.
    pub empty_equipment_indices: bool,
}

impl Default for CharacterSpec {
    /// The shipped card: the responder's character SOID, the profile's identity, level 40,
    /// Light 400 at stat index 0, and 20 explicitly-empty equipment slots.
    fn default() -> Self {
        CharacterSpec {
            soid: DEFAULT_SOID,
            identity: DEFAULT_IDENTITY,
            level: LEVEL_CAP,
            derived_level: LEVEL_CAP as f32,
            stats: vec![(LIGHT_STAT_INDEX, crate::profile::LIGHT_ROI_CAP)],
            equipment: Vec::new(),
            sentinel_stat_keys: true,
            sentinel_all_stat_tables: false,
            empty_equipment_indices: true,
        }
    }
}

impl CharacterSpec {
    /// Lay the spec out as the decoded 2,688-byte C struct.
    ///
    /// This never touches the wire - [`build`] hands the result to the client's own encoder, which
    /// is the only thing allowed to turn it into bytes.
    pub fn to_object(&self) -> Result<Vec<u8>> {
        let too_many = |what: &str, got: usize, cap: usize| {
            Err(OracleError::Shape(format!(
                "{got} {what} exceeds the schema's {cap} slots - the extra entries would land in \
                 the NEXT field and the codec would silently reshape them"
            )))
        };
        if self.stats.len() > STAT_SLOTS {
            return too_many("stat entries", self.stats.len(), STAT_SLOTS);
        }
        if self.equipment.len() > EQUIPMENT_SLOTS {
            return too_many("equipment slots", self.equipment.len(), EQUIPMENT_SLOTS);
        }

        let mut o = vec![0u8; STRUCT_SIZE];
        o[OFF_SOID..OFF_SOID + 8].copy_from_slice(&self.soid.to_le_bytes());
        o[OFF_IDENTITY..OFF_IDENTITY + 3].copy_from_slice(&self.identity);
        o[OFF_LEVEL..OFF_LEVEL + 4].copy_from_slice(&self.level.to_le_bytes());
        o[OFF_DERIVED_LEVEL..OFF_DERIVED_LEVEL + 4]
            .copy_from_slice(&self.derived_level.to_le_bytes());

        // Equipment: sentinel first, then the real slots on top.
        //
        // ⭐ Three sentinels per slot, not one: the content index at +0x08 (`0xA11642` skips only
        // `0xFFFF`), the art-arrangement at +0x0C (`FUN_0054B530` rejects only `-1` on its bound)
        // and all 8 extra arrangements at +0x0E (`FUN_0054E6C0` skips only `-1`, per entry). A `0`
        // in any of them is a LIVE reference to entry #0 of its table.
        if self.empty_equipment_indices {
            for i in 0..EQUIPMENT_SLOTS {
                let at = OFF_EQUIPMENT + i * EQUIPMENT_STRIDE;
                o[at + EQUIP_CONTENT_INDEX..at + EQUIP_CONTENT_INDEX + 2]
                    .copy_from_slice(&EMPTY_CONTENT_INDEX.to_le_bytes());
                o[at + EQUIP_ART_ARRANGEMENT..at + EQUIP_ART_ARRANGEMENT + 2]
                    .copy_from_slice(&crate::item_index::EMPTY_ARRANGEMENT.to_le_bytes());
                for k in 0..EXTRA_ARRANGEMENT_SLOTS {
                    let x = at + EQUIP_EXTRA_ARRANGEMENTS + k * 2;
                    o[x..x + 2]
                        .copy_from_slice(&crate::item_index::EMPTY_ARRANGEMENT.to_le_bytes());
                }
            }
        }
        for (i, e) in self.equipment.iter().enumerate() {
            let at = OFF_EQUIPMENT + i * EQUIPMENT_STRIDE;
            o[at + EQUIP_ITEM_SOID..at + EQUIP_ITEM_SOID + 8]
                .copy_from_slice(&e.item_soid.to_le_bytes());
            o[at + EQUIP_CONTENT_INDEX..at + EQUIP_CONTENT_INDEX + 2]
                .copy_from_slice(&e.content_index.to_le_bytes());
            o[at + EQUIP_ART_ARRANGEMENT..at + EQUIP_ART_ARRANGEMENT + 2]
                .copy_from_slice(&e.art_arrangement.to_le_bytes());
            o[at + EQUIP_FIELD_0A..at + EQUIP_FIELD_0A + 2]
                .copy_from_slice(&e.weapon_object.to_le_bytes());
        }

        // Stats: sentinel first, then the real entries on top. ⭐ Order matters - see the module
        // docs on `0x266740`.
        let mut tables: Vec<usize> = Vec::new();
        if self.sentinel_stat_keys {
            tables.push(OFF_STATS);
        }
        if self.sentinel_all_stat_tables {
            tables.extend_from_slice(&OFF_STATS_SIBLINGS);
        }
        for base in tables {
            for i in 0..STAT_SLOTS {
                o[base + i * STAT_STRIDE] = EMPTY_STAT_KEY;
            }
        }
        for (i, (key, value)) in self.stats.iter().enumerate() {
            let at = OFF_STATS + i * STAT_STRIDE;
            o[at] = *key;
            o[at + 4..at + 8].copy_from_slice(&value.to_le_bytes());
        }
        Ok(o)
    }

    /// Convenience: put an emblem content index into slot `slot`, preserving that slot's SOID.
    ///
    /// ⛔ Both numbers are `[U]` in this project - see the module docs. This exists so that closing
    /// the `[U]` is one flag, **not** so that a number can be invented.
    pub fn emblem(&mut self, slot: usize, content_index: u16) -> Result<()> {
        if slot >= EQUIPMENT_SLOTS {
            return Err(OracleError::Shape(format!(
                "emblem slot {slot} is outside the {EQUIPMENT_SLOTS}-slot equipment summary"
            )));
        }
        while self.equipment.len() <= slot {
            self.equipment.push(EquipSlot::EMPTY);
        }
        self.equipment[slot].content_index = content_index;
        Ok(())
    }
}

/// A built card: the wire payload plus the decode that proves it.
#[derive(Debug, Clone)]
pub struct BuiltCharacter {
    pub payload: Vec<u8>,
    pub bits: u32,
    pub decoded: Decoded,
}

impl BuiltCharacter {
    pub fn nonzero(&self) -> Vec<(usize, u64)> {
        self.decoded.nonzero_qwords()
    }
}

/// Encode a spec through the **client's own encoder**, then decode it back and require the C object
/// to be identical.
pub fn build(oracle: &mut ObjectOracle, spec: &CharacterSpec) -> Result<BuiltCharacter> {
    if oracle.tag() != TAG {
        return Err(OracleError::Shape(format!(
            "oracle is for tag {:#010x}, not the character-card tag {TAG:#010x}",
            oracle.tag()
        )));
    }
    let object = spec.to_object()?;
    let (enc, dec) = oracle.round_trip(&object, 0x4000)?;
    verify(spec, &dec)?;
    Ok(BuiltCharacter { payload: enc.payload, bits: enc.bits, decoded: dec })
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// ⭐⭐ The SECOND carrier of the same block - family 0 / type 1, the "peer view"
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// Family 0 / type 1 (`0x808017FD`, 2,632 B) - see [`crate::roster::PEER_TAG`].
pub const PEER_TAG: u32 = crate::roster::PEER_TAG;
pub const PEER_STRUCT_SIZE: usize = crate::roster::PEER_SIZE;
/// ⭐ Where the shared `0x80801855` block sits in the peer view. The card carries it at
/// [`OFF_STATS_BLOCK`] = `0x68`; this object carries it at **`0x30`**, and the *only* structural
/// difference between the two roots is the card's extra `0x80801755` field at `+0x30`:
///
/// | | `0x80801954` (family 3/1) | `0x808017FD` (family 0/1) |
/// |---|---|---|
/// | `[0]` | `+0x000` u64 SOID | `+0x000` u64 SOID |
/// | `[1]` | `+0x008` `0x808017FC` identity | `+0x008` `0x808017FC` identity |
/// | `[2]` | `+0x00C` `0x8080181A` customization | `+0x00C` `0x8080181A` customization |
/// | `[3]` | `+0x030` `0x80801755` (0x38 B) | ⭐ `+0x030` **`0x80801855`** |
/// | `[4]` | ⭐ `+0x068` **`0x80801855`** | `+0xA38` `0x80801923` trailer |
/// | `[5]` | `+0xA70` `0x80801923` trailer | - |
///
/// `[C]`, from `d1-eboot descriptors` on both tags; [`peer_controls`] re-derives it and refuses to
/// build if the eboot disagrees.
pub const PEER_BLOCK: usize = 0x0030;
/// The shared block's own size - `0x80801855`, 2,568 bytes.
pub const BLOCK_SIZE: usize = 0x0A08;
/// The `0x80801923` trailer, `{f32, u32, u16}` in 16 bytes, immediately after the block.
pub const TRAILER_SIZE: usize = 0x10;

/// ⭐⭐ **THE OBJECT THE CHARACTER SCREEN ACTUALLY READS.**
///
/// `c_cui_investment_character_state_component::vfunc[14]` (`0x9A3C30`) - and its siblings
/// `cui::c_investment_character_statistics_component::vfunc[14]` (`0x9A5F50`) and
/// `cui::c_investment_stat_group_component::vfunc[14]` (`0xA1AB60`) - resolve **family 1** or
/// **family 0**, never family 3. `0xA110F0` is the *only* function in the whole 47,304-function
/// corpus that resolves family 3, and it builds the character-SELECT tiles.
///
/// ⇒ level 40 / Light 400 on one screen and 0 / 0 on the other is **two readers of two different
/// objects**, and the one showing zeros has never been served. This function serves it: the same
/// [`CharacterSpec`] the card is built from, re-laid into the family-0 carrier.
///
/// ⚠ Reach: `static-client-inference`. The layout is `[C]`; that this object is what clears the
/// zeros is `[I]` until a run says so, and the run is the discriminator.
pub fn to_peer_object(spec: &CharacterSpec) -> Result<Vec<u8>> {
    let card = spec.to_object()?;
    if card.len() != STRUCT_SIZE {
        return Err(OracleError::Shape(format!(
            "card object is {} bytes, expected {STRUCT_SIZE}",
            card.len()
        )));
    }
    if PEER_BLOCK + BLOCK_SIZE + TRAILER_SIZE != PEER_STRUCT_SIZE {
        return Err(OracleError::Shape(format!(
            "the peer view does not tile: block +{PEER_BLOCK:#05x} + {BLOCK_SIZE} + \
             {TRAILER_SIZE} = {} but the struct is {PEER_STRUCT_SIZE} bytes. The block offset is \
             wrong, and copying into it would run off the end of the object",
            PEER_BLOCK + BLOCK_SIZE + TRAILER_SIZE
        )));
    }
    let mut o = vec![0u8; PEER_STRUCT_SIZE];
    // SOID + identity + customization occupy [0x00, 0x30) identically in both roots.
    o[0..PEER_BLOCK].copy_from_slice(&card[0..PEER_BLOCK]);
    // The block, moved from +0x68 to +0x30.
    o[PEER_BLOCK..PEER_BLOCK + BLOCK_SIZE]
        .copy_from_slice(&card[OFF_STATS_BLOCK..OFF_STATS_BLOCK + BLOCK_SIZE]);
    // The trailer follows the block in both.
    let ct = OFF_STATS_BLOCK + BLOCK_SIZE;
    let pt = PEER_BLOCK + BLOCK_SIZE;
    o[pt..pt + TRAILER_SIZE].copy_from_slice(&card[ct..ct + TRAILER_SIZE]);
    Ok(o)
}

/// Encode the family-0 peer view through the client's own encoder and prove the round trip.
pub fn build_peer(oracle: &mut ObjectOracle, spec: &CharacterSpec) -> Result<BuiltCharacter> {
    if oracle.tag() != PEER_TAG {
        return Err(OracleError::Shape(format!(
            "oracle is for tag {:#010x}, not the peer-view tag {PEER_TAG:#010x}",
            oracle.tag()
        )));
    }
    let object = to_peer_object(spec)?;
    let (enc, dec) = oracle.round_trip(&object, 0x4000)?;
    verify_peer(spec, &dec)?;
    Ok(BuiltCharacter { payload: enc.payload, bits: enc.bits, decoded: dec })
}

/// The peer view's own verification: **the same assertions as [`verify`], at the moved offsets.**
///
/// ⭐ It is written as a delta against the card rather than duplicated, so a change to one cannot
/// silently fail to reach the other.
pub fn verify_peer(spec: &CharacterSpec, dec: &Decoded) -> Result<()> {
    let bad = |m: String| Err(OracleError::Shape(m));
    if dec.object.len() != PEER_STRUCT_SIZE {
        return bad(format!("decoded {} bytes, expected {PEER_STRUCT_SIZE}", dec.object.len()));
    }
    if dec.qword(OFF_SOID) != spec.soid {
        return bad(format!("+{OFF_SOID:#06x} SOID is {:#x}, expected {:#x}", dec.qword(OFF_SOID), spec.soid));
    }
    if dec.object[OFF_IDENTITY..OFF_IDENTITY + 3] != spec.identity {
        return bad(format!(
            "+{OFF_IDENTITY:#06x} identity is {:?}, expected {:?}",
            &dec.object[OFF_IDENTITY..OFF_IDENTITY + 3],
            spec.identity
        ));
    }
    // Re-project the decoded peer object back into a card-shaped buffer and reuse `verify`, so the
    // two carriers can never drift apart in what they assert.
    let mut as_card = vec![0u8; STRUCT_SIZE];
    as_card[0..PEER_BLOCK].copy_from_slice(&dec.object[0..PEER_BLOCK]);
    as_card[OFF_STATS_BLOCK..OFF_STATS_BLOCK + BLOCK_SIZE]
        .copy_from_slice(&dec.object[PEER_BLOCK..PEER_BLOCK + BLOCK_SIZE]);
    let ct = OFF_STATS_BLOCK + BLOCK_SIZE;
    let pt = PEER_BLOCK + BLOCK_SIZE;
    as_card[ct..ct + TRAILER_SIZE].copy_from_slice(&dec.object[pt..pt + TRAILER_SIZE]);
    verify(spec, &Decoded { object: as_card, ..dec.clone() })
}

/// ⭐ **The control trap 2 prescribes**: assert that every value comes back **at the offset it was
/// written to**, out of the client's own decoder - not that some algebra closes.
///
/// `round_trip` already proves the whole struct survived; this exists so a *silently empty* spec
/// cannot pass as a populated card.
pub fn verify(spec: &CharacterSpec, dec: &Decoded) -> Result<()> {
    let u32_at =
        |off: usize| -> u32 { u32::from_le_bytes(dec.object[off..off + 4].try_into().unwrap_or([0; 4])) };
    let u16_at =
        |off: usize| -> u16 { u16::from_le_bytes(dec.object[off..off + 2].try_into().unwrap_or([0; 2])) };
    let bad = |m: String| Err(OracleError::Shape(m));

    if dec.object.len() != STRUCT_SIZE {
        return bad(format!("decoded {} bytes, expected {STRUCT_SIZE}", dec.object.len()));
    }
    if dec.qword(OFF_SOID) != spec.soid {
        return bad(format!(
            "+{OFF_SOID:#06x} SOID is {:#x}, expected {:#x} - the object header and the payload have \
             to agree or the client drops it",
            dec.qword(OFF_SOID),
            spec.soid
        ));
    }
    if dec.object[OFF_IDENTITY..OFF_IDENTITY + 3] != spec.identity {
        return bad(format!(
            "+{OFF_IDENTITY:#06x} identity is {:?}, expected {:?}",
            &dec.object[OFF_IDENTITY..OFF_IDENTITY + 3],
            spec.identity
        ));
    }
    if u32_at(OFF_LEVEL) != spec.level {
        return bad(format!(
            "+{OFF_LEVEL:#06x} level is {}, expected {} - 0xA11555 reads this exact u32",
            u32_at(OFF_LEVEL),
            spec.level
        ));
    }
    let got = f32::from_bits(u32_at(OFF_DERIVED_LEVEL));
    if got.to_bits() != spec.derived_level.to_bits() {
        return bad(format!(
            "+{OFF_DERIVED_LEVEL:#06x} derived level is {got}, expected {}",
            spec.derived_level
        ));
    }
    for (i, e) in spec.equipment.iter().enumerate() {
        let at = OFF_EQUIPMENT + i * EQUIPMENT_STRIDE;
        if dec.qword(at + EQUIP_ITEM_SOID) != e.item_soid {
            return bad(format!(
                "equipment[{i}] SOID @+{:#06x} is {:#x}, expected {:#x}",
                at + EQUIP_ITEM_SOID,
                dec.qword(at + EQUIP_ITEM_SOID),
                e.item_soid
            ));
        }
        if u16_at(at + EQUIP_CONTENT_INDEX) != e.content_index {
            return bad(format!(
                "equipment[{i}] content index @+{:#06x} is {:#06x}, expected {:#06x}",
                at + EQUIP_CONTENT_INDEX,
                u16_at(at + EQUIP_CONTENT_INDEX),
                e.content_index
            ));
        }
        if u16_at(at + EQUIP_ART_ARRANGEMENT) != e.art_arrangement {
            return bad(format!(
                "equipment[{i}] art arrangement @+{:#06x} is {:#06x}, expected {:#06x} - this is \
                 the u16 FUN_0054CF70 hands to FUN_0054B530, i.e. the 3D model",
                at + EQUIP_ART_ARRANGEMENT,
                u16_at(at + EQUIP_ART_ARRANGEMENT),
                e.art_arrangement
            ));
        }
    }
    if spec.empty_equipment_indices {
        for i in spec.equipment.len()..EQUIPMENT_SLOTS {
            let at = OFF_EQUIPMENT + i * EQUIPMENT_STRIDE + EQUIP_CONTENT_INDEX;
            if u16_at(at) != EMPTY_CONTENT_INDEX {
                return bad(format!(
                    "equipment[{i}] content index @+{at:#06x} is {:#06x}, expected the EMPTY marker \
                     {EMPTY_CONTENT_INDEX:#06x} - 0x0000 is a valid index, so a zero here claims the \
                     Guardian is wearing content 0",
                    u16_at(at)
                ));
            }
        }
        // The two arrangement sentinels cover ALL 20 slots - a populated slot's +0x0E extras come
        // from the sentinel pass too, so check every element.
        for i in 0..EQUIPMENT_SLOTS {
            let at = OFF_EQUIPMENT + i * EQUIPMENT_STRIDE;
            let want_art = spec
                .equipment
                .get(i)
                .map(|e| e.art_arrangement)
                .unwrap_or(crate::item_index::EMPTY_ARRANGEMENT);
            if u16_at(at + EQUIP_ART_ARRANGEMENT) != want_art {
                return bad(format!(
                    "equipment[{i}] art arrangement @+{:#06x} is {:#06x}, expected {want_art:#06x} \
                     - 0 is IN RANGE for FUN_0054B530 and silently names arrangement #0",
                    at + EQUIP_ART_ARRANGEMENT,
                    u16_at(at + EQUIP_ART_ARRANGEMENT)
                ));
            }
            for k in 0..EXTRA_ARRANGEMENT_SLOTS {
                let x = at + EQUIP_EXTRA_ARRANGEMENTS + k * 2;
                if u16_at(x) != crate::item_index::EMPTY_ARRANGEMENT {
                    return bad(format!(
                        "equipment[{i}] extra arrangement [{k}] @+{x:#06x} is {:#06x}, expected \
                         {:#06x} - FUN_0054E6C0 walks all 8 and skips only -1",
                        u16_at(x),
                        crate::item_index::EMPTY_ARRANGEMENT
                    ));
                }
            }
        }
    }
    for (i, (key, value)) in spec.stats.iter().enumerate() {
        let at = OFF_STATS + i * STAT_STRIDE;
        if dec.object[at] != *key || u32_at(at + 4) != *value {
            return bad(format!(
                "stat[{i}] @+{at:#06x} is {{index {}, value {}}}, expected {{index {key}, value \
                 {value}}}",
                dec.object[at],
                u32_at(at + 4)
            ));
        }
    }
    if spec.sentinel_stat_keys {
        for i in spec.stats.len()..STAT_SLOTS {
            let at = OFF_STATS + i * STAT_STRIDE;
            if dec.object[at] != EMPTY_STAT_KEY {
                return bad(format!(
                    "stat[{i}] key @+{at:#06x} is {:#04x}, expected the ABSENT marker \
                     {EMPTY_STAT_KEY:#04x} - 0x266740 writes every non-0xFF entry into out[key], so a \
                     zero key here overwrites Light with 0",
                    dec.object[at]
                ));
            }
        }
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Layout, re-derived from the eboot rather than transcribed
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// The whole card layout, derived from the descriptor tree in this eboot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub struct_size: u32,
    pub soid: usize,
    pub identity: usize,
    pub customization: usize,
    pub stats_block: usize,
    pub level: usize,
    pub derived_level: usize,
    pub equipment: ArrayShape,
    /// The equipment element's own tag and the fields this module writes.
    pub equip_elem_tag: u32,
    pub equip_item_soid: usize,
    pub equip_content_index: usize,
    /// ⭐ Element field `[3]` - the art-arrangement u16 `FUN_0054CF70` reads at `+0x1F4 + 64*slot`.
    pub equip_art_arrangement: usize,
    /// Element field `[4]` - the 8 × u16 extra-arrangement array (`0x80800F9D`).
    pub equip_extra_arrangements: usize,
    pub equip_extra_count: u32,
    pub stats: ArrayShape,
    pub stat_elem_tag: u32,
    pub stat_key: usize,
    pub stat_value: usize,
    pub stats_siblings: [usize; 3],
}

fn eb(e: impl std::fmt::Display) -> OracleError {
    OracleError::Eboot(e.to_string())
}

fn desc<'a>(idx: &'a Index, tag: u32) -> Result<&'a Descriptor> {
    idx.get(tag).ok_or(OracleError::UnknownTag(tag))
}

fn field(image: &Image, idx: &Index, tag: u32, i: usize) -> Result<Field> {
    let d = desc(idx, tag)?;
    descriptors::fields(image, d.anchor, d.field_count)
        .get(i)
        .and_then(|(_, f)| *f)
        .ok_or_else(|| OracleError::Shape(format!("{tag:#010x} has no field [{i}]")))
}

fn child(image: &Image, idx: &Index, tag: u32, i: usize) -> Result<(Field, u32)> {
    let f = field(image, idx, tag, i)?;
    if f.child_tag == LEAF {
        return Err(OracleError::Shape(format!(
            "{tag:#010x} field [{i}] is a leaf, expected a struct"
        )));
    }
    Ok((f, f.child_tag))
}

/// The container's own element count and element size.
///
/// ⚠ Read from `anchor+0x20` / `anchor+0x28`. The field record's offset column carries the **element
/// stride** for `kind 0x1`, and reading it as a C offset is the eight-byte error `crate::profile`
/// documents - here it would be a *sixty-four*-byte one.
fn array_at(image: &Image, idx: &Index, tag: u32, base: usize) -> Result<ArrayShape> {
    let d = desc(idx, tag)?;
    let count = image.u32(d.anchor + 0x20).map_err(eb)?;
    let elem_size = image.u32(d.anchor + 0x28).map_err(eb)?;
    if u64::from(count) * u64::from(elem_size) != u64::from(d.struct_size) {
        return Err(OracleError::Shape(format!(
            "{tag:#010x}: count {count} × element {elem_size} != struct size {} - the array does not \
             tile its container, so one of the three numbers is misread",
            d.struct_size
        )));
    }
    Ok(ArrayShape { tag, base, count, elem_size })
}

/// Walk `0x80801954`'s tree and produce the offsets this module writes to.
///
/// **This is the control on every constant above.** Nothing here is transcribed from a document; if
/// the eboot disagrees, [`controls`] fails and exits 2.
pub fn derive_layout(image: &Image, idx: &Index) -> Result<Layout> {
    let card = desc(idx, TAG)?.clone();

    let soid = field(image, idx, TAG, 0)?.offset as usize;
    let identity = child(image, idx, TAG, 1)?.0.offset as usize;
    let customization = child(image, idx, TAG, 2)?.0.offset as usize;

    // [4] +0x68 -> 0x80801855, 2568 B, 16 fields - everything below is relative to it.
    let (blk_f, blk_tag) = child(image, idx, TAG, 4)?;
    let stats_block = blk_f.offset as usize;

    // [0] u32 LEVEL, [2] f32 the derived level 0xA1151C floors.
    let level_f = field(image, idx, blk_tag, 0)?;
    if level_f.kind != 0x105 {
        return Err(OracleError::Shape(format!(
            "{blk_tag:#010x} field [0] is kind {:#x}, expected the 0x105 plain u32 LEVEL",
            level_f.kind
        )));
    }
    let derived_f = field(image, idx, blk_tag, 2)?;
    if derived_f.kind != 0x10b {
        return Err(OracleError::Shape(format!(
            "{blk_tag:#010x} field [2] is kind {:#x}, expected the 0x10b float 0x161550 reads",
            derived_f.kind
        )));
    }

    // [6] +0x1E8 -> 0x808045D9 (1280 B) = 20 × 0x8080183C (64 B).
    let (eq_f, eq_tag) = child(image, idx, blk_tag, 6)?;
    let equipment = array_at(image, idx, eq_tag, stats_block + eq_f.offset as usize)?;
    let (_, eq_elem_tag) = child(image, idx, eq_tag, 0)?;
    let eq_soid_f = field(image, idx, eq_elem_tag, 0)?;
    let eq_index_f = field(image, idx, eq_elem_tag, 1)?;
    if eq_soid_f.kind != 0x106 || eq_index_f.kind != 0x104 {
        return Err(OracleError::Shape(format!(
            "{eq_elem_tag:#010x} fields [0]/[1] are kinds {:#x}/{:#x}, expected 0x106 u64 + 0x104 \
             u16 - 0xA11627 reads element+0x08 as the content index",
            eq_soid_f.kind, eq_index_f.kind
        )));
    }
    // [3] the art-arrangement u16, [4] the 8 × u16 extra-arrangement array. The kind checks are
    // what turn an off-by-one field index into a refusal instead of a plausible offset.
    let eq_art_f = field(image, idx, eq_elem_tag, 3)?;
    if eq_art_f.kind != 0x104 {
        return Err(OracleError::Shape(format!(
            "{eq_elem_tag:#010x} field [3] is kind {:#x}, expected the 0x104 u16 art-arrangement \
             index FUN_0054CF70 reads at block+0x1F4+64*slot",
            eq_art_f.kind
        )));
    }
    let (eq_extra_f, eq_extra_tag) = child(image, idx, eq_elem_tag, 4)?;
    let extra = array_at(image, idx, eq_extra_tag, eq_extra_f.offset as usize)?;
    if extra.elem_size != 2 {
        return Err(OracleError::Shape(format!(
            "{eq_extra_tag:#010x} elements are {} bytes, expected u16 - FUN_0054E6C0 walks \
             param_7 + i*2 for i in 0..8",
            extra.elem_size
        )));
    }

    // [12] +0x808 -> 0x80804703 (128 B) = 16 × 0x808017E3 (8 B). [13]/[14]/[15] are the siblings.
    let (st_f, st_tag) = child(image, idx, blk_tag, 12)?;
    let stats = array_at(image, idx, st_tag, stats_block + st_f.offset as usize)?;
    let (_, st_elem_tag) = child(image, idx, st_tag, 0)?;
    let st_key_f = field(image, idx, st_elem_tag, 0)?;
    let st_val_f = field(image, idx, st_elem_tag, 1)?;
    if st_key_f.kind != 0x103 || st_val_f.kind != 0x105 {
        return Err(OracleError::Shape(format!(
            "{st_elem_tag:#010x} fields [0]/[1] are kinds {:#x}/{:#x}, expected 0x103 u8 + 0x105 u32",
            st_key_f.kind, st_val_f.kind
        )));
    }
    let mut siblings = [0usize; 3];
    for (n, i) in [13usize, 14, 15].iter().enumerate() {
        let (f, tag) = child(image, idx, blk_tag, *i)?;
        if tag != st_tag {
            return Err(OracleError::Shape(format!(
                "{blk_tag:#010x} field [{i}] is {tag:#010x}, expected the same {st_tag:#010x} the \
                 stat table uses"
            )));
        }
        siblings[n] = stats_block + f.offset as usize;
    }

    Ok(Layout {
        struct_size: card.struct_size,
        soid,
        identity,
        customization,
        stats_block,
        level: stats_block + level_f.offset as usize,
        derived_level: stats_block + derived_f.offset as usize,
        equipment,
        equip_elem_tag: eq_elem_tag,
        equip_item_soid: eq_soid_f.offset as usize,
        equip_content_index: eq_index_f.offset as usize,
        equip_art_arrangement: eq_art_f.offset as usize,
        equip_extra_arrangements: eq_extra_f.offset as usize,
        equip_extra_count: extra.count,
        stats,
        stat_elem_tag: st_elem_tag,
        stat_key: st_key_f.offset as usize,
        stat_value: st_val_f.offset as usize,
        stats_siblings: siblings,
    })
}

/// The layout this module's constants declare - what [`derive_layout`] must reproduce.
pub fn declared_layout() -> Layout {
    Layout {
        struct_size: STRUCT_SIZE as u32,
        soid: OFF_SOID,
        identity: OFF_IDENTITY,
        customization: OFF_CUSTOMIZATION,
        stats_block: OFF_STATS_BLOCK,
        level: OFF_LEVEL,
        derived_level: OFF_DERIVED_LEVEL,
        equipment: ArrayShape {
            tag: 0x8080_45D9,
            base: OFF_EQUIPMENT,
            count: EQUIPMENT_SLOTS as u32,
            elem_size: EQUIPMENT_STRIDE as u32,
        },
        equip_elem_tag: 0x8080_183C,
        equip_item_soid: EQUIP_ITEM_SOID,
        equip_content_index: EQUIP_CONTENT_INDEX,
        equip_art_arrangement: EQUIP_ART_ARRANGEMENT,
        equip_extra_arrangements: EQUIP_EXTRA_ARRANGEMENTS,
        equip_extra_count: EXTRA_ARRANGEMENT_SLOTS as u32,
        stats: ArrayShape {
            tag: 0x8080_4703,
            base: OFF_STATS,
            count: STAT_SLOTS as u32,
            elem_size: STAT_STRIDE as u32,
        },
        stat_elem_tag: 0x8080_17E3,
        stat_key: 0,
        stat_value: 4,
        stats_siblings: OFF_STATS_SIBLINGS,
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Controls
// ─────────────────────────────────────────────────────────────────────────────────────────────────

pub use crate::profile::Control;

fn ok(name: &'static str, detail: impl Into<String>) -> Control {
    Control { name, pass: true, detail: detail.into() }
}
fn no(name: &'static str, detail: impl Into<String>) -> Control {
    Control { name, pass: false, detail: detail.into() }
}

/// The offset the codec must refuse a 32-byte equipment stride at: element 0 `+0x29` is a padding
/// byte inside the 4-byte sub-element of `0x808046BC` (`{u8 @+0, u16 @+2}`), so no field covers it.
pub const EQUIP_STRIDE32_REFUSAL: usize = 0x279;
/// The offset the codec must refuse a `+4`-shifted stat table at: `{u8 @+0, u32 @+4}` leaves element
/// bytes `+1..+3` uncovered, and a shifted entry 0's value spills into `+0x879`.
///
/// ⚠ **The refusal offset depends on the probe VALUE**, not only on the shift: `round_trip` reports
/// the *first* differing byte, so a value whose second byte happens to be zero slides the refusal to
/// the next entry. [`STAT_PROBE_BASE`] is chosen so that byte 1 is always `0x01`. A control whose
/// expected failure address moves with the payload is a control that will one day pass by accident.
pub const STAT_SHIFT4_REFUSAL: usize = 0x879;
/// `0x190` - every probe value is `STAT_PROBE_BASE + i`, so byte 1 is `0x01` for all sixteen.
pub const STAT_PROBE_BASE: u32 = 400;

/// Every control this module ships. **Exit 2 if any `pass` is false.**
///
/// | control | the axis it tests |
/// |---|---|
/// | `layout` | a constant here drifting from the eboot |
/// | `identity-agrees-with-profile` | the card and the profile claiming different `{race,gender,class}` |
/// | `level-is-a-plain-u32` | `C+0x68` is a real `u32` field, not padding |
/// | `derived-level-is-a-real-float` | `C+0x70` survives a *fractional* float - so `kind 0x10b` is a full f32 and the offset is right |
/// | `equipment-stride-64` | 20 content indices at `+0x250 + 64i + 8` - **positive** |
/// | `equipment-stride-32-NEGATIVE` | ⭐ the same written at stride 32 **must be REFUSED** at `+0x279` |
/// | `stat-table-base` | 16 entries at `+0x870` - **positive** |
/// | `stat-table-base-NEGATIVE` | ⭐ the same written at `+0x874` **must be REFUSED** at `+0x879` |
/// | `empty-markers-survive` | `0xFFFF` content indices and `0xFF` stat keys decode back unchanged, not zeroed |
/// | `stat-sentinel-is-required` | ⭐ a zero-filled stat table would make `0x266740` write `out[0] = 0` **after** Light - asserted on the built object |
/// | `baseline-card-is-empty` | the fixture we are replacing really does have zero equipment and zero stats |
/// | `built-character` | the shipped spec round-trips and `verify` accepts it |
/// | `strictly-more-than-identity-only` | ⭐ monotonicity: the new card's non-zero set must be a strict superset of `{SOID, identity}` |
pub fn controls(baseline: Option<&Path>) -> Result<Vec<Control>> {
    let mut out = Vec::new();

    // 1. Layout, re-derived.
    let image = Image::open_default().map_err(eb)?;
    let idx = descriptors::index(&image).map_err(eb)?;
    let derived = derive_layout(&image, &idx)?;
    let declared = declared_layout();
    out.push(if derived == declared {
        ok(
            "layout",
            format!(
                "{} B; identity +{:#x}; level +{:#x}; derived-level float +{:#x}; equipment {}×{} \
                 @+{:#x} ({:#010x}, soid +{:#x}, content index +{:#x}); stats {}×{} @+{:#x} \
                 ({:#010x}, key +{:#x}, value +{:#x}); siblings {:#x?}",
                derived.struct_size,
                derived.identity,
                derived.level,
                derived.derived_level,
                derived.equipment.count,
                derived.equipment.elem_size,
                derived.equipment.base,
                derived.equip_elem_tag,
                derived.equip_item_soid,
                derived.equip_content_index,
                derived.stats.count,
                derived.stats.elem_size,
                derived.stats.base,
                derived.stat_elem_tag,
                derived.stat_key,
                derived.stat_value,
                derived.stats_siblings
            ),
        )
    } else {
        no("layout", format!("eboot says {derived:#?}\ncode says {declared:#?}"))
    });

    // 2. ⭐ The card and the profile must claim the same Guardian.
    out.push(if DEFAULT_IDENTITY == crate::profile::DEFAULT_IDENTITY {
        ok(
            "identity-agrees-with-profile",
            format!(
                "{{race, gender, class}} = {DEFAULT_IDENTITY:?} in BOTH the family-3 card and the \
                 family-4/1 profile"
            ),
        )
    } else {
        no(
            "identity-agrees-with-profile",
            format!(
                "card says {DEFAULT_IDENTITY:?}, profile says {:?} - two objects describing one \
                 Guardian differently is a bug the client cannot report",
                crate::profile::DEFAULT_IDENTITY
            ),
        )
    });

    let mut oracle = ObjectOracle::new(TAG)?;
    let bare = CharacterSpec {
        level: 0,
        derived_level: 0.0,
        stats: Vec::new(),
        equipment: Vec::new(),
        sentinel_stat_keys: false,
        sentinel_all_stat_tables: false,
        empty_equipment_indices: false,
        ..CharacterSpec::default()
    };

    // 3. Level is a plain u32 at +0x68.
    let mut o = bare.to_object()?;
    o[OFF_LEVEL..OFF_LEVEL + 4].copy_from_slice(&LEVEL_CAP.to_le_bytes());
    out.push(match oracle.round_trip(&o, 0x4000) {
        Ok((_, d)) if u32::from_le_bytes(d.object[OFF_LEVEL..OFF_LEVEL + 4].try_into().unwrap())
            == LEVEL_CAP =>
        {
            ok("level-is-a-plain-u32", format!("+{OFF_LEVEL:#x} carries {LEVEL_CAP} through the codec"))
        }
        Ok((_, d)) => no(
            "level-is-a-plain-u32",
            format!("+{OFF_LEVEL:#x} decoded {:#x}", d.qword(OFF_LEVEL)),
        ),
        Err(e) => no("level-is-a-plain-u32", e.to_string()),
    });

    // 4. The derived-level float. A FRACTIONAL value, so a lossy or absent field cannot pass.
    let probe = 40.5f32;
    let mut o = bare.to_object()?;
    o[OFF_DERIVED_LEVEL..OFF_DERIVED_LEVEL + 4].copy_from_slice(&probe.to_le_bytes());
    out.push(match oracle.round_trip(&o, 0x4000) {
        Ok((_, d)) => {
            let got = f32::from_bits(u32::from_le_bytes(
                d.object[OFF_DERIVED_LEVEL..OFF_DERIVED_LEVEL + 4].try_into().unwrap(),
            ));
            if got.to_bits() == probe.to_bits() {
                ok(
                    "derived-level-is-a-real-float",
                    format!("+{OFF_DERIVED_LEVEL:#x} carries {probe} exactly - kind 0x10b is a full f32"),
                )
            } else {
                no("derived-level-is-a-real-float", format!("wrote {probe}, decoded {got}"))
            }
        }
        Err(e) => no("derived-level-is-a-real-float", e.to_string()),
    });

    // 5. Equipment at stride 64 - the positive half.
    let mut o = bare.to_object()?;
    for i in 0..EQUIPMENT_SLOTS {
        let at = OFF_EQUIPMENT + i * EQUIPMENT_STRIDE + EQUIP_CONTENT_INDEX;
        o[at..at + 2].copy_from_slice(&(0x1234u16 + i as u16).to_le_bytes());
    }
    out.push(match oracle.round_trip(&o, 0x4000) {
        Ok(_) => ok(
            "equipment-stride-64",
            format!(
                "{EQUIPMENT_SLOTS} content indices at +{OFF_EQUIPMENT:#x} + \
                 {EQUIPMENT_STRIDE}i + {EQUIP_CONTENT_INDEX} round-trip IDENTICAL"
            ),
        ),
        Err(e) => no("equipment-stride-64", e.to_string()),
    });

    // 6. ⭐ The SAME writes at stride 32 - the reading `descriptors`' offset column invites.
    //    They MUST be refused, at +0x279: element 0 +0x29 is padding inside 0x80801704's {u8,u16}.
    let mut o = bare.to_object()?;
    for i in 0..EQUIPMENT_SLOTS {
        let at = OFF_EQUIPMENT + i * 32 + EQUIP_CONTENT_INDEX;
        o[at..at + 2].copy_from_slice(&(0x1234u16 + i as u16).to_le_bytes());
    }
    out.push(match oracle.round_trip(&o, 0x4000) {
        Err(OracleError::Shape(m)) if m.contains(&format!("+{EQUIP_STRIDE32_REFUSAL:#05x}:")) => ok(
            "equipment-stride-32-NEGATIVE",
            format!("a 32-byte stride is correctly REFUSED - {m}"),
        ),
        Ok(_) => no(
            "equipment-stride-32-NEGATIVE",
            format!(
                "a 32-byte equipment stride round-tripped. It must not: element 0 +0x29 is padding \
                 inside 0x80801704's {{u8, u16}}. Either the schema moved or this control has \
                 stopped testing anything."
            ),
        ),
        Err(e) => no("equipment-stride-32-NEGATIVE", format!("refused, but not where expected: {e}")),
    });

    // 6b. ⭐ The ART-ARRANGEMENT field: the pinned Gjallarhorn arrangement in slot 3 must survive
    //     the codec at element +0x0C - this is the u16 that decides whether gear DRAWS.
    let mut o = bare.to_object()?;
    let at3 = OFF_EQUIPMENT + 3 * EQUIPMENT_STRIDE + EQUIP_ART_ARRANGEMENT;
    o[at3..at3 + 2]
        .copy_from_slice(&crate::item_index::PINNED_ARRANGEMENT.1.to_le_bytes());
    out.push(match oracle.round_trip(&o, 0x4000) {
        Ok((_, d))
            if u16::from_le_bytes(d.object[at3..at3 + 2].try_into().unwrap())
                == crate::item_index::PINNED_ARRANGEMENT.1 =>
        {
            ok("art-arrangement-round-trip", format!(
                "arrangement {} at +{at3:#x} (element +{EQUIP_ART_ARRANGEMENT:#04x}) survives the \
                 codec - the field FUN_0054CF70 reads at block+0x1F4+64*slot is servable",
                crate::item_index::PINNED_ARRANGEMENT.1))
        }
        Ok((_, d)) => no("art-arrangement-round-trip", format!(
            "wrote {}, decoded {}", crate::item_index::PINNED_ARRANGEMENT.1,
            u16::from_le_bytes(d.object[at3..at3 + 2].try_into().unwrap()))),
        Err(e) => no("art-arrangement-round-trip", e.to_string()),
    });

    //     NEGATIVE: element +0x1E - the two bytes between the extras array (field [4], 16 B ending
    //     at +0x1E) and field [5] @+0x20 - is NOT covered by any field. If it ever round-trips,
    //     the extras array is not where this module says it is.
    let gap = OFF_EQUIPMENT + EQUIP_EXTRA_ARRANGEMENTS + EXTRA_ARRANGEMENT_SLOTS * 2;
    let mut o = bare.to_object()?;
    o[gap] = 1;
    out.push(match oracle.round_trip(&o, 0x4000) {
        Err(OracleError::Shape(m)) if m.contains(&format!("+{gap:#05x}:")) => ok(
            "art-extras-extent-NEGATIVE",
            format!("element +0x1E (card +{gap:#x}) is correctly NOT a field - {m}. The 8-slot \
                     extras array really ends at element +0x1E")),
        Ok(_) => no("art-extras-extent-NEGATIVE", format!(
            "a byte at +{gap:#x} round-tripped - the extras array is not 8 × u16 at +0x0E and \
             every sentinel this module writes may be landing in the wrong place")),
        Err(e) => no("art-extras-extent-NEGATIVE", format!("refused, but not where expected: {e}")),
    });

    // 7. The stat table at +0x870 - the positive half.
    let mut o = bare.to_object()?;
    for i in 0..STAT_SLOTS {
        let at = OFF_STATS + i * STAT_STRIDE;
        o[at] = EMPTY_STAT_KEY;
        o[at + 4..at + 8].copy_from_slice(&(STAT_PROBE_BASE + i as u32).to_le_bytes());
    }
    o[OFF_STATS] = LIGHT_STAT_INDEX;
    out.push(match oracle.round_trip(&o, 0x4000) {
        Ok(_) => ok(
            "stat-table-base",
            format!("{STAT_SLOTS} entries at +{OFF_STATS:#x} round-trip IDENTICAL"),
        ),
        Err(e) => no("stat-table-base", e.to_string()),
    });

    // 8. ⭐ The same table four bytes later - REFUSED at +0x879, the element's uncovered +1..+3.
    let mut o = bare.to_object()?;
    for i in 0..STAT_SLOTS {
        let at = OFF_STATS + 4 + i * STAT_STRIDE;
        o[at] = EMPTY_STAT_KEY;
        o[at + 4..at + 8].copy_from_slice(&(STAT_PROBE_BASE + i as u32).to_le_bytes());
    }
    o[OFF_STATS + 4] = LIGHT_STAT_INDEX;
    out.push(match oracle.round_trip(&o, 0x4000) {
        Err(OracleError::Shape(m)) if m.contains(&format!("+{STAT_SHIFT4_REFUSAL:#05x}:")) => ok(
            "stat-table-base-NEGATIVE",
            format!("+{:#x} correctly REFUSED - {m}", OFF_STATS + 4),
        ),
        Ok(_) => no(
            "stat-table-base-NEGATIVE",
            format!(
                "a 16-entry stat table at +{:#x} round-tripped. It must not: {{u8 @+0, u32 @+4}} \
                 leaves element bytes +1..+3 uncovered.",
                OFF_STATS + 4
            ),
        ),
        Err(e) => no("stat-table-base-NEGATIVE", format!("refused, but not where expected: {e}")),
    });

    // 9/10/11. The shipped spec, its markers, and the thing that makes the markers necessary.
    let spec = CharacterSpec::default();
    match build(&mut oracle, &spec) {
        Ok(b) => {
            let d = &b.decoded;
            let markers_ok = (spec.equipment.len()..EQUIPMENT_SLOTS).all(|i| {
                let at = OFF_EQUIPMENT + i * EQUIPMENT_STRIDE + EQUIP_CONTENT_INDEX;
                u16::from_le_bytes(d.object[at..at + 2].try_into().unwrap()) == EMPTY_CONTENT_INDEX
            }) && (spec.stats.len()..STAT_SLOTS)
                .all(|i| d.object[OFF_STATS + i * STAT_STRIDE] == EMPTY_STAT_KEY);
            out.push(if markers_ok {
                ok(
                    "empty-markers-survive",
                    format!(
                        "{} unused content indices decode {EMPTY_CONTENT_INDEX:#06x} and {} unused \
                         stat keys decode {EMPTY_STAT_KEY:#04x} - the codec does not flatten them to 0",
                        EQUIPMENT_SLOTS - spec.equipment.len(),
                        STAT_SLOTS - spec.stats.len()
                    ),
                )
            } else {
                no(
                    "empty-markers-survive",
                    "an empty marker decoded as something other than its sentinel".to_string(),
                )
            });

            // ⭐ Simulate 0x266740 over the built table, both ways. The zero-filled variant MUST
            //    lose Light; if it does not, this control has stopped testing anything.
            let sim = |keys_zero: bool| -> u32 {
                let mut slots = [0u32; 64];
                for i in 0..STAT_SLOTS {
                    let at = OFF_STATS + i * STAT_STRIDE;
                    let k = if keys_zero && i >= spec.stats.len() { 0u8 } else { d.object[at] };
                    if k == EMPTY_STAT_KEY {
                        continue;
                    }
                    let v = u32::from_le_bytes(d.object[at + 4..at + 8].try_into().unwrap());
                    slots[k as usize] = v;
                }
                slots[LIGHT_STAT_INDEX as usize]
            };
            let want_light = spec
                .stats
                .iter()
                .find(|(k, _)| *k == LIGHT_STAT_INDEX)
                .map(|(_, v)| *v)
                .unwrap_or(0);
            let (kept, lost) = (sim(false), sim(true));
            out.push(if kept == want_light && lost == 0 && want_light != 0 {
                ok(
                    "stat-sentinel-is-required",
                    format!(
                        "0x266740 simulated over the built table: with 0xFF markers out[{}] = {kept}; \
                         with the keys zeroed it is {lost} - the sentinel is what keeps Light",
                        LIGHT_STAT_INDEX
                    ),
                )
            } else {
                no(
                    "stat-sentinel-is-required",
                    format!(
                        "simulated 0x266740 gives {kept} with markers and {lost} without. Expected \
                         {want_light} and 0 - if both match the control no longer demonstrates anything"
                    ),
                )
            });

            out.push(ok(
                "built-character",
                format!(
                    "{} B / {} bits; {} non-zero qwords; level {} @+{OFF_LEVEL:#x}; \
                     stat[0] {{{}, {}}} @+{OFF_STATS:#x}",
                    b.payload.len(),
                    b.bits,
                    b.nonzero().len(),
                    spec.level,
                    spec.stats.first().map(|s| s.0).unwrap_or(0),
                    spec.stats.first().map(|s| s.1).unwrap_or(0)
                ),
            ));

            let got: BTreeSet<usize> = d.nonzero_offsets().into_iter().collect();
            let want: BTreeSet<usize> = [OFF_SOID, OFF_IDENTITY].into_iter().collect();
            out.push(if want.is_subset(&got) && got.len() > want.len() {
                ok(
                    "strictly-more-than-identity-only",
                    format!(
                        "{} non-zero qwords vs the identity-only card's {} - and BOTH of its fields \
                         survive",
                        got.len(),
                        want.len()
                    ),
                )
            } else {
                no(
                    "strictly-more-than-identity-only",
                    format!(
                        "non-zero qwords {got:?} is not a strict superset of {want:?} - a populated \
                         card that drops the SOID or the identity is a regression, not a fix"
                    ),
                )
            });
        }
        Err(e) => out.push(no("built-character", e.to_string())),
    }

    // 12. The fixture we are replacing really is empty where it matters.
    if let Some(p) = baseline {
        match std::fs::read(p).map_err(|e| eb(format!("{}: {e}", p.display()))) {
            Ok(raw) => match oracle.decode(&raw) {
                Ok(d) => {
                    let eq_zero = d.object
                        [OFF_EQUIPMENT..OFF_EQUIPMENT + EQUIPMENT_SLOTS * EQUIPMENT_STRIDE]
                        .iter()
                        .all(|b| *b == 0);
                    let st_zero = d.object[OFF_STATS..OFF_STATS + STAT_SLOTS * STAT_STRIDE]
                        .iter()
                        .all(|b| *b == 0);
                    out.push(if eq_zero && st_zero {
                        ok(
                            "baseline-card-is-empty",
                            format!(
                                "{}: equipment +{OFF_EQUIPMENT:#x}..+{:#x} and stats \
                                 +{OFF_STATS:#x}..+{:#x} are ENTIRELY ZERO - no gear, no Light",
                                p.display(),
                                OFF_EQUIPMENT + EQUIPMENT_SLOTS * EQUIPMENT_STRIDE,
                                OFF_STATS + STAT_SLOTS * STAT_STRIDE
                            ),
                        )
                    } else {
                        no(
                            "baseline-card-is-empty",
                            format!(
                                "{}: equipment zero = {eq_zero}, stats zero = {st_zero} - expected \
                                 both. This control asserts the claim the module is answering.",
                                p.display()
                            ),
                        )
                    });
                }
                Err(e) => out.push(no("baseline-card-is-empty", e.to_string())),
            },
            Err(e) => out.push(no("baseline-card-is-empty", e.to_string())),
        }
    }

    // 13. ⭐ The emblem, and the NEGATIVE that keeps the index table honest.
    //
    // `emblem-index-is-measured` fails if the shipped table stops resolving the emblem's hash;
    // `not-an-item-is-REFUSED` fails if it ever starts resolving a hash that is NOT an item
    // (a progression, a stat, a bucket). The second is the one that can rot silently: a table
    // regenerated from the wrong content entry would still be a table of plausible integers.
    match crate::item_index::IndexTable::load_default() {
        Ok(t) => {
            match t.index_of(crate::item_index::DEFAULT_EMBLEM_HASH) {
                Some(i) => out.push(ok("emblem-index-is-measured", format!(
                    "slot {EMBLEM_SLOT} = investment_globals->vtbl[0x160]()[0x18] (replayed 0x0C); \
                     {:#010x} -> content index {i}, world tag {:#010x}; card offset +{:#x}",
                    crate::item_index::DEFAULT_EMBLEM_HASH,
                    t.world_tag_of(crate::item_index::DEFAULT_EMBLEM_HASH).unwrap_or(0),
                    OFF_EQUIPMENT + EMBLEM_SLOT * EQUIPMENT_STRIDE + EQUIP_CONTENT_INDEX))),
                None => out.push(no("emblem-index-is-measured", format!(
                    "{:#010x} is not in {}", crate::item_index::DEFAULT_EMBLEM_HASH,
                    t.path.display()))),
            }
            let leaked: Vec<&str> = crate::item_index::NOT_ITEMS.iter()
                .filter(|(h, _)| t.require(*h).is_ok())
                .map(|(_, n)| *n)
                .collect();
            out.push(if leaked.is_empty() {
                ok("not-an-item-is-REFUSED", format!(
                    "require() refuses all of {:?} - the table is the ITEM hash space, so an index \
                     read out of it names an item and not something that merely hashes",
                    crate::item_index::NOT_ITEMS.map(|(_, n)| n)))
            } else {
                no("not-an-item-is-REFUSED", format!(
                    "these resolved and must not: {leaked:?} - the shipped table is not the item space"))
            });
        }
        Err(e) => out.push(no("emblem-index-is-measured", e.to_string())),
    }

    out.extend(peer_controls());
    Ok(out)
}

/// ⭐⭐ The controls on the **family-0 peer view** - the object the character screen reads.
///
/// Three of them, and the third is **negative**: the card and the peer view must carry a
/// *byte-identical* shared block, so a change that reaches one carrier and not the other fails
/// here rather than on screen.
pub fn peer_controls() -> Vec<Control> {
    let mut out = Vec::new();

    // 1. The two carriers' geometry, re-derived from the eboot. Nothing transcribed.
    match Image::open_default().and_then(|i| descriptors::index(&i).map(|x| (i, x))) {
        Ok((image, idx)) => {
            let card = idx.get(TAG).cloned();
            let peer = idx.get(PEER_TAG).cloned();
            let card_blk = field(&image, &idx, TAG, 4).map(|f| f.offset as usize);
            let peer_blk = field(&image, &idx, PEER_TAG, 3).map(|f| f.offset as usize);
            let card_child = child(&image, &idx, TAG, 4).map(|(_, t)| t);
            let peer_child = child(&image, &idx, PEER_TAG, 3).map(|(_, t)| t);
            match (card, peer, card_blk, peer_blk, card_child, peer_child) {
                (Some(c), Some(p), Ok(cb), Ok(pb), Ok(ct), Ok(pt))
                    if ct == pt
                        && cb == OFF_STATS_BLOCK
                        && pb == PEER_BLOCK
                        && c.struct_size as usize == STRUCT_SIZE
                        && p.struct_size as usize == PEER_STRUCT_SIZE =>
                {
                    out.push(ok(
                        "peer-carrier-from-eboot",
                        format!(
                            "{TAG:#010x} {} B carries {ct:#010x} at +{cb:#05x}; {PEER_TAG:#010x} \
                             {} B carries the SAME child at +{pb:#05x} - one block, two carriers",
                            c.struct_size, p.struct_size
                        ),
                    ));
                }
                (c, p, cb, pb, ct, pt) => out.push(no(
                    "peer-carrier-from-eboot",
                    format!(
                        "card {:?}/{cb:?}/{ct:?} vs peer {:?}/{pb:?}/{pt:?} - the eboot disagrees \
                         with this module's constants",
                        c.map(|x| x.struct_size),
                        p.map(|x| x.struct_size)
                    ),
                )),
            }
        }
        Err(e) => out.push(no("peer-carrier-from-eboot", e.to_string())),
    }

    // 2. The round trip, through the client's own codec, on a populated spec.
    let spec = CharacterSpec { level: 40, ..CharacterSpec::default() };
    let built = ObjectOracle::new(PEER_TAG).and_then(|mut o| build_peer(&mut o, &spec));
    match &built {
        Ok(b) => out.push(ok(
            "peer-round-trip",
            format!(
                "{PEER_TAG:#010x} {} B / {} bits, object IDENTICAL; level lands at +{:#05x} and the \
                 stat table at +{:#05x}",
                b.payload.len(),
                b.bits,
                PEER_BLOCK,
                PEER_BLOCK + (OFF_STATS - OFF_STATS_BLOCK)
            ),
        )),
        Err(e) => out.push(no("peer-round-trip", e.to_string())),
    }

    // 3. ⭐ NEGATIVE-shaped: the block must be byte-identical in both carriers.
    match (spec.to_object(), to_peer_object(&spec)) {
        (Ok(card), Ok(peer)) => {
            let a = &card[OFF_STATS_BLOCK..OFF_STATS_BLOCK + BLOCK_SIZE];
            let b = &peer[PEER_BLOCK..PEER_BLOCK + BLOCK_SIZE];
            let differ: Vec<usize> = (0..BLOCK_SIZE).filter(|&i| a[i] != b[i]).collect();
            // And the *shifted* comparison must FAIL - otherwise the block is all zeros and the
            // equality above proves nothing.
            let shifted_same = card[OFF_STATS_BLOCK..OFF_STATS_BLOCK + BLOCK_SIZE - 1]
                == peer[PEER_BLOCK + 1..PEER_BLOCK + BLOCK_SIZE];
            if differ.is_empty() && !shifted_same {
                out.push(ok(
                    "peer-block-identical-and-nonempty",
                    format!(
                        "all {BLOCK_SIZE} bytes match at +{OFF_STATS_BLOCK:#05x} / +{PEER_BLOCK:#05x}, \
                         and the one-byte-shifted comparison DIFFERS - so the match is content, not \
                         a field of zeros"
                    ),
                ));
            } else if !differ.is_empty() {
                out.push(no(
                    "peer-block-identical-and-nonempty",
                    format!("{} block bytes differ, first at +{:#x}", differ.len(), differ[0]),
                ));
            } else {
                out.push(no(
                    "peer-block-identical-and-nonempty",
                    "the shifted comparison also matched - the block is uniform and this control \
                     is decoration",
                ));
            }
        }
        (a, b) => out.push(no(
            "peer-block-identical-and-nonempty",
            format!(
                "could not build both carriers: card {}, peer {}",
                a.map(|v| format!("{} B", v.len())).unwrap_or_else(|e| e.to_string()),
                b.map(|v| format!("{} B", v.len())).unwrap_or_else(|e| e.to_string())
            ),
        )),
    }

    // 4. ⭐ THE QUEUEZ LINK-KEY IDENTITY - the porcupine guard.
    // The peer view's +0x00 is the QueueZ envelope key, NOT a free SOID: the responder headers f0t1
    // by its filename key and the client rejects the object (porcupine) if the header key and the
    // payload's +0x00 disagree. `to_peer_object` must therefore write `spec.soid` (which build-peer
    // sets to the card/link key) into +0x00 verbatim. This is the same rule roster.rs §52 enforces
    // for family 2 (card_key into 2/1's +0x00). INVERTED below with a deliberately wrong key.
    {
        let link_key = 0xD100_0000_0000_0501u64;
        let wrong_key = 0xD100_0000_0000_0101u64; // the exact pre-fix bug value
        let spec = CharacterSpec { soid: link_key, level: 40, ..CharacterSpec::default() };
        match to_peer_object(&spec) {
            Ok(peer) => {
                let plus0 = u64::from_le_bytes(peer[0x00..0x08].try_into().unwrap());
                if plus0 == link_key && plus0 != wrong_key {
                    out.push(ok(
                        "peer-link-key-at-plus0",
                        format!(
                            "f0t1 +0x00 = {plus0:#018x} == the link key spec.soid, and != the \
                             pre-fix CHARACTER_SOID {wrong_key:#018x} - so the QueueZ header key \
                             (= filename key) will match the payload and porcupine cannot fire"
                        ),
                    ));
                } else {
                    out.push(no(
                        "peer-link-key-at-plus0",
                        format!(
                            "f0t1 +0x00 = {plus0:#018x}, wanted the link key {link_key:#018x} - \
                             a header/payload key mismatch is exactly \
                             error.investment_server_object_queuez_id_mismatch_in_header_and_rsat_payload"
                        ),
                    ));
                }
            }
            Err(e) => out.push(no("peer-link-key-at-plus0", e.to_string())),
        }
    }

    out
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Equipment sourcing
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// Read the item-instance SOIDs out of the staged family-4/3 elements and turn them into equipment
/// slots with **empty** content indices - the pre-2026-08-09 shape, kept for bisecting.
///
/// ⚠ A slot with [`EMPTY_CONTENT_INDEX`] says *"this instance is equipped here"* and nothing about
/// what it looks like. Use [`equipment_from_inventory_with`] and a measured table.
pub fn equipment_from_inventory(dir: &Path) -> Result<Vec<EquipSlot>> {
    equipment_from_inventory_with(dir, None, None)
}

/// The same, with **measured content indices** and [`EMBLEM_SLOT`] left free.
///
/// Each staged element names its item by hash at `C+0x18`; [`crate::item_index::IndexTable::require`]
/// turns that into the `u16` the client keys the item by, and **refuses** a hash 01.29 content does
/// not know rather than inventing a number. Slot [`EMBLEM_SLOT`] is skipped because the client reads
/// exactly that slot's `+0x08` as the emblem (`0xA11627`); an item parked there renders as the
/// player's emblem.
///
/// `art` is `(arrangement table, class byte)` - it fills element `+0x0C` with the item's MEASURED
/// art-arrangement index, and ⭐ **places each item at its definition-named equipped ordinal**
/// (`def_client+0x18`, the table's `equip_slot` column). `0x269080` derefs
/// `character + 0x2B80 + slot*8` with that byte, so the consumer looks the item up **by ordinal,
/// not by file position** - file order (what every builder before 2026-08-10 used) makes every
/// item whose ordinal differs from its position resolve to the wrong entry.
///
/// `None`, or a missing row, writes [`crate::item_index::EMPTY_ARRANGEMENT`] - no geometry,
/// honestly, rather than arrangement #0 silently - and falls back to file-order placement.
pub fn equipment_from_inventory_with(
    dir: &Path,
    table: Option<&crate::item_index::IndexTable>,
    art: Option<(&crate::item_index::ArtTable, u8)>,
) -> Result<Vec<EquipSlot>> {
    let staged = crate::profile::inventory_items(dir)?;
    let mut out: Vec<EquipSlot> = if art.is_some() {
        vec![EquipSlot::EMPTY; EQUIPMENT_SLOTS]
    } else {
        Vec::new()
    };
    for it in staged {
        // ⛔⛔ **A TOOL THAT REGENERATES ITS OWN INPUT EATS ITS OWN OUTPUT** (`status/LESSONS.md`).
        // `+0x18` carried the item HASH only while `stage_loadout` was writing hashes into a field
        // the client reads as a LEVEL. A *rebuilt* element carries `40` there - and feeding that
        // to `require()` fails with "item hash 0x00000028 is not in the table", which reads like a
        // broken content table and is actually this function eating the fix.
        // ⭐ So take the element's OWN `+0x24` when it is a real content index, and fall back to
        // the hash table only for a legacy element (which still says `1`, 'Bronze Tier').
        let content_index = if it.content_index_field != EMPTY_CONTENT_INDEX
            && it.content_index_field != LEGACY_BRONZE_TIER_INDEX
        {
            it.content_index_field
        } else {
            match table {
                Some(t) => t.require(it.item_hash)?,
                None => EMPTY_CONTENT_INDEX,
            }
        };
        let art_arrangement = match (art, content_index) {
            (Some((t, class)), ci) if ci != EMPTY_CONTENT_INDEX => {
                t.arrangement_for(ci, class).unwrap_or(crate::item_index::EMPTY_ARRANGEMENT)
            }
            _ => crate::item_index::EMPTY_ARRANGEMENT,
        };
        // ⭐ Element +0x0A for the three weapon ordinals: the definition's weapon object index.
        // Without it every weapon resolves record 0 of the object table and spawns as a non-weapon.
        let weapon_object = match art {
            Some((t, _)) if t
                .equip_slot_for(content_index)
                .is_some_and(|s| WEAPON_ORDINALS.contains(&s)) =>
            {
                t.weapon_object_for(content_index).unwrap_or(0)
            }
            _ => 0,
        };
        let slot = EquipSlot { item_soid: it.soid, content_index, art_arrangement, weapon_object };

        if let Some((t, _)) = art {
            match t.equip_slot_for(content_index) {
                Some(s) if s < EQUIPMENT_SLOTS => {
                    if out[s] != EquipSlot::EMPTY {
                        return Err(OracleError::Shape(format!(
                            "two staged items claim equipped ordinal {s} (content indices {} and \
                             {content_index}) - the consumer 0x269080 can only see one of them",
                            out[s].content_index)));
                    }
                    if s == EMBLEM_SLOT {
                        return Err(OracleError::Shape(format!(
                            "content index {content_index} names equipped ordinal {EMBLEM_SLOT} - \
                             the emblem slot. An item there renders as the player's emblem")));
                    }
                    out[s] = slot;
                }
                _ => {
                    // No measured ordinal ⇒ the item CANNOT be placed honestly. It stays in
                    // family 4/3 and the backpack; parking it at a guessed ordinal would occupy
                    // another item's slot.
                    eprintln!(
                        "⚠ content index {content_index} has no measured equipped ordinal - NOT \
                         placed in the 20-slot equipment summary (it remains in the backpack)");
                }
            }
            continue;
        }

        // Legacy file-order placement (no art table): the pre-2026-08-10 shape, kept for bisecting.
        while out.len() == EMBLEM_SLOT {
            out.push(EquipSlot::EMPTY);
        }
        if out.len() >= EQUIPMENT_SLOTS {
            return Err(OracleError::Shape(format!(
                "more staged items than the {EQUIPMENT_SLOTS}-slot equipment summary holds \
                 (slot {EMBLEM_SLOT} is reserved for the emblem)"
            )));
        }
        out.push(slot);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_is_derived_from_the_eboot_not_transcribed() {
        if !crate::test_inputs::reference_eboot() {
            return;
        }
        let image = Image::open_reference().expect("pinned eboot");
        let idx = descriptors::index(&image).expect("descriptor index");
        assert_eq!(derive_layout(&image, &idx).expect("walk"), declared_layout());
    }

    #[test]
    fn the_spec_refuses_to_overflow_an_array() {
        let spec = CharacterSpec {
            stats: (0..STAT_SLOTS + 1).map(|i| (i as u8, 1)).collect(),
            ..Default::default()
        };
        assert!(matches!(spec.to_object(), Err(OracleError::Shape(_))));
        let spec = CharacterSpec {
            equipment: vec![EquipSlot::EMPTY; EQUIPMENT_SLOTS + 1],
            ..Default::default()
        };
        assert!(matches!(spec.to_object(), Err(OracleError::Shape(_))));
    }

    /// The sentinels are the whole point - assert them on the raw layout, before the codec.
    #[test]
    fn unused_slots_carry_their_sentinel_not_zero() {
        let o = CharacterSpec::default().to_object().unwrap();
        for i in 0..EQUIPMENT_SLOTS {
            let at = OFF_EQUIPMENT + i * EQUIPMENT_STRIDE + EQUIP_CONTENT_INDEX;
            assert_eq!(
                u16::from_le_bytes(o[at..at + 2].try_into().unwrap()),
                EMPTY_CONTENT_INDEX,
                "equipment slot {i}"
            );
        }
        assert_eq!(o[OFF_STATS], LIGHT_STAT_INDEX, "stat[0] is Light");
        assert_eq!(
            u32::from_le_bytes(o[OFF_STATS + 4..OFF_STATS + 8].try_into().unwrap()),
            crate::profile::LIGHT_ROI_CAP,
            "the default Light must be the RoI cap 385 - 400 is impossible and proves invention"
        );
        for i in 1..STAT_SLOTS {
            assert_eq!(o[OFF_STATS + i * STAT_STRIDE], EMPTY_STAT_KEY, "stat slot {i}");
        }
        // Historical compatibility: weapon tables stay untouched until explicitly populated.
        for base in OFF_STATS_SIBLINGS {
            assert!(o[base..base + STAT_SLOTS * STAT_STRIDE].iter().all(|b| *b == 0));
        }
    }

    #[test]
    fn the_card_and_the_profile_describe_one_guardian() {
        assert_eq!(CharacterSpec::default().identity, crate::profile::ProfileSpec::default().identity);
    }

    /// The whole control set, so `cargo test` and `d1-oracle character controls` cannot drift.
    #[test]
    fn controls_pass() {
        if !crate::test_inputs::selected_eboot() {
            return;
        }
        let baseline = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../capture-rig/characters/char-lvl40-probe.bin");
        let cs = controls(baseline.exists().then_some(baseline.as_path())).expect("controls ran");
        let failed: Vec<_> =
            cs.iter().filter(|c| !c.pass).map(|c| format!("{}: {}", c.name, c.detail)).collect();
        assert!(failed.is_empty(), "{failed:#?}");
    }
}
