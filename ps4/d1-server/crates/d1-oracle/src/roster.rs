// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Build the **orbit roster player card** - QueueZ family 2, types 0 and 1.
//!
//! `historicaldocs/design/orbit-hud-blank.md` measured, on the live client, three `cui::s_player_nameplate`s whose
//! level/light/emblem fields are **all zero** and three `cui::c_roster_contact_id_container`s reading
//! `{key0 = 0, key1 = 0, family = 0xFF}`. `0xFF` is the literal `0xA3F0E9` stamps on its NULL branch:
//! the hardcoded **family-2 two-hop** `0x57F6F0` found nothing, because `d1-responder` answers
//! family 2's subscription with `object_count = 0`. Those zeros were never written - nothing read
//! our family-3 and family-4 objects and produced 0.
//!
//! This module builds the two objects that two-hop resolves, through the **client's own encoder**
//! ([`crate::ObjectOracle`]) so a hand-packing mistake is a refusal instead of a payload.
//!
//! ```text
//! d1-oracle roster layout                       # the derived offset table + instance checksums
//! d1-oracle roster controls                     # 16 checks, exit 2 on failure
//! d1-oracle roster build --out capture-rig/rosters
//! ```
//!
//! # ⭐ THE CHAIN THIS FEEDS, AND WHY THE KEYS ARE THE WHOLE PROBLEM
//!
//! ```text
//! 0xA2A300  cui::c_orbit_roster_component::vfunc[14], per update
//!   0x57E860(iter, out)   copies contact i OUT OF THE STORE into a 0x5C-byte struct:
//!        0x57E8BE  imul r15, rax, 0x68            <- contact stride 0x68, count at store+0x20
//!        0x57E940  mov rax, [r14+r15+0x28]        <- ⭐ THE CONTACT KEY, store+0x28 + i*0x68
//!        0x57E945  mov [rbx], rax                 <-    into contact_out+0x00
//!   0xA3F000(container, contact_out)
//!     0xA3F09D  test r15,r15 ; je   <- 0x581900() roster store; live 0x20500f0d0, a NON-NULL GATE
//!                                     ONLY: 0x57F6F0 never reads its rdi (0x57F6FA mov r15,rsi)
//!     0xA3F0A9  mov rsi,rbx ; 0xA3F0AC call 0x57F6F0(store, THE CONTACT COPY)   THE TWO-HOP
//!         0x57F708  cmp qword [r15],0 ; je      <- early-out on a zero contact key
//!         0x57F720  mov rcx,r15 ; call 0x56CA70(mgr, 2, 0, &contact[0])       hop 1
//!         0x57F738  cmp qword [rbx+8],0 ; je    <- ⭐ 2/0's +0x08 must be NON-ZERO
//!         0x57F73F  add rbx,8 ; 0x57F755 mov rcx,rbx ; call 0x56CA70(mgr, 2, 1, &hop1[8])   hop 2
//!     0xA3F0C1  mov rcx,[rax] ; 0xA3F0C4 mov [r14+8],rcx    the ONLY producer of container+0x08
//!     0xA3F0B6  mov byte [r14+0x10], 2   bound   |  0xA3F0E9 mov byte [r14+0x10], 0xff   NULL
//! ```
//!
//! ⚠ The family and type arguments are **never immediates** - they are calls to two ICF-folded leaf
//! stubs, `0x1DF0` (`xor eax,eax; ret` ⇒ type 0) and `0x1048A40` (`mov eax,1; ret` ⇒ type 1), which
//! the decompiler mislabels as unrelated vfuncs. Reading the decompilation without resolving them
//! hides every family/type literal in this chain.
//!
//! So there are exactly **two key constraints**, and both are equalities the server has to honour:
//!
//! 1. [`RosterSpec::contact_key`] **must equal the roster store's contact key** - `store+0x28 +
//!    i*0x68`, read live as the account SOID `0xD100000000000001`. Hop 1 is a linear scan comparing
//!    the element's leading `u64` (`0x56CAC7 mov rsi,[rax] ; 0x56CACD cmp rsi,[rcx]`) - no mask, no
//!    SOID-kind test - so a mismatch is a miss with no diagnostic of any kind.
//! 2. [`RosterSpec::card_key`] is written into **both** 2/0's `+0x08` and 2/1's `+0x00`. Hop 2 keys
//!    on the *address of* 2/0's `+0x08` (`0x57F73F add rbx,8`), so the two must be byte-identical.
//!    [`RosterSpec`] carries one field for both and [`verify`] asserts the equality on the decodes,
//!    because two fields that must agree, written in two places, is how a silent miss gets built.
//!
//! ⚠ `card_key`'s **value** is unconstrained beyond "non-zero". A census of all 110 confirmed
//! `call 0x56CA70` sites - with the five known sites as its positive control - finds **no code that
//! reads 2/0's `+0x08` as anything but a key or a presence predicate** (`0xA3DBD0 cmp qword
//! [r14+8],0 ; 0xA3DBD7 or byte [r15+0x1a8],8`). Its real-world *form* is `[U]` and only a capture
//! can close it; ours is `synthetic-minimal`.
//!
//! ⚠ **Two other families share that key space**, so pick a value that cannot collide:
//! `0xA3DB55` looks up **family 0 / type 1** with the same key (see below), and `0x9E9AFD` in
//! `cui::c_fireteam_details_component::vfunc[14]` looks up **family 2 / type 1** keyed by
//! *family 6 / type 0*'s `+0x08`.
//!
//! # What the nameplate actually reads, and the instruction that reads it
//!
//! The compact fill `0xA2DE30` is reached from `0xA3D4E0` once the two-hop resolves. Every field
//! below is pinned by an operand, never by arithmetic:
//!
//! | card offset | instruction | lands on |
//! |---|---|---|
//! | `+0x10` `u32` | `0xA2DE67 mov r13d,[r15+0x10]` | `0xA2DE93 mov [rbx+0x188]` - ⭐ **LEVEL** |
//! | `+0x14` `u32` | `0xA2DE56 mov r12d,[r15+0x14]` | `0x281FA0` → `0xA2DE76 mov [rbx+0x180]` - ⭐ **LIGHT** |
//! | `+0x18` `f32` | `0xA3DD72 vmovss xmm0,[r12+0x18]` | `0xA2DEC6` → `[rbx+0x190]`, frac → `+0x194` |
//! | `+0x20` `u16` | `0xA2DF72 movzx esi,word [r15+0x20]` | `0xA2DF7A call 0xA2DBC0` - the **EMBLEM** |
//!
//! and `0xA2DEB2 call 0x276220` (the level cap, replayed offline as **40**) `/ 0xA2DEB7 cmp
//! eax,[rbx+0x188] / sete cl` → `[rbx+0x19d]` is the *at-cap* marker, so level 40 lights it too.
//!
//! # ⛔ THE EMBLEM STAYS BLACK, AND SAYING SO IS PART OF SHIPPING THIS
//!
//! [`EMPTY_CONTENT_INDEX`] is `0xFFFF` and that is what [`RosterSpec::default`] sends. **No item
//! content index has ever been measured on this build** (`THE-EMPTY-PROFILE.md` §8.5), and a
//! fabricated one is worse than an empty slot because it renders as *something* and looks like
//! success. Expect level and light to come alive with the emblem still black.
//!
//! # ⛔⛔ DO NOT ALSO SEND FAMILY 0. IT IS ALL-OR-NOTHING, AND A HALF-FILLED ONE IS A REGRESSION
//!
//! `0xA3D4E0` has **four** gates, in this order, and family 0 is the last:
//!
//! | # | instruction | tests | on failure |
//! |---|---|---|---|
//! | A | `0xA3DAE4 test byte [rax+0x50],2 ; 0xA3DAE8 je` | contact flags | full blank, `0xA3DD55` |
//! | B | `0xA3DD05 cmp qword [rbp-0x1b8],0 ; 0xA3DD14 je` | the `0x548440()` content handle | full blank |
//! | C | `0xA3DD16 test r12,r12 ; 0xA3DD19 je 0xA3DD5F` | ⭐ **family 2 / type 1** | `0xA3DD68` black emblem, **neither fill runs** - *this is the gate firing today* |
//! | D | `0xA3DD1B test rsi,rsi ; 0xA3DD1E je 0xA3DD72` | family 0 / type 1 | **falls through to the compact fill** - not a blank |
//!
//! Gate B is not a server object at all (`0x548440` is `return DAT_026A0910`, the investment-content
//! handle), so QueueZ can neither satisfy nor break it. ⇒ **family 2 alone binds the contact and
//! fills the plate.**
//!
//! ⛔ **The inverse is the trap.** When family 0/type 1 *does* resolve, `0xA3D4E0` runs the **rich**
//! fill `0xA2D9F0` instead of the compact one - and `0xA2D9F0` writes *the same eight nameplate
//! fields* from `0x808017FD`'s own body rather than from the card. It is a **substitution, not an
//! augmentation**. So an empty or half-built family-0 object **blanks a plate that family 2 alone
//! would have filled**. This module therefore builds family 2 only, and [`PEER_TAG`] exists to be
//! documented, not sent. (One field is genuinely rich-only: nameplate `+0x184`, which the compact
//! fill hardcodes to zero at `0xA2DF12 vxorps xmm0,xmm0,xmm0`. That is the entire cost.)
//!
//! # ⚠ Sentinels differ by width, and getting one wrong silently erases data
//!
//! * `0xFFFF` - the empty marker for a **content index** ([`EMPTY_CONTENT_INDEX`]).
//! * `0xFF` - the empty marker for a **stat key**.
//! * `0x0000` - a **valid index everywhere in this client**, never "absent".
//!
//! # ⭐ THE CLAN TAG MUST BE NUL-TERMINATED, AND THAT IS AN OVER-READ GUARD
//!
//! The widget copies both strings with `0x65A00`, which is `wcsncpy(dst, src, n)` where **`n` is the
//! DESTINATION capacity** and the loop **stops at the source's NUL**:
//!
//! ```text
//! 0x65A00  dec edx                       ; n-1
//! 0x65A10  mov cx,[rsi+rax*2]            ; src[i]
//! 0x65A14  mov [rdi+rax*2],cx
//! 0x65A18  test cx,cx ; je 0x65A24       ; ⭐ stops at the SOURCE's NUL
//! 0x65A1D  inc rax ; cmp eax,edx ; jl 0x65A10
//! 0x65A27  mov word [rdi+rax*2],0        ; always terminates the destination
//! ```
//!
//! `0xA3D4E0` calls it as `0x65A00(widget+0x118, contact+0x10, 0x1b)` and
//! `0x65A00(widget+0x158, contact+0x46, 0x11)`. The name's capacity `0x1B` = 27 matches its 27-unit
//! array, so a full name truncates but stays in bounds. **The clan's capacity is `0x11` = 17 against
//! an array of only 5 units** - so a clan tag with no NUL inside those 5 units makes the client read
//! up to **11 units past the end of the 80-byte object**, into the next registry slot. Four
//! characters plus the terminator is not a style rule; it is the bound that keeps the client inside
//! our object. [`RosterSpec::contact_object`] refuses anything longer.
//!
//! ⇒ And `+0x10` is the **first character**, not a length prefix: both readers pass `obj + 0x10`
//! straight in as `src` (`0xA3D4E0`'s `0x65A00(widget+0x118, lVar9+0x10, 0x1b)` and `0xA2D790`'s
//! `0x65A00(param_1+0x110, lVar7+0x10, 0x1b)`). A `{u16 len, u16 chars[26]}` reading of the same
//! array is **refuted** by those two operands.
//!
//! # ⚠ The 4-byte packed colour at `+0x24` is a BITFIELD and the codec proves it
//!
//! `0x808018E5` is `{u8, u8, 3 bits, 5 bits}`. Writing `0x07` into the 3-bit field at `+0x26` makes
//! the round trip come back with `+0x25 = 0x01` - the value spilled into its neighbour. That is
//! [`controls`]' negative control: it must keep failing, at that exact offset. The whole struct is
//! therefore left **zero**, and no colour is claimed.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use d1_eboot::descriptors::{self, Descriptor, Field, Index, LEAF};
use d1_eboot::image::Image;

use crate::error::{OracleError, Result};
use crate::object::Decoded;
use crate::ObjectOracle;

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Tags, sizes and the wire `instance_type`
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// Family 2 / type 0 - the roster **contact**: the two keys, the display name, the clan tag.
pub const CONTACT_TAG: u32 = 0x8080_18B7;
/// Family 2 / type 1 - the roster **player card**: level, Light, XP, emblem.
pub const CARD_TAG: u32 = 0x8080_1810;
/// Family 0 / type 1 - the rich **peer view**. Not built here; see [`PEER_TAG`]'s docs.
///
/// `0x808017FD` is **2,632 bytes = `0xA48`** - the two figures two different traces published are
/// the same number in different bases, and [`controls`] asserts it. `0xA3DB55` looks it up with
/// family **0**, type **1** and *the same key as 2/1*, and `0xA3D4E0` prefers its rich fill
/// `0xA2D9F0` when it resolves; when it does not, control falls through to the compact fill
/// `0xA2DE30`. So family 0 is an **upgrade path, not a gate** - `0xA3DD16 test r12,r12` (family 2)
/// is the branch that blanks, and `0xA3DD1B test rsi,rsi` (family 0) only chooses which fill runs.
pub const PEER_TAG: u32 = 0x8080_17FD;
/// Family 0 / type 0 - the peer **contact**, and it is `0x808018B7`'s twin: an identical 4-field
/// tree (`{u64 key, u64 secondary, 27×u16 name @+0x10, 5×u16 clan @+0x46}`, 80 B).
///
/// ⭐ It was believed unreadable statically (`design/eboot-map.md` §3.0b). It is not: `0x56C550`'s
/// leaf `0x56C717 lea rax,[0x19662E0]` names a `.data` slot whose relocated BSS object carries the
/// **instance checksum** `0x1D7BB8F6`, and exactly one descriptor in the image has it.
/// Family 0 has its own two-hop `0x58D350`/`0x5468A0`, structurally identical to `0x57F6F0`.
/// Recorded, not built - see [`PEER_TAG`]'s all-or-nothing warning.
pub const PEER_CONTACT_TAG: u32 = 0x8080_1921;

pub const CONTACT_SIZE: usize = 80;
pub const CARD_SIZE: usize = 48;
pub const PEER_SIZE: usize = 2632;
pub const PEER_CONTACT_SIZE: usize = 80;

/// ⭐ Registry capacity, from `0x56C550`'s own immediates - `0x56C735 mov dword [rcx+8],0xE7` for
/// both family-2 types, `0x56C708`/`0x56C57E mov dword [rcx+8],0x14` for both family-0 types.
/// A roster body may not carry more objects than these.
pub const CONTACT_SLOTS: u32 = 231;
pub const CARD_SLOTS: u32 = 231;
pub const PEER_SLOTS: u32 = 20;

/// The wire `instance_type` a QueueZ object header carries, for a schema tag.
///
/// `0x56B6BC` resolves the schema handle to its registry object and compares `object+0x08` against
/// the wire `instance_type`. The registry object starts [`RECORD_PREFIX`] bytes before the
/// descriptor anchor, so the checksum is `u32[anchor - 0x50 + 8]` - the same expression
/// `d1-eboot controls` pins for family 4/4.
pub const RECORD_PREFIX: u64 = 0x50;

/// ⭐ Family 2/0's checksum. Reproduces `d1-responder`'s `PROGRESSION_ROOT_INSTANCE_TYPE`.
pub const CONTACT_INSTANCE_TYPE: u32 = 0x3E3A_CE74;
/// ⭐ Family 2/1's checksum. Reproduces `d1-responder`'s `PROGRESSION_INSTANCE_TYPE`.
pub const CARD_INSTANCE_TYPE: u32 = 0x0444_D2D9;
/// ⭐ Family 0/1's checksum - **derived here, never before written down.** Its two siblings above
/// are the positive controls: the same expression reproduces two constants the shipping server
/// already uses, so this third one is not a lone unchecked read. Independently reproduced by the
/// `0x56C550` → `.data` slot `0x1966268` → BSS `0x28B2280` route.
pub const PEER_INSTANCE_TYPE: u32 = 0x67D9_7FC4;
/// Family 0/0's checksum, by the same expression and the same two positive controls.
pub const PEER_CONTACT_INSTANCE_TYPE: u32 = 0x1D7B_B8F6;

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// C offsets. ABSOLUTE in the decoded struct. Every one is re-derived by `derive_layout`.
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// `0x808018B7` field `[0]` - the contact key. Hop 1 scans this.
pub const OFF_CONTACT_KEY: usize = 0x00;
/// ⭐ `0x808018B7` field `[1]` - **the key into family 2/type 1**, and the only producer of
/// `container+0x08` in the whole binary (`0xA3F0C1 mov rcx,[rax]`).
pub const OFF_CONTACT_CARD_KEY: usize = 0x08;
/// `0x80805AF2`, 27 × `u16` UTF-16.
pub const OFF_CONTACT_NAME: usize = 0x10;
pub const CONTACT_NAME_UNITS: usize = 27;
/// `0x80804712`, 5 × `u16` UTF-16 - Destiny clan tags are at most 4 characters.
pub const OFF_CONTACT_CLAN: usize = 0x46;
pub const CONTACT_CLAN_UNITS: usize = 5;

/// `0x80801810` field `[0]` - must equal [`OFF_CONTACT_CARD_KEY`]'s value.
pub const OFF_CARD_KEY: usize = 0x00;
/// `[1]` `u16`. `[U]`; the sibling at `+0x0A` is a content index, so `0xFFFF` is left here too.
pub const OFF_CARD_U16_A: usize = 0x08;
/// `[2]` `u16` - ⭐ **a content index with the `0xFFFF` sentinel**, not an opaque `[U]`:
/// `0x9E9B11 cmp word [rbx+0xa],0xffff ; je` then `0x9E9B3A movzx esi,word [rbx+0xa] ;
/// 0x9E9B41 call 0x548860(investment_globals, idx)`. Same shape as [`OFF_CARD_EMBLEM`], same rule:
/// ship `0xFFFF` until an index has been *measured*.
pub const OFF_CARD_U16_B: usize = 0x0A;
/// `[3]` `u8` - `0xA3DD9A mov al,[r12+0xc]` → `widget+0x1C0`. `[U]`.
pub const OFF_CARD_U8: usize = 0x0C;
/// ⭐ **LEVEL** - `0xA2DE67 mov r13d,[r15+0x10]` → `0xA2DE93 mov [rbx+0x188]`.
pub const OFF_CARD_LEVEL: usize = 0x10;
/// ⭐ **LIGHT, raw** - `0xA2DE56 mov r12d,[r15+0x14]` → `0x281FA0` → `0xA2DE76 mov [rbx+0x180]`.
pub const OFF_CARD_LIGHT: usize = 0x14;
/// XP fraction - `0xA3DD72 vmovss xmm0,[r12+0x18]`; `0xA2DEC6` → `[rbx+0x190]`.
pub const OFF_CARD_XP: usize = 0x18;
/// `0xA3DD82 mov eax,[r12+0x1c]` → `[rbx+0x1B0]`; `0xA2DEF1` → `[rbx+0x18C]`. `[U]`.
pub const OFF_CARD_U32: usize = 0x1C;
/// ⭐ **EMBLEM content index** - `0xA2DF72 movzx esi,word [r15+0x20]` → `0xA2DF7A call 0xA2DBC0`.
pub const OFF_CARD_EMBLEM: usize = 0x20;
/// `0x808018DB` field `[0]` - the 4-byte `0x808018E5` **packed bitfield** `{u8, u8, 3 bits,
/// 5 bits}`. Left zero; see the module docs. Spans `+0x24..+0x28` only.
pub const OFF_CARD_COLOUR: usize = 0x24;
/// The 3-bit field inside the packed colour. Writing `0x07` here **must** break the round trip -
/// that is the negative control, and it fails at [`CARD_COLOUR_SPILL`].
pub const OFF_CARD_COLOUR_3BIT: usize = 0x26;
/// Where a 3-bit overflow lands when the codec re-packs it.
pub const CARD_COLOUR_SPILL: usize = 0x25;
/// ⭐ `0x808018DB` field `[1]` - **the peer-group comparator, not part of the colour.**
///
/// `0xA3F0C8 mov eax,[rax+0x28] ; 0xA3F0CB mov [r14+0x58],eax` stores this card's value on the
/// container; `0xA3F111` then re-runs the two-hop for the **local** player and
/// `0xA3F123 cmp edx,[rax+0x28] ; 0xA3F128 or cl,2` sets bit 1 of `container+0x6D` when they match.
/// So every card carrying the *same* value reads as "in my group". Zero is such a value, which is
/// why leaving it zero is a coherent solo answer rather than an omission. `[C]`
pub const OFF_CARD_PEER_GROUP: usize = 0x28;
/// `0x808018DB` field `[2]` - `u8`, `[U]`.
pub const OFF_CARD_TAIL_U8: usize = 0x2C;

/// ⭐ `0xFFFF` is "empty" for a **content index**. `0x0000` is a valid index and never "absent".
pub const EMPTY_CONTENT_INDEX: u16 = 0xFFFF;

/// The level cap, replayed offline from `0x276220` (`THE-EMPTY-PROFILE.md` §4). Sending it lights
/// the at-cap marker at `nameplate+0x19D` as well as the number.
pub const LEVEL_CAP: u32 = 40;

/// The account SOID this project keys everything by, and the value read live at `roster_store+0x28`.
pub const DEFAULT_CONTACT_KEY: u64 = crate::ACCOUNT_SOID;
/// The 2/0 → 2/1 link key. Any stable non-zero value works; this one keeps the `0xD1…05xx` block
/// clear of the account (`…0001`), character (`…0101`), profile (`…0201`) and item (`…03xx`) SOIDs
/// `d1-responder` already mints, so a stray equality cannot be mistaken for a resolution.
pub const DEFAULT_CARD_KEY: u64 = 0xD100_0000_0000_0501;

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// The spec
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// Everything the two family-2 objects need, and nothing that is a guess.
#[derive(Debug, Clone, PartialEq)]
pub struct RosterSpec {
    /// ⭐ Must equal the roster store's contact key (`roster_store+0x28`, live `0xD100000000000001`).
    pub contact_key: u64,
    /// ⭐ Written into 2/0 `+0x08` **and** 2/1 `+0x00`. One field, because they must be equal.
    pub card_key: u64,
    /// Up to [`CONTACT_NAME_UNITS`] UTF-16 units. Longer is a refusal, not a truncation.
    pub display_name: String,
    /// Up to [`CONTACT_CLAN_UNITS`] UTF-16 units.
    pub clan_tag: String,
    /// `+0x10`. The server states it; nothing derives it.
    pub level: u32,
    /// `+0x14`, **raw**. `0x281FA0` scales it through the stat definition's interpolation curve
    /// before it reaches the plate, so the displayed number may not be this one. `[U]`.
    pub light: u32,
    /// `+0x18`, the XP bar fraction.
    pub xp_fraction: f32,
    /// `+0x20`. [`EMPTY_CONTENT_INDEX`] unless a real content index has been *measured*.
    pub emblem: u16,
    /// `+0x08` / `+0x0A`. `+0x0A` is a content index whose sentinel `0x9E9B11` tests as `0xFFFF`,
    /// so `0xFFFF` is what an unmeasured pair carries - **never zero**, which claims index 0.
    pub unknown_u16_pair: (u16, u16),
    /// `+0x28` - the peer-group comparator. Cards sharing a value read as "in my group"
    /// (`0xA3F123 cmp edx,[rax+0x28]`). Zero is a coherent answer for a solo roster.
    pub peer_group: u32,
}

impl Default for RosterSpec {
    fn default() -> Self {
        RosterSpec {
            contact_key: DEFAULT_CONTACT_KEY,
            card_key: DEFAULT_CARD_KEY,
            display_name: "D1Guardian".into(),
            clan_tag: String::new(),
            level: LEVEL_CAP,
            light: crate::profile::LIGHT_ROI_CAP,
            xp_fraction: 0.0,
            emblem: EMPTY_CONTENT_INDEX,
            unknown_u16_pair: (EMPTY_CONTENT_INDEX, EMPTY_CONTENT_INDEX),
            peer_group: 0,
        }
    }
}

fn utf16_into(dst: &mut [u8], s: &str, units: usize, what: &str) -> Result<()> {
    let u: Vec<u16> = s.encode_utf16().collect();
    // One unit is reserved for the terminator: `0xA3DBF3`'s copy takes a fixed count and the widget
    // treats the buffer as a C string, so a name that exactly fills the array has no NUL.
    if u.len() >= units {
        return Err(OracleError::Shape(format!(
            "{what} {s:?} is {} UTF-16 units; the schema holds {units} including the terminator. \
             Truncating here would put a name on the wire that is not the name asked for",
            u.len()
        )));
    }
    for (i, c) in u.iter().enumerate() {
        dst[i * 2..i * 2 + 2].copy_from_slice(&c.to_le_bytes());
    }
    Ok(())
}

impl RosterSpec {
    /// The 80-byte family-2/type-0 contact, as the decoded C struct.
    pub fn contact_object(&self) -> Result<Vec<u8>> {
        if self.contact_key == 0 {
            return Err(OracleError::Shape(
                "contact_key is 0 - 0x57F6F0 early-returns on `*contact == 0`, before it touches \
                 family 2 at all, so this object could never be found"
                    .into(),
            ));
        }
        if self.card_key == 0 {
            return Err(OracleError::Shape(
                "card_key is 0 - hop 2 would scan family 2/type 1 for key 0 and the container's \
                 +0x08 would stay 0, which is exactly the blank the fill tests for"
                    .into(),
            ));
        }
        let mut o = vec![0u8; CONTACT_SIZE];
        o[OFF_CONTACT_KEY..OFF_CONTACT_KEY + 8].copy_from_slice(&self.contact_key.to_le_bytes());
        o[OFF_CONTACT_CARD_KEY..OFF_CONTACT_CARD_KEY + 8]
            .copy_from_slice(&self.card_key.to_le_bytes());
        utf16_into(
            &mut o[OFF_CONTACT_NAME..OFF_CONTACT_NAME + CONTACT_NAME_UNITS * 2],
            &self.display_name,
            CONTACT_NAME_UNITS,
            "display name",
        )?;
        utf16_into(
            &mut o[OFF_CONTACT_CLAN..OFF_CONTACT_CLAN + CONTACT_CLAN_UNITS * 2],
            &self.clan_tag,
            CONTACT_CLAN_UNITS,
            "clan tag",
        )?;
        Ok(o)
    }

    /// The 48-byte family-2/type-1 player card, as the decoded C struct.
    pub fn card_object(&self) -> Result<Vec<u8>> {
        if self.card_key == 0 {
            return Err(OracleError::Shape("card_key is 0 - see contact_object".into()));
        }
        let mut o = vec![0u8; CARD_SIZE];
        o[OFF_CARD_KEY..OFF_CARD_KEY + 8].copy_from_slice(&self.card_key.to_le_bytes());
        o[OFF_CARD_U16_A..OFF_CARD_U16_A + 2].copy_from_slice(&self.unknown_u16_pair.0.to_le_bytes());
        o[OFF_CARD_U16_B..OFF_CARD_U16_B + 2].copy_from_slice(&self.unknown_u16_pair.1.to_le_bytes());
        o[OFF_CARD_LEVEL..OFF_CARD_LEVEL + 4].copy_from_slice(&self.level.to_le_bytes());
        o[OFF_CARD_LIGHT..OFF_CARD_LIGHT + 4].copy_from_slice(&self.light.to_le_bytes());
        o[OFF_CARD_XP..OFF_CARD_XP + 4].copy_from_slice(&self.xp_fraction.to_le_bytes());
        o[OFF_CARD_EMBLEM..OFF_CARD_EMBLEM + 2].copy_from_slice(&self.emblem.to_le_bytes());
        o[OFF_CARD_PEER_GROUP..OFF_CARD_PEER_GROUP + 4]
            .copy_from_slice(&self.peer_group.to_le_bytes());
        // +0x24..+0x28 stays zero: the packed colour is a bitfield we cannot claim (module docs).
        Ok(o)
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Building
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// One built QueueZ object: everything `d1-responder` needs to frame it, plus the decode that
/// proves it.
#[derive(Debug, Clone)]
pub struct BuiltObject {
    pub tag: u32,
    /// The wire `instance_type` for this schema, derived from the eboot.
    pub instance_type: u32,
    /// The QueueZ object header's key - for these two, the leading `u64` of the payload itself.
    pub key: u64,
    pub family: u32,
    pub obj_type: u32,
    pub payload: Vec<u8>,
    pub bits: u32,
    pub decoded: Decoded,
}

impl BuiltObject {
    /// `f<family>t<type>-<key:016x>.bin` - the filename `d1-responder`'s loader parses. The key and
    /// the family live **in the name**, so a fixture cannot be served under a key that disagrees
    /// with its own contents; [`load_fixture_dir`] re-decodes and asserts the equality anyway.
    pub fn file_name(&self) -> String {
        format!("f{}t{}-{:016x}.bin", self.family, self.obj_type, self.key)
    }

    pub fn nonzero(&self) -> Vec<(usize, u64)> {
        self.decoded.nonzero_qwords()
    }
}

/// A built roster: the contact and the card, in the order the responder must send them.
#[derive(Debug, Clone)]
pub struct BuiltRoster {
    pub contact: BuiltObject,
    pub card: BuiltObject,
}

impl BuiltRoster {
    pub fn objects(&self) -> [&BuiltObject; 2] {
        [&self.contact, &self.card]
    }
}

/// Encode both objects through the **client's own encoder**, then decode them back and require the
/// C structs to be identical.
pub fn build(spec: &RosterSpec) -> Result<BuiltRoster> {
    let image = Image::open_default().map_err(eb)?;
    let idx = descriptors::index(&image).map_err(eb)?;

    let mut o = ObjectOracle::new(CONTACT_TAG)?;
    let (enc, dec) = o.round_trip(&spec.contact_object()?, 0x1000)?;
    let contact = BuiltObject {
        tag: CONTACT_TAG,
        instance_type: instance_type(&image, &idx, CONTACT_TAG)?,
        key: spec.contact_key,
        family: 2,
        obj_type: 0,
        payload: enc.payload,
        bits: enc.bits,
        decoded: dec,
    };

    let mut o = ObjectOracle::new(CARD_TAG)?;
    let (enc, dec) = o.round_trip(&spec.card_object()?, 0x1000)?;
    let card = BuiltObject {
        tag: CARD_TAG,
        instance_type: instance_type(&image, &idx, CARD_TAG)?,
        key: spec.card_key,
        family: 2,
        obj_type: 1,
        payload: enc.payload,
        bits: enc.bits,
        decoded: dec,
    };

    let built = BuiltRoster { contact, card };
    verify(spec, &built)?;
    Ok(built)
}

/// Assert the decodes carry every field the spec asked for, **at the offset it asked for**, and -
/// the check that exists because it is the failure mode - that the two keys still agree.
///
/// `round_trip` already proves the structs survived the codec. This exists so a spec whose fields
/// were silently dropped, or whose link key drifted between the two objects, cannot pass as a
/// populated roster.
pub fn verify(spec: &RosterSpec, built: &BuiltRoster) -> Result<()> {
    let bad = |m: String| Err(OracleError::Shape(m));
    let u32_at = |d: &Decoded, off: usize| -> u32 {
        u32::from_le_bytes(d.object[off..off + 4].try_into().unwrap_or([0; 4]))
    };
    let u16_at = |d: &Decoded, off: usize| -> u16 {
        u16::from_le_bytes(d.object[off..off + 2].try_into().unwrap_or([0; 2]))
    };

    let c = &built.contact.decoded;
    let k = &built.card.decoded;
    if c.object.len() != CONTACT_SIZE {
        return bad(format!("contact decoded {} bytes, expected {CONTACT_SIZE}", c.object.len()));
    }
    if k.object.len() != CARD_SIZE {
        return bad(format!("card decoded {} bytes, expected {CARD_SIZE}", k.object.len()));
    }
    if c.qword(OFF_CONTACT_KEY) != spec.contact_key {
        return bad(format!(
            "contact +{OFF_CONTACT_KEY:#04x} is {:#x}, expected the roster store's contact key \
             {:#x} - hop 1 is a linear scan on this qword and a miss is silent",
            c.qword(OFF_CONTACT_KEY),
            spec.contact_key
        ));
    }
    // ⭐ THE EQUALITY THE WHOLE CHAIN TURNS ON, asserted on the DECODES rather than on the spec:
    // 0x57F6F0 passes `&contact_object[8]` to 0x56CA70, so hop 2 compares these two qwords byte
    // for byte. Two fields that must agree, written in two places, is how a silent miss gets built.
    if c.qword(OFF_CONTACT_CARD_KEY) != k.qword(OFF_CARD_KEY) {
        return bad(format!(
            "the two-hop link is broken: contact +{OFF_CONTACT_CARD_KEY:#04x} = {:#x} but card \
             +{OFF_CARD_KEY:#04x} = {:#x}. Hop 2 keys on the contact's own +0x08, so these must be \
             identical or 0x57F6F0 returns NULL and 0xA3F0E9 stamps the family byte 0xFF",
            c.qword(OFF_CONTACT_CARD_KEY),
            k.qword(OFF_CARD_KEY)
        ));
    }
    if c.qword(OFF_CONTACT_CARD_KEY) != spec.card_key {
        return bad(format!(
            "contact +{OFF_CONTACT_CARD_KEY:#04x} is {:#x}, expected {:#x}",
            c.qword(OFF_CONTACT_CARD_KEY),
            spec.card_key
        ));
    }

    // The name survived as UTF-16, and is NUL-terminated inside the array.
    let name: String = {
        let u: Vec<u16> = (0..CONTACT_NAME_UNITS)
            .map(|i| u16_at(c, OFF_CONTACT_NAME + i * 2))
            .take_while(|&x| x != 0)
            .collect();
        String::from_utf16_lossy(&u)
    };
    if name != spec.display_name {
        return bad(format!(
            "contact +{OFF_CONTACT_NAME:#04x} decodes to {name:?}, expected {:?}",
            spec.display_name
        ));
    }
    let clan: String = {
        let u: Vec<u16> = (0..CONTACT_CLAN_UNITS)
            .map(|i| u16_at(c, OFF_CONTACT_CLAN + i * 2))
            .take_while(|&x| x != 0)
            .collect();
        String::from_utf16_lossy(&u)
    };
    if clan != spec.clan_tag {
        return bad(format!(
            "contact +{OFF_CONTACT_CLAN:#04x} decodes to {clan:?}, expected {:?}",
            spec.clan_tag
        ));
    }

    for (off, got, want, what) in [
        (OFF_CARD_LEVEL, u32_at(k, OFF_CARD_LEVEL), spec.level, "LEVEL"),
        (OFF_CARD_LIGHT, u32_at(k, OFF_CARD_LIGHT), spec.light, "LIGHT (raw)"),
    ] {
        if got != want {
            return bad(format!("card +{off:#04x} {what} is {got}, expected {want}"));
        }
    }
    if u16_at(k, OFF_CARD_EMBLEM) != spec.emblem {
        return bad(format!(
            "card +{OFF_CARD_EMBLEM:#04x} emblem is {:#06x}, expected {:#06x}",
            u16_at(k, OFF_CARD_EMBLEM),
            spec.emblem
        ));
    }
    // ⚠ The packed colour must have stayed zero - and ONLY the packed colour. `+0x28` is the
    // peer-group comparator and `+0x2C` a `[U]` u8; asserting the whole tail was zero would have
    // made a legitimate `peer_group` look like corruption. A non-zero byte in `+0x24..+0x28` is a
    // bitfield spill: some field wrote past its width, which the round trip surfaces as a byte
    // error but never as a *meaning* error.
    if k.object[OFF_CARD_COLOUR..OFF_CARD_PEER_GROUP].iter().any(|&b| b != 0) {
        return bad(format!(
            "card +{OFF_CARD_COLOUR:#04x}..+{OFF_CARD_PEER_GROUP:#04x} is not zero ({:02x?}) - \
             0x808018E5 is a bitfield and nothing here claims a colour, so a non-zero byte means a \
             neighbour overflowed",
            &k.object[OFF_CARD_COLOUR..OFF_CARD_PEER_GROUP]
        ));
    }
    if u32_at(k, OFF_CARD_PEER_GROUP) != spec.peer_group {
        return bad(format!(
            "card +{OFF_CARD_PEER_GROUP:#04x} peer-group is {}, expected {}",
            u32_at(k, OFF_CARD_PEER_GROUP),
            spec.peer_group
        ));
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Layout, re-derived from the eboot rather than transcribed
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// One array container's shape, read from its **descriptor header** and not from the offset column
/// `d1-eboot descriptors` prints for a `kind 0x1` field (that column is the element stride).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArrayShape {
    pub tag: u32,
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

/// Both family-2 layouts plus the three instance checksums, derived from this eboot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub contact_size: u32,
    pub contact_key: usize,
    pub contact_card_key: usize,
    pub name: ArrayShape,
    pub clan: ArrayShape,
    pub card_size: u32,
    pub card_key: usize,
    pub card_u16_a: usize,
    pub card_u16_b: usize,
    pub card_u8: usize,
    pub card_level: usize,
    pub card_light: usize,
    pub card_xp: usize,
    pub card_u32: usize,
    pub card_emblem: usize,
    pub card_colour: usize,
    pub peer_size: u32,
    pub contact_instance_type: u32,
    pub card_instance_type: u32,
    pub peer_instance_type: u32,
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

fn array_at(image: &Image, idx: &Index, tag: u32, base: usize) -> Result<ArrayShape> {
    let d = desc(idx, tag)?;
    let count = image.u32(d.anchor + 0x20).map_err(eb)?;
    let elem_size = image.u32(d.anchor + 0x28).map_err(eb)?;
    let shape = ArrayShape { tag, base, count, elem_size };
    shape.check(d)?;
    Ok(shape)
}

/// The wire `instance_type` for a schema tag: `u32[anchor - 0x50 + 8]`.
///
/// ⚠ **A lone read here would be an unchecked claim.** [`controls`] runs this against
/// [`CONTACT_INSTANCE_TYPE`] and [`CARD_INSTANCE_TYPE`], which are the two constants `d1-responder`
/// has been shipping - so the derivation is validated against known-good answers before its third
/// result ([`PEER_INSTANCE_TYPE`]) is believed.
pub fn instance_type(image: &Image, idx: &Index, tag: u32) -> Result<u32> {
    let d = desc(idx, tag)?;
    image.u32(d.anchor - RECORD_PREFIX + 8).map_err(eb)
}

/// Walk both family-2 schemas and produce the offsets this module writes to.
///
/// **This is the control on every constant above.** Nothing is transcribed from a document; if the
/// eboot disagrees, [`controls`] fails and exits 2.
pub fn derive_layout(image: &Image, idx: &Index) -> Result<Layout> {
    let contact = desc(idx, CONTACT_TAG)?.clone();
    let card = desc(idx, CARD_TAG)?.clone();
    let peer = desc(idx, PEER_TAG)?.clone();

    let contact_key = field(image, idx, CONTACT_TAG, 0)?.offset as usize;
    let contact_card_key = field(image, idx, CONTACT_TAG, 1)?.offset as usize;
    let (name_f, name_tag) = child(image, idx, CONTACT_TAG, 2)?;
    let name = array_at(image, idx, name_tag, name_f.offset as usize)?;
    let (clan_f, clan_tag) = child(image, idx, CONTACT_TAG, 3)?;
    let clan = array_at(image, idx, clan_tag, clan_f.offset as usize)?;

    let card_key = field(image, idx, CARD_TAG, 0)?.offset as usize;
    let card_u16_a = field(image, idx, CARD_TAG, 1)?.offset as usize;
    let card_u16_b = field(image, idx, CARD_TAG, 2)?.offset as usize;
    let card_u8 = field(image, idx, CARD_TAG, 3)?.offset as usize;

    // [4] +0x10 -> 0x808016F8 {u32 level, u32 light, 0x80801923 {f32 xp, u32, u16 emblem}}
    let (stats_f, stats_tag) = child(image, idx, CARD_TAG, 4)?;
    let stats_base = stats_f.offset as usize;
    let card_level = stats_base + field(image, idx, stats_tag, 0)?.offset as usize;
    let card_light = stats_base + field(image, idx, stats_tag, 1)?.offset as usize;
    let (xp_f, xp_tag) = child(image, idx, stats_tag, 2)?;
    let xp_base = stats_base + xp_f.offset as usize;
    let card_xp = xp_base + field(image, idx, xp_tag, 0)?.offset as usize;
    let card_u32 = xp_base + field(image, idx, xp_tag, 1)?.offset as usize;
    let emblem_f = field(image, idx, xp_tag, 2)?;
    // The emblem is a 16-bit leaf. The kind check is what makes an off-by-one in the field index a
    // refusal rather than a plausible offset - 0xFFFF written into a 32-bit field is not "empty".
    if emblem_f.bits() != Some(16) {
        return Err(OracleError::Shape(format!(
            "{xp_tag:#010x} field [2] is child {:#x} / {:?} bits, expected a 16-bit leaf - the \
             emblem's 0xFFFF sentinel is width-specific",
            emblem_f.child_tag,
            emblem_f.bits()
        )));
    }
    let card_emblem = xp_base + emblem_f.offset as usize;
    let card_colour = child(image, idx, CARD_TAG, 5)?.0.offset as usize;

    Ok(Layout {
        contact_size: contact.struct_size,
        contact_key,
        contact_card_key,
        name,
        clan,
        card_size: card.struct_size,
        card_key,
        card_u16_a,
        card_u16_b,
        card_u8,
        card_level,
        card_light,
        card_xp,
        card_u32,
        card_emblem,
        card_colour,
        peer_size: peer.struct_size,
        contact_instance_type: instance_type(image, idx, CONTACT_TAG)?,
        card_instance_type: instance_type(image, idx, CARD_TAG)?,
        peer_instance_type: instance_type(image, idx, PEER_TAG)?,
    })
}

/// The layout this module's constants declare - what [`derive_layout`] must reproduce.
pub fn declared_layout() -> Layout {
    Layout {
        contact_size: CONTACT_SIZE as u32,
        contact_key: OFF_CONTACT_KEY,
        contact_card_key: OFF_CONTACT_CARD_KEY,
        name: ArrayShape {
            tag: 0x8080_5AF2,
            base: OFF_CONTACT_NAME,
            count: CONTACT_NAME_UNITS as u32,
            elem_size: 2,
        },
        clan: ArrayShape {
            tag: 0x8080_4712,
            base: OFF_CONTACT_CLAN,
            count: CONTACT_CLAN_UNITS as u32,
            elem_size: 2,
        },
        card_size: CARD_SIZE as u32,
        card_key: OFF_CARD_KEY,
        card_u16_a: OFF_CARD_U16_A,
        card_u16_b: OFF_CARD_U16_B,
        card_u8: OFF_CARD_U8,
        card_level: OFF_CARD_LEVEL,
        card_light: OFF_CARD_LIGHT,
        card_xp: OFF_CARD_XP,
        card_u32: OFF_CARD_U32,
        card_emblem: OFF_CARD_EMBLEM,
        card_colour: OFF_CARD_COLOUR,
        peer_size: PEER_SIZE as u32,
        contact_instance_type: CONTACT_INSTANCE_TYPE,
        card_instance_type: CARD_INSTANCE_TYPE,
        peer_instance_type: PEER_INSTANCE_TYPE,
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Fixtures
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// Write both payloads into `dir` as `f<family>t<type>-<key>.bin`. Returns the paths written.
pub fn write_fixtures(dir: &Path, built: &BuiltRoster) -> Result<Vec<PathBuf>> {
    std::fs::create_dir_all(dir).map_err(|e| eb(format!("{}: {e}", dir.display())))?;
    let mut out = Vec::new();
    for o in built.objects() {
        let p = dir.join(o.file_name());
        std::fs::write(&p, &o.payload).map_err(|e| eb(format!("{}: {e}", p.display())))?;
        out.push(p);
    }
    Ok(out)
}

/// One fixture as `d1-responder` will read it: family, type, key and the raw payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fixture {
    pub family: u32,
    pub obj_type: u32,
    pub key: u64,
    pub payload: Vec<u8>,
}

/// Parse `f<family>t<type>-<key:016x>.bin`. `None` for any name that does not match - the caller
/// must treat that as a skip-with-a-warning, never as a default.
pub fn parse_fixture_name(name: &str) -> Option<(u32, u32, u64)> {
    let stem = name.strip_suffix(".bin")?;
    let rest = stem.strip_prefix('f')?;
    let (fam, rest) = rest.split_once('t')?;
    let (ty, key) = rest.split_once('-')?;
    Some((fam.parse().ok()?, ty.parse().ok()?, u64::from_str_radix(key, 16).ok()?))
}

/// Load and **re-verify** every fixture in `dir`, sorted by filename (type 0 before type 1).
///
/// ⚠ The re-decode is the point. A payload whose leading `u64` disagrees with the key in its own
/// filename would be installed in the registry under a name nothing looks up - the exact silent
/// miss this module exists to prevent - so that is an error here, not a warning.
pub fn load_fixture_dir(dir: &Path) -> Result<Vec<Fixture>> {
    let mut names: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| eb(format!("{}: {e}", dir.display())))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "bin"))
        .collect();
    names.sort();

    let mut out = Vec::new();
    for p in &names {
        let file = p.file_name().and_then(|s| s.to_str()).unwrap_or_default();
        let Some((family, obj_type, key)) = parse_fixture_name(file) else {
            return Err(OracleError::Shape(format!(
                "{}: name does not parse as f<family>t<type>-<key:016x>.bin",
                p.display()
            )));
        };
        let payload = std::fs::read(p).map_err(|e| eb(format!("{}: {e}", p.display())))?;
        let tag = match (family, obj_type) {
            (2, 0) => CONTACT_TAG,
            (2, 1) => CARD_TAG,
            (0, 1) => PEER_TAG,
            _ => {
                return Err(OracleError::Shape(format!(
                    "{}: family {family}/type {obj_type} has no schema in this module",
                    p.display()
                )))
            }
        };
        let d = ObjectOracle::new(tag)?.decode(&payload)?;
        if d.qword(0) != key {
            return Err(OracleError::Shape(format!(
                "{}: the payload's leading u64 is {:#x} but the filename says {key:#x}. The \
                 registry is keyed by the header, and 0x56CA70 scans the payload's own first \
                 qword - a disagreement installs the object under a name nothing looks up",
                p.display(),
                d.qword(0)
            )));
        }
        out.push(Fixture { family, obj_type, key, payload });
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
/// Each one can fail in the direction it is worried about - a control that only ever passes is
/// decoration:
///
/// | control | the axis it tests |
/// |---|---|
/// | `layout` | a constant here drifting from the eboot |
/// | `instance-checksums` | ⭐ the checksum derivation, against the **two constants `d1-responder` already ships** |
/// | `peer-size-0xa48` | the "20 × 0xA48" / "2632 B" contradiction - the same number in two bases |
/// | `contact-round-trip` / `card-round-trip` | the objects encode and decode identically |
/// | `two-hop-link` | contact `+0x08` == card `+0x00`, asserted on the DECODES |
/// | `two-hop-link-NEGATIVE` | ⭐ a spec whose keys disagree **must be refused** |
/// | `name-overflow-NEGATIVE` | ⭐ a 27-unit name **must be refused**, terminator included |
/// | `zero-key-NEGATIVE` | ⭐ `contact_key = 0` **must be refused** - `0x57F6F0` early-returns on it |
/// | `colour-bitfield-NEGATIVE` | ⭐ `+0x26 = 0x07` **must break the round trip** at `+0x25`. This is the control that proves the round trip discriminates at all |
/// | `emblem-zero-is-valid` | `0x0000` round-trips, so it is a **claim** and not an absence |
/// | `emblem-sentinel` | `0xFFFF` survives at `+0x20` as a `u16` |
/// | `level-and-light-land` | the two fields the HUD reads survive at the offsets two instructions name |
/// | `strictly-more-than-empty` | ⭐ monotonicity: the built card's non-zero set is a strict superset of a key-only card's |
/// | `fixture-name-round-trip` | the filename the responder parses reproduces `(family, type, key)` |
pub fn controls() -> Result<Vec<Control>> {
    let mut out = Vec::new();

    let image = Image::open_default().map_err(eb)?;
    let idx = descriptors::index(&image).map_err(eb)?;

    // 1. Layout, re-derived.
    let derived = derive_layout(&image, &idx)?;
    let declared = declared_layout();
    out.push(if derived == declared {
        ok(
            "layout",
            format!(
                "2/0 {} B: key +{:#x}, card-key +{:#x}, name {}×{} @+{:#x}, clan {}×{} @+{:#x}; \
                 2/1 {} B: key +{:#x}, level +{:#x}, light +{:#x}, xp +{:#x}, emblem +{:#x}, \
                 colour +{:#x}; 0/1 {} B",
                derived.contact_size,
                derived.contact_key,
                derived.contact_card_key,
                derived.name.count,
                derived.name.elem_size,
                derived.name.base,
                derived.clan.count,
                derived.clan.elem_size,
                derived.clan.base,
                derived.card_size,
                derived.card_key,
                derived.card_level,
                derived.card_light,
                derived.card_xp,
                derived.card_emblem,
                derived.card_colour,
                derived.peer_size,
            ),
        )
    } else {
        no("layout", format!("eboot says {derived:#?}\ncode says {declared:#?}"))
    });

    // 2. ⭐ The instance-checksum derivation, validated against two known-good answers.
    let (c0, c1, c2) = (
        instance_type(&image, &idx, CONTACT_TAG)?,
        instance_type(&image, &idx, CARD_TAG)?,
        instance_type(&image, &idx, PEER_TAG)?,
    );
    out.push(if c0 == CONTACT_INSTANCE_TYPE && c1 == CARD_INSTANCE_TYPE {
        ok(
            "instance-checksums",
            format!(
                "u32[anchor-{RECORD_PREFIX:#x}+8] reproduces d1-responder's two shipped constants \
                 (2/0 {c0:#010x}, 2/1 {c1:#010x}) - so its third answer, family 0/1 = {c2:#010x}, \
                 is a validated read and not a lone claim"
            ),
        )
    } else {
        no(
            "instance-checksums",
            format!(
                "derived 2/0 {c0:#010x} (want {CONTACT_INSTANCE_TYPE:#010x}), 2/1 {c1:#010x} \
                 (want {CARD_INSTANCE_TYPE:#010x}) - the derivation is wrong, so {c2:#010x} for \
                 family 0/1 may not be cited"
            ),
        )
    });

    // 2b. ⭐ Family 0/0 is family 2/0's twin, so a future family-0 build reuses this code path -
    //     and so "family 0/0 cannot be read statically" is closed. Compared FIELD BY FIELD, because
    //     equal struct sizes is the kind of match that closes on the wrong answer.
    let ck0 = instance_type(&image, &idx, PEER_CONTACT_TAG)?;
    let same_tree = (0..4).all(|i| {
        match (
            field(&image, &idx, CONTACT_TAG, i),
            field(&image, &idx, PEER_CONTACT_TAG, i),
        ) {
            (Ok(a), Ok(b)) => a.offset == b.offset && a.kind == b.kind && a.child_tag == b.child_tag,
            _ => false,
        }
    });
    let sz0 = desc(&idx, PEER_CONTACT_TAG)?.struct_size as usize;
    out.push(if same_tree && sz0 == PEER_CONTACT_SIZE && ck0 == PEER_CONTACT_INSTANCE_TYPE {
        ok(
            "family-0-type-0-is-the-twin",
            format!(
                "{PEER_CONTACT_TAG:#010x} is {sz0} B, instance {ck0:#010x}, and all four fields \
                 match {CONTACT_TAG:#010x} in offset/kind/child - so family 0/0 IS readable \
                 statically, closing eboot-map.md §3.0b's \"it cannot be read statically\""
            ),
        )
    } else {
        no(
            "family-0-type-0-is-the-twin",
            format!("{PEER_CONTACT_TAG:#010x}: {sz0} B, instance {ck0:#010x}, same tree {same_tree}"),
        )
    });

    // 2c. Registry capacity. A body carrying more objects than a family has slots is a silent
    //     truncation in the client, so the numbers belong in code with their source named.
    out.push(if (CONTACT_SLOTS, CARD_SLOTS, PEER_SLOTS) == (231, 231, 20) {
        ok(
            "registry-capacity",
            "0x56C735 mov dword [rcx+8],0xE7 -> family 2/0 and 2/1 hold 231 each; \
             0x56C708 / 0x56C57E mov dword [rcx+8],0x14 -> family 0/0 and 0/1 hold 20 each"
                .to_string(),
        )
    } else {
        no("registry-capacity", format!("{CONTACT_SLOTS}/{CARD_SLOTS}/{PEER_SLOTS}"))
    });

    // 3. The size contradiction, settled by arithmetic on one number.
    out.push(if derived.peer_size as usize == PEER_SIZE && PEER_SIZE == 0xA48 {
        ok(
            "peer-size-0xa48",
            format!("family 0/1 is {PEER_SIZE} B = {PEER_SIZE:#x} - \"2632 B\" and \"0xA48\" are the same number"),
        )
    } else {
        no("peer-size-0xa48", format!("eboot says {} B", derived.peer_size))
    });

    // 4/5/6/7. The shipped spec: both objects, the link, and the fields the HUD reads.
    let spec = RosterSpec::default();
    match build(&spec) {
        Ok(b) => {
            out.push(ok(
                "contact-round-trip",
                format!(
                    "{} B / {} bits, instance {:#010x}, key {:#018x}, name {:?}",
                    b.contact.payload.len(),
                    b.contact.bits,
                    b.contact.instance_type,
                    b.contact.key,
                    spec.display_name
                ),
            ));
            out.push(ok(
                "card-round-trip",
                format!(
                    "{} B / {} bits, instance {:#010x}, key {:#018x}",
                    b.card.payload.len(),
                    b.card.bits,
                    b.card.instance_type,
                    b.card.key
                ),
            ));
            out.push(ok(
                "two-hop-link",
                format!(
                    "contact +{OFF_CONTACT_CARD_KEY:#04x} == card +{OFF_CARD_KEY:#04x} == {:#018x}",
                    b.card.decoded.qword(OFF_CARD_KEY)
                ),
            ));
            let lvl = u32::from_le_bytes(
                b.card.decoded.object[OFF_CARD_LEVEL..OFF_CARD_LEVEL + 4].try_into().unwrap(),
            );
            let lt = u32::from_le_bytes(
                b.card.decoded.object[OFF_CARD_LIGHT..OFF_CARD_LIGHT + 4].try_into().unwrap(),
            );
            out.push(if lvl == spec.level && lt == spec.light {
                ok(
                    "level-and-light-land",
                    format!(
                        "+{OFF_CARD_LEVEL:#04x} = {lvl} (0xA2DE67 -> nameplate+0x188), \
                         +{OFF_CARD_LIGHT:#04x} = {lt} (0xA2DE56 -> 0x281FA0 -> nameplate+0x180)"
                    ),
                )
            } else {
                no("level-and-light-land", format!("decoded level {lvl}, light {lt}"))
            });

            // ⭐ Monotonicity: a populated card must strictly dominate a key-only one. Catches a
            // rewrite that quietly drops level or light while still round-tripping.
            let mut o = ObjectOracle::new(CARD_TAG)?;
            let mut bare = vec![0u8; CARD_SIZE];
            bare[..8].copy_from_slice(&spec.card_key.to_le_bytes());
            match o.round_trip(&bare, 0x1000) {
                Ok((_, d)) => {
                    let want: BTreeSet<usize> = d.nonzero_offsets().into_iter().collect();
                    let got: BTreeSet<usize> =
                        b.card.decoded.nonzero_offsets().into_iter().collect();
                    out.push(if want.is_subset(&got) && got.len() > want.len() {
                        ok(
                            "strictly-more-than-empty",
                            format!(
                                "{} non-zero qwords vs a key-only card's {} - and the key survives",
                                got.len(),
                                want.len()
                            ),
                        )
                    } else {
                        no(
                            "strictly-more-than-empty",
                            format!(
                                "populated {got:?} is not a strict superset of key-only {want:?} - \
                                 a card that drops level or light is a regression, not a fix"
                            ),
                        )
                    });
                }
                Err(e) => out.push(no("strictly-more-than-empty", e.to_string())),
            }

            // The filename the responder parses.
            let n = b.card.file_name();
            out.push(match parse_fixture_name(&n) {
                Some((2, 1, k)) if k == spec.card_key => {
                    ok("fixture-name-round-trip", format!("{n} -> family 2, type 1, key {k:#018x}"))
                }
                other => no("fixture-name-round-trip", format!("{n} parsed as {other:?}")),
            });
        }
        Err(e) => {
            out.push(no("contact-round-trip", e.to_string()));
            out.push(no("card-round-trip", e.to_string()));
            out.push(no("two-hop-link", e.to_string()));
            out.push(no("level-and-light-land", e.to_string()));
            out.push(no("strictly-more-than-empty", e.to_string()));
            out.push(no("fixture-name-round-trip", e.to_string()));
        }
    }

    // 8. ⭐ NEGATIVE - the link equality must be enforced, not merely documented.
    let mut o = ObjectOracle::new(CONTACT_TAG)?;
    let bad_spec = RosterSpec { card_key: DEFAULT_CARD_KEY, ..RosterSpec::default() };
    let (_, cdec) = o.round_trip(&bad_spec.contact_object()?, 0x1000)?;
    let mut o = ObjectOracle::new(CARD_TAG)?;
    let drifted = RosterSpec { card_key: DEFAULT_CARD_KEY ^ 1, ..RosterSpec::default() };
    let (_, kdec) = o.round_trip(&drifted.card_object()?, 0x1000)?;
    let mixed = BuiltRoster {
        contact: BuiltObject {
            tag: CONTACT_TAG,
            instance_type: CONTACT_INSTANCE_TYPE,
            key: bad_spec.contact_key,
            family: 2,
            obj_type: 0,
            payload: Vec::new(),
            bits: 0,
            decoded: cdec,
        },
        card: BuiltObject {
            tag: CARD_TAG,
            instance_type: CARD_INSTANCE_TYPE,
            key: drifted.card_key,
            family: 2,
            obj_type: 1,
            payload: Vec::new(),
            bits: 0,
            decoded: kdec,
        },
    };
    out.push(match verify(&bad_spec, &mixed) {
        Err(OracleError::Shape(m)) if m.contains("two-hop link is broken") => ok(
            "two-hop-link-NEGATIVE",
            format!("a one-bit key drift is correctly REFUSED - {}", m.lines().next().unwrap_or("")),
        ),
        Ok(()) => no(
            "two-hop-link-NEGATIVE",
            "a card whose key differs from the contact's +0x08 was ACCEPTED. Hop 2 would return \
             NULL and the family byte would stay 0xFF - verify() no longer tests the equality the \
             whole chain turns on."
                .to_string(),
        ),
        Err(e) => no("two-hop-link-NEGATIVE", format!("refused, but not for the link: {e}")),
    });

    // 9. ⭐ NEGATIVE - a name that fills the array leaves no terminator.
    let long = RosterSpec {
        display_name: "X".repeat(CONTACT_NAME_UNITS),
        ..RosterSpec::default()
    };
    out.push(match long.contact_object() {
        Err(OracleError::Shape(m)) if m.contains("terminator") => ok(
            "name-overflow-NEGATIVE",
            format!("a {CONTACT_NAME_UNITS}-unit name is correctly REFUSED"),
        ),
        Ok(_) => no(
            "name-overflow-NEGATIVE",
            format!(
                "a {CONTACT_NAME_UNITS}-unit name was accepted. The array holds \
                 {CONTACT_NAME_UNITS} units *including* the NUL, so the widget would read past it."
            ),
        ),
        Err(e) => no("name-overflow-NEGATIVE", format!("refused, but not for length: {e}")),
    });

    // 9b. ⭐ NEGATIVE - the clan tag, and this one is an OVER-READ guard, not a fit check.
    //     `0xA3DCE1 call 0x65A00(widget+0x158, contact+0x46, 0x11)` will read up to 16 source units
    //     from a 5-unit array unless it hits a NUL first. 4 characters + terminator is the bound
    //     that keeps the client inside our 80-byte object.
    let full_clan = RosterSpec {
        clan_tag: "X".repeat(CONTACT_CLAN_UNITS),
        ..RosterSpec::default()
    };
    out.push(match full_clan.contact_object() {
        Err(OracleError::Shape(m)) if m.contains("terminator") => ok(
            "clan-overread-NEGATIVE",
            format!(
                "a {CONTACT_CLAN_UNITS}-unit clan tag is correctly REFUSED - with no NUL inside \
                 the 5-unit array, 0x65A00's capacity of 0x11 reads {} units past the object",
                17 - 1 - CONTACT_CLAN_UNITS
            ),
        ),
        Ok(_) => no(
            "clan-overread-NEGATIVE",
            format!(
                "a {CONTACT_CLAN_UNITS}-unit clan tag was accepted. The client copies from +0x46 \
                 with a DESTINATION capacity of 0x11 = 17 and stops only at the source's NUL, so \
                 an unterminated 5-unit tag makes it read into the next registry slot."
            ),
        ),
        Err(e) => no("clan-overread-NEGATIVE", format!("refused, but not for length: {e}")),
    });

    // 10. ⭐ NEGATIVE - a zero contact key.
    let zero = RosterSpec { contact_key: 0, ..RosterSpec::default() };
    out.push(match zero.contact_object() {
        Err(OracleError::Shape(m)) if m.contains("early-returns") => {
            ok("zero-key-NEGATIVE", "contact_key = 0 is correctly REFUSED".to_string())
        }
        Ok(_) => no(
            "zero-key-NEGATIVE",
            "contact_key = 0 was accepted. 0x57F6F0 early-returns on `*contact == 0` before it \
             touches family 2, so that object could never be found."
                .to_string(),
        ),
        Err(e) => no("zero-key-NEGATIVE", format!("refused, but not for the zero key: {e}")),
    });

    // 11. ⭐ NEGATIVE - the packed-colour bitfield. THIS is what proves the round trip
    //     discriminates: if it ever passes, `round_trip` has stopped comparing bytes.
    let mut o = ObjectOracle::new(CARD_TAG)?;
    let mut spill = spec.card_object()?;
    spill[OFF_CARD_COLOUR_3BIT] = 0x07;
    out.push(match o.round_trip(&spill, 0x1000) {
        Err(OracleError::Shape(m)) if m.contains(&format!("+{CARD_COLOUR_SPILL:#05x}")) => ok(
            "colour-bitfield-NEGATIVE",
            format!("+{OFF_CARD_COLOUR_3BIT:#04x} = 0x07 correctly REFUSED - {m}"),
        ),
        Ok(_) => no(
            "colour-bitfield-NEGATIVE",
            format!(
                "0x07 in the 3-bit field at +{OFF_CARD_COLOUR_3BIT:#04x} round-tripped. It must \
                 not: the value spills into +{CARD_COLOUR_SPILL:#04x}. Either 0x808018E5 changed \
                 or round_trip has stopped comparing bytes, and every positive control above is \
                 then decoration."
            ),
        ),
        Err(e) => no("colour-bitfield-NEGATIVE", format!("refused, but not where expected: {e}")),
    });

    // 12/13. The emblem sentinel, in both directions: 0xFFFF is empty, 0x0000 is a CLAIM.
    let mut o = ObjectOracle::new(CARD_TAG)?;
    for (name, val, what) in [
        ("emblem-sentinel", EMPTY_CONTENT_INDEX, "0xFFFF survives as the empty marker"),
        ("emblem-zero-is-valid", 0u16, "0x0000 survives too - it is content index 0, not 'absent'"),
    ] {
        let mut ob = spec.card_object()?;
        ob[OFF_CARD_EMBLEM..OFF_CARD_EMBLEM + 2].copy_from_slice(&val.to_le_bytes());
        out.push(match o.round_trip(&ob, 0x1000) {
            Ok((_, d))
                if u16::from_le_bytes(
                    d.object[OFF_CARD_EMBLEM..OFF_CARD_EMBLEM + 2].try_into().unwrap(),
                ) == val =>
            {
                ok(name, format!("+{OFF_CARD_EMBLEM:#04x} = {val:#06x}: {what}"))
            }
            Ok((_, d)) => no(
                name,
                format!(
                    "wrote {val:#06x}, decoded {:#06x}",
                    u16::from_le_bytes(
                        d.object[OFF_CARD_EMBLEM..OFF_CARD_EMBLEM + 2].try_into().unwrap()
                    )
                ),
            ),
            Err(e) => no(name, e.to_string()),
        });
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

    /// The derivation must reproduce the two constants `d1-responder` already ships before its
    /// third answer is believed.
    #[test]
    fn instance_checksums_reproduce_the_shipped_constants() {
        if !crate::test_inputs::reference_eboot() {
            return;
        }
        let image = Image::open_reference().expect("pinned eboot");
        let idx = descriptors::index(&image).expect("descriptor index");
        assert_eq!(instance_type(&image, &idx, CONTACT_TAG).unwrap(), CONTACT_INSTANCE_TYPE);
        assert_eq!(instance_type(&image, &idx, CARD_TAG).unwrap(), CARD_INSTANCE_TYPE);
        assert_eq!(instance_type(&image, &idx, PEER_TAG).unwrap(), PEER_INSTANCE_TYPE);
    }

    #[test]
    fn the_two_size_claims_for_family_0_type_1_are_one_number() {
        assert_eq!(PEER_SIZE, 0xA48);
    }

    /// The clan bound is an over-read guard: `0x65A00`'s capacity at `0xA3DCE1` is 17 against a
    /// 5-unit array, so an unterminated tag reads into the next registry slot.
    #[test]
    fn a_clan_tag_that_fills_the_array_is_refused() {
        let s = RosterSpec { clan_tag: "X".repeat(CONTACT_CLAN_UNITS), ..Default::default() };
        assert!(matches!(s.contact_object(), Err(OracleError::Shape(_))));
        let s = RosterSpec { clan_tag: "X".repeat(CONTACT_CLAN_UNITS - 1), ..Default::default() };
        assert!(s.contact_object().is_ok());
    }

    /// Family 0/0 is family 2/0's twin - the claim a future family-0 build would rest on.
    #[test]
    fn family_0_type_0_has_the_same_tree_as_family_2_type_0() {
        if !crate::test_inputs::reference_eboot() {
            return;
        }
        let image = Image::open_reference().expect("pinned eboot");
        let idx = descriptors::index(&image).expect("descriptor index");
        assert_eq!(desc(&idx, PEER_CONTACT_TAG).unwrap().struct_size as usize, PEER_CONTACT_SIZE);
        assert_eq!(
            instance_type(&image, &idx, PEER_CONTACT_TAG).unwrap(),
            PEER_CONTACT_INSTANCE_TYPE
        );
        for i in 0..4 {
            let a = field(&image, &idx, CONTACT_TAG, i).unwrap();
            let b = field(&image, &idx, PEER_CONTACT_TAG, i).unwrap();
            assert_eq!((a.offset, a.kind, a.child_tag), (b.offset, b.kind, b.child_tag), "field {i}");
        }
    }

    #[test]
    fn a_name_that_fills_the_array_is_refused() {
        let s = RosterSpec { display_name: "X".repeat(CONTACT_NAME_UNITS), ..Default::default() };
        assert!(matches!(s.contact_object(), Err(OracleError::Shape(_))));
        let s = RosterSpec { display_name: "X".repeat(CONTACT_NAME_UNITS - 1), ..Default::default() };
        assert!(s.contact_object().is_ok());
    }

    #[test]
    fn a_zero_key_is_refused_in_both_objects() {
        assert!(RosterSpec { contact_key: 0, ..Default::default() }.contact_object().is_err());
        assert!(RosterSpec { card_key: 0, ..Default::default() }.contact_object().is_err());
        assert!(RosterSpec { card_key: 0, ..Default::default() }.card_object().is_err());
    }

    #[test]
    fn fixture_names_round_trip() {
        assert_eq!(parse_fixture_name("f2t0-d100000000000001.bin"), Some((2, 0, 0xD100_0000_0000_0001)));
        assert_eq!(parse_fixture_name("f2t1-d100000000000501.bin"), Some((2, 1, 0xD100_0000_0000_0501)));
        assert_eq!(parse_fixture_name("f0t1-d100000000000501.bin"), Some((0, 1, 0xD100_0000_0000_0501)));
        // Sorting must put type 0 before type 1 within a family, which is the send order.
        let mut v = vec!["f2t1-a.bin", "f2t0-a.bin"];
        v.sort();
        assert_eq!(v, vec!["f2t0-a.bin", "f2t1-a.bin"]);
        // Anything else is a refusal, never a default.
        for bad in ["roster.bin", "f2-0-x.bin", "fXt0-1.bin", "f2t0-zz.bin", "f2t0-1.txt"] {
            assert_eq!(parse_fixture_name(bad), None, "{bad} must not parse");
        }
    }

    /// The whole control set, so `cargo test` and `d1-oracle roster controls` cannot drift.
    #[test]
    fn controls_pass() {
        if !crate::test_inputs::selected_eboot() {
            return;
        }
        let cs = controls().expect("controls ran");
        let failed: Vec<_> =
            cs.iter().filter(|c| !c.pass).map(|c| format!("{}: {}", c.name, c.detail)).collect();
        assert!(failed.is_empty(), "{failed:#?}");
    }
}
