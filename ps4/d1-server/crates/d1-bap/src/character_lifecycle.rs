//! The four **character-lifecycle** investment messages - create / delete / login / signout.
//!
//! ⭐ These are the client→server *commands* of `THE-BLOCKS.md` **B11**, the direction nobody in
//! this tree had implemented. Character creation and selection are client-side *behaviour*, but the
//! cloud **tracks** them: a Guardian lives in the Worldserver's objects, not in a console save. So
//! these four are our deliverable.
//!
//! # 1. The ids, and how they were resolved `[C]`
//!
//! Not by guessing and not by a call-site scan. The shipped investment-message definition table
//! (`capture-rig/schemas/investment-message-definitions.bin`, tag `0x80A5E37A`, 101 records of
//! `0x40` bytes) carries a **name hash at `+0x00`** of every record. The hash is the same **FNV-1**
//! this project already pinned (`fnv1("platform_signin") == 0xE9D46191`, a `d1-eboot controls`
//! check). Hashing candidate names and matching against the table is exact - a 32-bit preimage
//! match on a 101-entry table is not a coincidence, and five of the six names in the 5xx block
//! resolved at once, which is a much stronger joint signal than any single hit:
//!
//! | id | name (FNV-1 preimage) | hash |
//! |---:|---|---|
//! | **501** | **`login_create_character`** | `0xE906221F` |
//! | **502** | **`login_delete_character`** | `0xF721133E` |
//! | 503 | `login_account` | `0x0C662090` |
//! | **504** | **`login_character`** | `0x7D6146CC` |
//! | **505** | **`signout_character`** | `0x1CE11BB6` |
//! | 506 | `customize_character` | `0x27B12A16` |
//!
//! (Also resolved on the same pass, recorded here because they are free and useful:
//! 201 `fetch_account`, 202 `fetch_character`, 203 `fetch_item`, 204 `fetch_time`,
//! 403 `inventory_equip_item`, 601 `loot_pickup_item`.)
//!
//! ## ⛔ This RETRACTS "501 is `login_character`"
//!
//! `historicaldocs/design/server-object-model.md`, `historicaldocs/design/rsat-character-object.md` §"0x551040 sends server
//! message 501 (`create_character`)" and `historicaldocs/design/character-and-orbit.md` §26.55 disagree with each
//! other about 501. **The name table settles it: 501 is CREATE, 504 is LOGIN.** Both prose readings
//! that called 501 `login_character` are wrong. The independent cross-check is the client's own
//! encoder: `d1-oracle message --tag 0x8080185d --msgid 501` emits the wire id `01f5` = 501 in front
//! of 501's *own* response schema, and that schema returns a `u64` - a login does not mint an id, a
//! create does.
//!
//! # 2. The schemas `[C]` - read from the descriptor tables, confirmed at the codec
//!
//! ```text
//! 501 login_create_character   req 0x80801860 (60 B, 3 fields, 397 bits)  rsp 0x8080185D (16 B, 2 fields, 100 bits)
//! 502 login_delete_character   req 0x80801939 ( 8 B, 1 field,   64 bits)  rsp 0x808017F9 ( 8 B, 1 field,  36 bits)
//! 504 login_character          req 0x808017CE ( 8 B, 1 field,   64 bits)  rsp 0x80801845 ( 8 B, 1 field,  36 bits)
//! 505 signout_character        req 0x808017E5 ( 1 B, 0 fields,   0 bits)  rsp 0x80801886 ( 8 B, 1 field,  36 bits)
//! 506 customize_character      req 0x80801814 (40 B, 3 fields)            rsp 0x80801736 ( 8 B, 1 field,  36 bits)
//! ```
//!
//! 502 / 504 / 505 / 506 all answer with the *same* `0x80801796` composite (`{i4, i32}`) that 504,
//! 701, 702 and 104 already use, so their body is the familiar `18 00 00 00 00`. Only **501** is
//! different: it answers `{0x80801796 result, u64}` and **the `u64` is the new character's SOID**
//! (`[I]`, strong - it is the only payload field, and creation is the one operation that mints an
//! id; every other id in this protocol is chosen by the server and echoed).
//!
//! # 3. ⭐ `s_create_character_server_message_request_arguments` - the client telling us what a
//!    Guardian is
//!
//! 501's request `0x80801860` is **three blocks and nothing else** - 27 + 234 + 136 = **397 bits**,
//! exactly the total the client's own encoder emits, so there is no top-level presence bit and no
//! slack:
//!
//! ```text
//! C offset  bits      tag           what
//! +0x00     0..27     0x808017FC    the IDENTITY TRIPLE  {race, gender, class}
//! +0x04     27..261   0x8080181A    the 36-byte, 12-field CUSTOMIZATION block
//! +0x28     261..397  0x80804257    char[17] - 16 bytes plus a terminator
//! ```
//!
//! ⭐⭐ **`0x808017FC` and `0x8080181A` are literally the first two members of the character
//! object.** The descriptor index says so: both tags are referenced by `0x80801954` (the family-3/1
//! character-select card), `0x80801714` (the family-4/1 profile), `0x808017FD` (family 0/1),
//! `0x808017A5`, `0x80801814` (message 506) **and** `0x80801860` (this request). So the create
//! request is not a bespoke form - it is the *prefix of the character object itself*. That converts
//! `historicaldocs/design/level-light-customization.md` §5.1's block identification from a structural argument into
//! a contract: whatever the client sends here is, by construction, what a character is made of.
//!
//! `0x80804257` (→ `0x808041CC`) is referenced by **`0x80801860` alone** - it exists only for
//! creation. Its wire form is 17 × 8 bits with no length prefix and no presence bits, i.e. a
//! fixed `char[17]`.
//!
//! ⭐ **It is the signed-in user's PSN Online ID** - 16 ASCII characters plus a NUL. `[C]`, and it
//! does **not** come from the CUI form at all:
//!
//! ```text
//! 0x996FB0  c_create_character_server_message_component::vfunc[21]
//!             FUN_00244C30()            -> the active controller index
//!             FUN_00244650(idx, &rec)   -> the "Platform User" record (table `DAT_02638CD8`,
//!                                          literal tag "Platform User", stride 0x198)
//!             FUN_00244EA0(&rec, &name) -> copies 17 bytes from record + 0x3C
//!             FUN_00551040(this+0xB0, this+0xB4, name)   -> sends 501 (`mov edi, 0x1f5`)
//! ```
//!
//! Three independent tells that it is a NUL-terminated ASCII string rather than an opaque id:
//! the copy is `{u64, u64, u8}` (the `SceNpOnlineId` shape); the field's initialiser writes **one**
//! zero byte (`FUN_0006C520` is literally `*param_1 = 0`) - you only clear a 17-byte blob with a
//! single NUL if it is a string; and `cui::c_investment_login_info_component_v2::vfunc[14]`
//! (`0xA110F0`) hands the same blob to `FUN_00248180(dst, src, 0x11)`, a narrow→wide converter that
//! does `strnlen(src, 0x11)` and substitutes **U+25A1 WHITE SQUARE** for any high-bit byte, i.e. it
//! renders it as a name on screen.
//!
//! ⇒ **The create request carries the account's display name, not a per-character name.** D1
//! characters are unnamed; this is the platform identity being stamped onto the creation.
//!
//! # 3.1 The senders, by address `[C]`
//!
//! All four funnel through one core, `FUN_00551FB0(msg_id, flag, payload, dest)` at **`0x551FB0`**
//! (`networking:investment:server_message_interface`). The message id is a literal immediate, which
//! is a second, fully independent confirmation of the name-hash derivation:
//!
//! | component | event slot | sender | immediate |
//! |---|---|---|---|
//! | `c_create_character_server_message_component` | vt[21] `0x996FB0` | `0x551040` | `0x1F5` = **501** |
//! | `c_cui_delete_character_server_message_component` | vt[21] `0x99B3A0` opens a **dialog**; vt[14] `0x99B2E0` sends | `0x5510C0` | `0x1F6` = **502** |
//! | `c_login_character_server_message_component` | vt[21] `0x9B82A0` (a 2-instruction **adjustor thunk**) | `0x551640` | `0x1F8` = **504** |
//! | `c_cui_signout_character_server_message_component` | vt[21] `0x9B2660` opens a **dialog**; vt[14] `0x9B25C0` sends | `0x551C80` | `0x1F9` = **505** |
//!
//! ⚠ Two traps worth keeping: the vtables are **22 slots (0..21)**, not 27; and delete/signout do
//! **not** send on the UI event - slot 21 raises a modal confirm and slot 14 polls it, so a probe on
//! slot 21 alone would record "the user asked" and never see the message.
//! ⚠ `0x9B82A0` has no atlas row and no decompiled file and `disasm` refuses it - it is
//! `add rdi,0xB0; jmp 0x551640`. A call-graph scan reports the login component as having **no
//! sender**. That is a tail merge, not an absence (the fifth time this pattern has cost this
//! project a session).
//!
//! Bonus, from the same neighbourhood: `0x5515D0` sends `0x1F7` = **503** `login_account` with flag
//! **1** (all four others use flag 0) and a 48-byte payload copied wholesale from the Platform User
//! record at `+0x08..+0x37` - which is the answer `historicaldocs/design/PUZZLE-503-account-login.md` was after.
//!
//! # 4. The encoding rules, and why they are not hand-derived `[C]`
//!
//! ⚠ CLAUDE.md: *"A self-consistent algebraic control can close on the WRONG answer."* Every bias
//! below was **measured by driving the client's own codec** (`d1-oracle object --tag … --obj …`,
//! encoder `0x2C9000`), never inferred:
//!
//! | descriptor kind | width | encoding, as measured |
//! |---|---|---|
//! | `0x103` | 9 | 1 presence bit, then `value + 0x80` (an `i8`) |
//! | `0x3` @ 8 bits | 8 | `value + 0x80` (an `i8`), **no** presence bit |
//! | `0x4` @ 16 bits | 16 | `value + 0x8000` (an `i16`) |
//! | `0x9` @ 32 bits | 32 | **raw**, no bias - the hash-shaped field |
//! | `0x5` @ 32 bits | 32 | `value + 0x8000_0000` |
//! | `0x3` @ 4 bits | 4 | `value + 1` - so **`-1` encodes as 0**, the "none" sentinel |
//! | `0x3` @ 2 bits | 2 | `value + 1` - same rule |
//! | `0x6` @ 64 bits | 64 | **raw** big-endian, no bias |
//!
//! ⚠ The bias is **per field, not per width**: an 8-bit `i8` biases by `0x80` while a 4-bit field
//! biases by `1`. That is a range minimum stored in the descriptor, and it is exactly the kind of
//! rule that a "consistent" arithmetic model gets wrong. The decoder below therefore carries an
//! **explicit per-field bias table**, and [`tests`] replays three codec-produced vectors - including
//! the extreme `{0xFF, 0x7F, 0x80}` identity and a full 16-character name - through it. A negative
//! control (`decoder_rejects_a_wrong_bias`) must keep failing.
//!
//! Independent corroboration of block A: [`crate::character_rsat`] already emits
//! `1 · u8(race+0x80) · 1 · u8(gender+0x80) · 1 · u8(class+0x80)` for the character object, written
//! by a different session from different evidence. Same nine-bit element, same bias, same order.
//!
//! # 5. Reach
//!
//! Everything here is `static-client-inference` plus `synthetic-minimal`. **No client has ever sent
//! us a 501.** The response bodies are the client's own encoder's output for an object we chose; the
//! request decoder has never seen a real request. That is precisely why
//! [`CreateCharacterRequest::transcript`] keeps the **raw bitstream** alongside the decode: when the
//! first real one arrives, the hex is the evidence and the decode is merely a convenience that can
//! be re-derived.

use std::fmt::Write as _;

/// `login_create_character` - FNV-1 `0xE906221F`.
pub const CREATE_CHARACTER: u16 = 501;
/// `login_delete_character` - FNV-1 `0xF721133E`.
pub const DELETE_CHARACTER: u16 = 502;
/// `login_character` - FNV-1 `0x7D6146CC`. (Already answered by the responder; named here so all
/// four live in one place.)
pub const LOGIN_CHARACTER: u16 = 504;
/// `signout_character` - FNV-1 `0x1CE11BB6`.
pub const SIGNOUT_CHARACTER: u16 = 505;
/// `customize_character` - FNV-1 `0x27B12A16`. Not one of the four, but it carries the *same*
/// 36-byte customization block and is the obvious sibling.
pub const CUSTOMIZE_CHARACTER: u16 = 506;

/// FNV-1 (not FNV-1a) over ASCII, the hash the definition table's `+0x00` word holds.
///
/// Pinned by `d1-eboot controls`: `fnv1("platform_signin") == 0xE9D46191`.
pub fn fnv1(s: &str) -> u32 {
    let mut h: u32 = 0x811C_9DC5;
    for b in s.as_bytes() {
        h = h.wrapping_mul(0x0100_0193);
        h ^= *b as u32;
    }
    h
}

/// Total width of 501's request bitstream. Measured at the client's own encoder.
pub const CREATE_REQUEST_BITS: usize = 397;
/// `ceil(397 / 8)`.
pub const CREATE_REQUEST_BYTES: usize = 50;

/// The 36-byte, 12-field block `0x8080181A`, shared by the character card, the profile, message 506
/// and this request.
///
/// Field *names* are `[U]`: `historicaldocs/design/level-light-customization.md` §5.2 maps the shape 1:1 onto D1's
/// public `DestinyCharacterCustomization`, but which scalar is which was never resolved - and its
/// candidate for `[10]` is **refuted** (block `+0x1C`, absolute `+0x28` in the profile, is
/// `last_orbited_destination`, confirmed by a live read and by orbit rendering Old Russia). So the
/// fields are numbered, not named, and the offsets are the contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CustomizationBlock {
    /// `+0x00` `i16`
    pub f0: i16,
    /// `+0x02` `i8`
    pub f1: i8,
    /// `+0x03` `i8`
    pub f2: i8,
    /// `+0x04` `i8`
    pub f3: i8,
    /// `+0x06` `i16`
    pub f4: i16,
    /// `+0x08` `i16`
    pub f5: i16,
    /// `+0x0A` `i16`
    pub f6: i16,
    /// `+0x0C` `[3] × i16`
    pub f7: [i16; 3],
    /// `+0x12` `[3] × i16`
    pub f8: [i16; 3],
    /// `+0x18` `i16`
    pub f9: i16,
    /// `+0x1C` **raw** `u32` - a **32-bit FNV-1 name hash**. `[C]`: the component's constructor
    /// (`0x996D50`) initialises struct `+0x20` (= block `+0x1C`) to `0x811C9DC5`, the FNV-1 basis,
    /// i.e. the hash of the empty string, i.e. **unset**. In the *profile* this same field is
    /// `last_orbited_destination` - also an FNV-1 name hash. So the wire type is "a name hash" and
    /// the *referent* depends on the container. `[U]` which name space it indexes in a create
    /// request; `historicaldocs/design/level-light-customization.md` §5.2's `personality` candidate is consistent
    /// with the shape but unproven.
    ///
    /// ⚠ Do not read `0x551040`'s `mov dword [rbp-0x38], 0x811C9DC5` as the wire value - the packer
    /// `0x27DB90` unconditionally overwrites it from the source block. Dead defensive init.
    pub f10_raw32: u32,
    /// `+0x20`, 2 bits, biased by 1 - so `-1` is expressible and means "none".
    pub f11: i8,
}

/// Decoded `s_create_character_server_message_request_arguments`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateCharacterRequest {
    /// Presence bit of the `race` element (block `0x808017FC` element 0).
    pub race_present: bool,
    /// `+0x00`. Wire order is `{race, gender, class}` - see [`crate::Identity`], which pins the same
    /// order for the character object and is ⚠ **not** the `{class, race, gender}` order the public
    /// manifest lists.
    pub race: i8,
    pub gender_present: bool,
    /// `+0x01`
    pub gender: i8,
    pub class_present: bool,
    /// `+0x02`
    pub class: i8,
    /// `+0x04`, tag `0x8080181A`.
    pub customization: CustomizationBlock,
    /// `+0x28`, tag `0x80804257`. A fixed `char[17]` - the **PSN Online ID**, copied at send time
    /// from the Platform User record `+0x3C` (`0x244EA0`), not from the CUI form. `[C]`
    pub name: [u8; 17],
    /// The bytes exactly as they arrived. **This is the evidence**; the fields above are a reading.
    pub raw: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// Fewer than [`CREATE_REQUEST_BYTES`] bytes of argument bitstream.
    TooShort { got: usize, want: usize },
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeError::TooShort { got, want } => write!(
                f,
                "create-character request is {got} bytes, need {want} \
                 (397 bits); a short body decodes 'fine' and silently reads zeros past the end"
            ),
        }
    }
}

impl std::error::Error for DecodeError {}

/// MSB-first bit reader. The only bit order this protocol uses.
struct BitReader<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> BitReader<'a> {
    fn new(b: &'a [u8]) -> Self {
        BitReader { b, i: 0 }
    }
    fn take(&mut self, n: usize) -> u64 {
        let mut v: u64 = 0;
        for _ in 0..n {
            let byte = self.b[self.i >> 3];
            let bit = (byte >> (7 - (self.i & 7))) & 1;
            v = (v << 1) | bit as u64;
            self.i += 1;
        }
        v
    }
    /// A biased signed leaf: the wire carries `value + bias`.
    fn signed(&mut self, n: usize, bias: i64) -> i64 {
        self.take(n) as i64 - bias
    }
}

/// Where a Guardian's look sits in the objects that carry it: the peer view `0x808017FD` and the
/// profile's character record `0x80801714` (at `+0` of `0x80801743`) both hold the identity
/// `0x808017FC` at `+0x08` and the customization `0x8080181A` at `+0x0C`
/// (`ps4/fixtures/56578/rtti/fields.tsv`), as this request does at `+0x00` and `+0x04`.
pub const LOOK_AT: usize = 0x08;
/// [`CreateCharacterRequest::look_image`]: the identity (4 bytes with its pad) and customization
/// fields f0..f9 (block `+0x00..+0x1C`).
pub const LOOK_BYTES: usize = 0x20;

impl CreateCharacterRequest {
    /// The look the player chose, as the bytes `[LOOK_AT, LOOK_AT + LOOK_BYTES)` of a peer view or
    /// profile object: race, gender and class (then the pad), and customization f0..f9 with the
    /// block's pads at 0. f10 (a name hash: the profile's last-orbited destination) and f11 (the
    /// wear-helmet field, absolute `+0x2C`) are not part of it; the target keeps its own.
    pub fn look_image(&self) -> [u8; LOOK_BYTES] {
        let mut b = [0u8; LOOK_BYTES];
        b[0] = self.race as u8;
        b[1] = self.gender as u8;
        b[2] = self.class as u8;
        let c = &self.customization;
        let mut at = |o: usize, v: &[u8]| b[4 + o..4 + o + v.len()].copy_from_slice(v);
        at(0x00, &c.f0.to_le_bytes());
        at(0x02, &[c.f1 as u8, c.f2 as u8, c.f3 as u8]);
        at(0x06, &c.f4.to_le_bytes());
        at(0x08, &c.f5.to_le_bytes());
        at(0x0A, &c.f6.to_le_bytes());
        for (i, v) in c.f7.iter().enumerate() {
            at(0x0C + 2 * i, &v.to_le_bytes());
        }
        for (i, v) in c.f8.iter().enumerate() {
            at(0x12 + 2 * i, &v.to_le_bytes());
        }
        at(0x18, &c.f9.to_le_bytes());
        b
    }

    /// Decode the argument bitstream of investment message **501**.
    ///
    /// `bits` is everything after the two-byte message id in the `0x000A` body.
    ///
    /// ⚠ Refuses a short body rather than reading zeros past the end - the §26.67 under-length trap,
    /// where an under-length body "decodes" fine and silently yields zeros.
    pub fn decode(bits: &[u8]) -> Result<Self, DecodeError> {
        if bits.len() < CREATE_REQUEST_BYTES {
            return Err(DecodeError::TooShort {
                got: bits.len(),
                want: CREATE_REQUEST_BYTES,
            });
        }
        let mut r = BitReader::new(bits);

        // ── block A: 0x808017FC, 3 × {presence, i8 bias 0x80} = 27 bits ──────────────────────
        let race_present = r.take(1) == 1;
        let race = r.signed(8, 0x80) as i8;
        let gender_present = r.take(1) == 1;
        let gender = r.signed(8, 0x80) as i8;
        let class_present = r.take(1) == 1;
        let class = r.signed(8, 0x80) as i8;
        debug_assert_eq!(r.i, 27);

        // ── block B: 0x8080181A, 234 bits ────────────────────────────────────────────────────
        let customization = CustomizationBlock {
            f0: r.signed(16, 0x8000) as i16,
            f1: r.signed(8, 0x80) as i8,
            f2: r.signed(8, 0x80) as i8,
            f3: r.signed(8, 0x80) as i8,
            f4: r.signed(16, 0x8000) as i16,
            f5: r.signed(16, 0x8000) as i16,
            f6: r.signed(16, 0x8000) as i16,
            f7: [
                r.signed(16, 0x8000) as i16,
                r.signed(16, 0x8000) as i16,
                r.signed(16, 0x8000) as i16,
            ],
            f8: [
                r.signed(16, 0x8000) as i16,
                r.signed(16, 0x8000) as i16,
                r.signed(16, 0x8000) as i16,
            ],
            f9: r.signed(16, 0x8000) as i16,
            f10_raw32: r.take(32) as u32,
            f11: r.signed(2, 1) as i8,
        };
        debug_assert_eq!(r.i, 27 + 234);

        // ── block C: 0x80804257 → 0x808041CC, char[17], 136 bits ─────────────────────────────
        let mut name = [0u8; 17];
        for slot in name.iter_mut() {
            *slot = (r.signed(8, 0x80) as i8) as u8;
        }
        debug_assert_eq!(r.i, CREATE_REQUEST_BITS);

        Ok(CreateCharacterRequest {
            race_present,
            race,
            gender_present,
            gender,
            class_present,
            class,
            customization,
            name,
            raw: bits[..CREATE_REQUEST_BYTES].to_vec(),
        })
    }

    /// The `char[17]` as a lossy string, up to the first NUL. Convenience only - [`Self::name`] is
    /// the datum.
    pub fn name_str(&self) -> String {
        let end = self.name.iter().position(|&c| c == 0).unwrap_or(self.name.len());
        String::from_utf8_lossy(&self.name[..end]).into_owned()
    }

    /// ⭐ **The deliverable.** A durable, self-describing transcript of one real create-character
    /// request: the raw bitstream first (that is the evidence), then the reading.
    ///
    /// Written to the run directory so it outlives the process. A decode can be re-derived from the
    /// hex; the hex cannot be re-derived from a decode.
    pub fn transcript(&self, wire_body: &[u8]) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "# create_character transcript");
        let _ = writeln!(s, "#");
        let _ = writeln!(
            s,
            "# investment message {CREATE_CHARACTER} `login_create_character` \
             (FNV-1 {:#010x})",
            fnv1("login_create_character")
        );
        let _ = writeln!(s, "# request schema  0x80801860  60 B / 3 fields / 397 bits");
        let _ = writeln!(s, "# response schema 0x8080185D  16 B / 2 fields / 100 bits");
        let _ = writeln!(s, "#");
        let _ = writeln!(
            s,
            "# ⭐ THE RAW BITSTREAM IS THE EVIDENCE. Everything below it is a reading, produced by\
             \n# a decoder pinned against three vectors from the client's own encoder. If the two\
             \n# ever disagree, the hex wins."
        );
        let _ = writeln!(s, "#");
        let _ = writeln!(s, "# reach: observed-live (the client sent this) - but the boot that");
        let _ = writeln!(s, "#        produced it is synthetic_ancestor throughout.");
        let _ = writeln!(s);
        let _ = writeln!(s, "wire_body_len  {}", wire_body.len());
        let _ = writeln!(s, "wire_body      {}", hex(wire_body));
        let _ = writeln!(s, "arg_bitstream  {}", hex(&self.raw));
        let _ = writeln!(s);
        let _ = writeln!(s, "[identity]  tag 0x808017FC @ +0x00, bits 0..27");
        let _ = writeln!(
            s,
            "race    {:>6}   present={}",
            self.race, self.race_present
        );
        let _ = writeln!(
            s,
            "gender  {:>6}   present={}",
            self.gender, self.gender_present
        );
        let _ = writeln!(
            s,
            "class   {:>6}   present={}",
            self.class, self.class_present
        );
        let _ = writeln!(s);
        let _ = writeln!(s, "[customization]  tag 0x8080181A @ +0x04, bits 27..261");
        let c = &self.customization;
        let _ = writeln!(s, "f0  @+0x04  i16      {:>6}", c.f0);
        let _ = writeln!(s, "f1  @+0x06  i8       {:>6}", c.f1);
        let _ = writeln!(s, "f2  @+0x07  i8       {:>6}", c.f2);
        let _ = writeln!(s, "f3  @+0x08  i8       {:>6}", c.f3);
        let _ = writeln!(s, "f4  @+0x0a  i16      {:>6}", c.f4);
        let _ = writeln!(s, "f5  @+0x0c  i16      {:>6}", c.f5);
        let _ = writeln!(s, "f6  @+0x0e  i16      {:>6}", c.f6);
        let _ = writeln!(s, "f7  @+0x10  i16[3]   {:?}", c.f7);
        let _ = writeln!(s, "f8  @+0x16  i16[3]   {:?}", c.f8);
        let _ = writeln!(s, "f9  @+0x1c  i16      {:>6}", c.f9);
        let _ = writeln!(
            s,
            "f10 @+0x20  u32 raw  {:#010x}{}",
            c.f10_raw32,
            if c.f10_raw32 == 0x811C_9DC5 {
                "   <- FNV-1 basis = hash of the empty string = UNSET"
            } else {
                ""
            }
        );
        let _ = writeln!(s, "f11 @+0x24  i8 (2b)  {:>6}", c.f11);
        let _ = writeln!(s);
        let _ = writeln!(
            s,
            "[online_id]  tag 0x80804257 @ +0x28, bits 261..397, char[17] - the PSN Online ID,\n\
             #            copied from the Platform User record +0x3C by 0x244EA0"
        );
        let _ = writeln!(s, "bytes   {}", hex(&self.name));
        let _ = writeln!(s, "ascii   {:?}", self.name_str());
        s
    }
}

fn hex(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len() * 2);
    for x in b {
        let _ = write!(s, "{x:02x}");
    }
    s
}

/// The `0x80801796` composite `{i4, i32}` at its zero value - 36 bits, then 4 bits of pad.
///
/// This is the whole body of 502 / 504 / 505 / 506's responses (and 104 / 701 / 702's). `[C]`: the
/// client's own encoder emits exactly these five bytes for a zeroed object at every one of those
/// response tags.
///
/// ⚠ The leading nibble is `1`, not `0`, because a 4-bit leaf is biased by 1 - `0` on the wire would
/// mean `-1`. Hand-writing `00 00 00 00 00` here would have been a *different value* that still
/// looks like "empty".
pub const RESULT_OK_BODY: [u8; 5] = [0x18, 0x00, 0x00, 0x00, 0x00];

/// Response body for **502** `login_delete_character`.
pub fn delete_character_response_body() -> Vec<u8> {
    RESULT_OK_BODY.to_vec()
}

/// Response body for **505** `signout_character`.
pub fn signout_character_response_body() -> Vec<u8> {
    RESULT_OK_BODY.to_vec()
}

/// Response body for **506** `customize_character`.
pub fn customize_character_response_body() -> Vec<u8> {
    RESULT_OK_BODY.to_vec()
}

/// Response body for **501** `login_create_character` - the result composite **plus the new
/// character's SOID**.
///
/// Schema `0x8080185D`: `[0] @+0x00 → 0x80801796` (36 bits), `[1] @+0x08` a raw 64-bit leaf.
/// 100 bits, zero-padded to 13 bytes.
///
/// Derived by driving the client's own encoder over a 16-byte object with `+0x08 = soid`, for four
/// distinct SOIDs including `0` and `u64::MAX`; the vectors are asserted in [`tests`].
pub fn create_character_response_body(new_character_soid: u64) -> Vec<u8> {
    let mut bits: Vec<bool> = Vec::with_capacity(104);
    // 0x80801796 at zero: a 4-bit leaf biased by 1 (so `0` encodes as `1`) then a 32-bit leaf
    // biased by 0x8000_0000 (so `0` encodes as 0x8000_0000).
    for i in (0..4).rev() {
        bits.push((1u64 >> i) & 1 == 1);
    }
    for i in (0..32).rev() {
        bits.push((0x8000_0000u64 >> i) & 1 == 1);
    }
    for i in (0..64).rev() {
        bits.push((new_character_soid >> i) & 1 == 1);
    }
    while bits.len() % 8 != 0 {
        bits.push(false);
    }
    bits.chunks(8)
        .map(|c| c.iter().fold(0u8, |a, &b| (a << 1) | b as u8))
        .collect()
}

/// The single `u64` argument of **504** `login_character` and **502** `login_delete_character`.
///
/// Both requests are one raw 64-bit big-endian leaf (`kind 0x6`) - no bias, no presence bit,
/// measured at the codec. Returns `None` rather than zero for a short body: a zero SOID and an
/// absent SOID are different claims and this protocol punishes conflating them.
pub fn character_soid_argument(bits: &[u8]) -> Option<u64> {
    if bits.len() < 8 {
        return None;
    }
    Some(u64::from_be_bytes(bits[..8].try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The name-hash preimages. This is the whole basis for the four ids, so it is a test, and it
    /// carries a **negative control**: `create_character` and `delete_character` - the names two
    /// design docs assumed - are NOT in the table, which is why the ids had to be re-derived.
    #[test]
    fn the_four_ids_come_from_fnv1_name_hashes() {
        assert_eq!(fnv1("platform_signin"), 0xE9D4_6191, "the pinned control");
        assert_eq!(fnv1("login_create_character"), 0xE906_221F);
        assert_eq!(fnv1("login_delete_character"), 0xF721_133E);
        assert_eq!(fnv1("login_character"), 0x7D61_46CC);
        assert_eq!(fnv1("signout_character"), 0x1CE1_1BB6);
        assert_eq!(fnv1("login_account"), 0x0C66_2090);
        assert_eq!(fnv1("customize_character"), 0x27B1_2A16);

        // NEGATIVE CONTROL - a control that only ever passes is decoration. These two names are
        // what `historicaldocs/design/server-object-model.md` and `historicaldocs/design/rsat-character-object.md` assumed, and
        // they hash to values that appear NOWHERE in the 101-row definition table.
        assert_ne!(fnv1("create_character"), 0xE906_221F);
        assert_ne!(fnv1("delete_character"), 0xF721_133E);
        assert_eq!(fnv1("create_character"), 0x7173_8EDD);
        assert_eq!(fnv1("delete_character"), 0x63FB_2504);
    }

    /// Vectors from the client's own encoder:
    /// `d1-oracle object --tag 0x80801860 --obj <60-byte C struct>`.
    /// Each pair is `(C object, emitted payload)`; the decoder must reproduce the object's fields.
    const VECTORS: &[(&str, &str)] = &[
        // all zero
        (
            "000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
            "c0603010001010101000100010001000100010001000100010001000000000000c0404040404040404040404040404040400",
        ),
        // race=1 gender=2 class=3, a fully populated customization block, online_id "Guardian"
        (
            "01020300020103040500070609080b0a01100210031001200220032019180000c59d1c8101000000477561726469616e000000000000000000000000",
            "c0e0b07020507090b0c0f10131417200320052007400340054007303302393b8b63faf0f97274f0f74040404040404040400",
        ),
        // ⭐ the extreme: race=-1, gender=127, class=-128, online_id filled to 16 chars.
        (
            "ff7f8000020103040500070609080b0a01100210031001200220032019180000c59d1c81010000005a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a00000000",
            "bfffe01020507090b0c0f10131417200320052007400340054007303302393b8b6d6d6d6d6d6d6d6d6d6d6d6d6d6d6d6d400",
        ),
    ];

    fn unhex(s: &str) -> Vec<u8> {
        let s: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
        (0..s.len() / 2)
            .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap())
            .collect()
    }

    /// The decoder reproduces the C object for every codec vector. This is the control that makes
    /// the per-field bias table evidence rather than arithmetic.
    #[test]
    fn decoder_reproduces_every_codec_vector() {
        for (obj_hex, payload_hex) in VECTORS {
            let obj = unhex(obj_hex);
            let payload = unhex(payload_hex);
            assert_eq!(obj.len(), 60, "the C struct is 60 bytes");
            assert_eq!(payload.len(), CREATE_REQUEST_BYTES);

            let d = CreateCharacterRequest::decode(&payload).expect("decodes");

            assert_eq!(d.race as u8, obj[0], "race @+0x00");
            assert_eq!(d.gender as u8, obj[1], "gender @+0x01");
            assert_eq!(d.class as u8, obj[2], "class @+0x02");
            assert!(d.race_present && d.gender_present && d.class_present);

            let c = &obj[4..40];
            let i16at = |o: usize| i16::from_le_bytes([c[o], c[o + 1]]);
            let cz = &d.customization;
            assert_eq!(cz.f0, i16at(0x00));
            assert_eq!(cz.f1 as u8, c[0x02]);
            assert_eq!(cz.f2 as u8, c[0x03]);
            assert_eq!(cz.f3 as u8, c[0x04]);
            assert_eq!(cz.f4, i16at(0x06));
            assert_eq!(cz.f5, i16at(0x08));
            assert_eq!(cz.f6, i16at(0x0a));
            assert_eq!(cz.f7, [i16at(0x0c), i16at(0x0e), i16at(0x10)]);
            assert_eq!(cz.f8, [i16at(0x12), i16at(0x14), i16at(0x16)]);
            assert_eq!(cz.f9, i16at(0x18));
            assert_eq!(
                cz.f10_raw32,
                u32::from_le_bytes([c[0x1c], c[0x1d], c[0x1e], c[0x1f]])
            );
            assert_eq!(cz.f11 as u8, c[0x20]);

            assert_eq!(&d.name[..], &obj[0x28..0x28 + 17], "char[17] @+0x28");
        }
    }

    /// The look image is the client's own C object, byte for byte, over identity and customization
    /// f0..f9 (`+0x00..+0x20`), for every codec vector.
    #[test]
    fn the_look_image_is_the_c_objects_identity_and_customization() {
        for (obj_hex, payload_hex) in VECTORS {
            let obj = unhex(obj_hex);
            let d = CreateCharacterRequest::decode(&unhex(payload_hex)).unwrap();
            assert_eq!(&d.look_image()[..], &obj[..LOOK_BYTES], "the C object's first 0x20 bytes");
        }
        // f10 (+0x20) and f11 (+0x24) lie past it.
        assert_eq!(LOOK_AT + LOOK_BYTES, 0x28, "the profile's last-orbited destination starts here");
    }

    /// ⚠ NEGATIVE CONTROL - the bias table must be load-bearing. If the 4-bit / 2-bit fields used
    /// the "obvious" `2^(n-1)` bias instead of the measured `+1`, this vector would decode
    /// differently. Assert the measured value, so a future "simplification" fails here.
    #[test]
    fn decoder_rejects_a_wrong_bias() {
        let (obj_hex, payload_hex) = VECTORS[1];
        let obj = unhex(obj_hex);
        let payload = unhex(payload_hex);
        let d = CreateCharacterRequest::decode(&payload).unwrap();

        // f11's C value is 1. The wire nibble-pair is `value + 1` = 2. The naive 2-bit bias (2)
        // would read 0. Assert we did NOT get the naive answer.
        assert_eq!(obj[4 + 0x20], 1);
        assert_eq!(d.customization.f11, 1);
        assert_ne!(d.customization.f11, 0, "the naive 2^(n-1) bias would give 0");

        // f10 is RAW - no bias at all. The naive i32 bias would flip the top bit.
        assert_eq!(d.customization.f10_raw32, 0x811C_9DC5);
        assert_ne!(d.customization.f10_raw32, 0x811C_9DC5 ^ 0x8000_0000);
    }

    /// A short body must be refused, not silently zero-extended (the §26.67 under-length trap).
    #[test]
    fn a_short_request_is_refused() {
        let e = CreateCharacterRequest::decode(&[0u8; 49]).unwrap_err();
        assert_eq!(
            e,
            DecodeError::TooShort {
                got: 49,
                want: CREATE_REQUEST_BYTES
            }
        );
        assert!(CreateCharacterRequest::decode(&[0u8; 50]).is_ok());
    }

    /// 501's response, against the client's own encoder for four SOIDs.
    /// `d1-oracle object --tag 0x8080185d --obj <16 B, +0x08 = soid>`.
    #[test]
    fn create_response_matches_the_native_encoder() {
        let cases: &[(u64, &str)] = &[
            (0x0000_0000_0000_0000, "18000000000000000000000000"),
            (0x0000_0000_0000_0101, "18000000000000000000001010"),
            (0xD100_0000_0000_0101, "180000000d1000000000001010"),
            (0xFFFF_FFFF_FFFF_FFFF, "180000000ffffffffffffffff0"),
        ];
        for (soid, want) in cases {
            let got = create_character_response_body(*soid);
            assert_eq!(
                hex(&got),
                *want,
                "501 response body for soid {soid:#018x}"
            );
            assert_eq!(got.len(), 13, "100 bits, padded to 13 bytes");
        }
        // The 36-bit prefix is exactly the `RESULT_OK_BODY` composite, nibble-shifted.
        assert_eq!(&create_character_response_body(0)[..4], &RESULT_OK_BODY[..4]);
    }

    /// 502 / 505 / 506 all answer with the same five bytes the native encoder emits for their own
    /// response tags (`0x808017F9`, `0x80801886`, `0x80801736`) - and it is the same body 504 uses
    /// (`0x80801845`), which is already pinned elsewhere in this crate.
    #[test]
    fn the_result_only_responses_match_the_native_encoder() {
        assert_eq!(hex(&delete_character_response_body()), "1800000000");
        assert_eq!(hex(&signout_character_response_body()), "1800000000");
        assert_eq!(hex(&customize_character_response_body()), "1800000000");
        assert_eq!(
            delete_character_response_body(),
            crate::investment_504_response_body(),
            "502/505/506 share 504's response shape - if 504's body ever changes, so must these"
        );
    }

    #[test]
    fn the_soid_argument_is_raw_big_endian() {
        // `d1-oracle object --tag 0x808017ce --obj 01010000000000d1` -> `d100000000000101`
        assert_eq!(
            character_soid_argument(&unhex("d100000000000101")),
            Some(0xD100_0000_0000_0101)
        );
        assert_eq!(character_soid_argument(&unhex("00000000000000")), None);
    }

    /// The 100-bit response leaves four padding bits. This reads the 64-bit SOID
    /// independently of the production encoder, across each bit position and
    /// deterministic boundary vectors. No native game inputs are required.
    #[test]
    fn create_response_soid_bits_round_trip_at_boundaries() {
        let mut cases = vec![0u64, 1, u64::MAX, 0xD100_0000_0000_0101];
        for bit in 0..64 {
            cases.push(1u64 << bit);
            cases.push(!(1u64 << bit));
        }
        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        for _ in 0..512 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            cases.push(state);
        }
        for soid in cases {
            let body = create_character_response_body(soid);
            assert_eq!(body.len(), 13);
            assert_eq!(&body[..4], &[0x18, 0, 0, 0]);
            assert_eq!(body[4] >> 4, 0, "36-bit result prefix");
            assert_eq!(body[12] & 0x0f, 0, "four trailing pad bits");

            let mut decoded = 0u64;
            for pos in 36..100 {
                let bit = (body[pos / 8] >> (7 - pos % 8)) & 1;
                decoded = (decoded << 1) | u64::from(bit);
            }
            assert_eq!(decoded, soid, "SOID shifted or truncated: {soid:#018x}");
        }
    }

    /// A short 64-bit argument is not SOID zero. Extra bytes are outside the
    /// single-argument field and must not alter the first eight bytes.
    #[test]
    fn character_soid_argument_checks_all_lengths_and_bit_positions() {
        let mut cases = vec![0u64, 1, u64::MAX, 0xD100_0000_0000_0101];
        for bit in 0..64 {
            cases.push(1u64 << bit);
        }
        for soid in cases {
            let bytes = soid.to_be_bytes();
            for len in 0..8 {
                assert_eq!(character_soid_argument(&bytes[..len]), None);
            }
            assert_eq!(character_soid_argument(&bytes), Some(soid));
            let mut with_suffix = bytes.to_vec();
            with_suffix.extend_from_slice(&[0xAA, 0x55]);
            assert_eq!(character_soid_argument(&with_suffix), Some(soid));
        }
    }

    /// Presence bits are distinct from their biased i8 payloads. Flip one
    /// presence flag in a known native-encoder vector and verify that the
    /// remaining decoded fields, especially customization and name, survive.
    #[test]
    fn create_request_presence_bits_are_independent_of_values() {
        let payload = unhex(VECTORS[1].1);
        let reference = CreateCharacterRequest::decode(&payload).unwrap();
        assert!(reference.race_present && reference.gender_present && reference.class_present);
        for (position, field) in [(0usize, 0usize), (9, 1), (18, 2)] {
            let mut edited = payload.clone();
            edited[position / 8] ^= 1u8 << (7 - position % 8);
            let mut expected = reference.clone();
            match field {
                0 => expected.race_present = false,
                1 => expected.gender_present = false,
                2 => expected.class_present = false,
                _ => unreachable!(),
            }
            expected.raw = edited.clone();
            assert_eq!(CreateCharacterRequest::decode(&edited).unwrap(), expected);
        }
    }

    /// ⭐⭐ **The trigger.** An account with an EMPTY character list must encode a count of **0**,
    /// and an account with `[0]` must not be mistaken for it.
    ///
    /// This is the fix's control. `d1-responder` used to build the list as `&[chr]` unconditionally,
    /// so `D1_CHARACTER_SOID=0` produced *one character whose SOID is 0* - and the client, which
    /// reads `account + 0x08` as the count (`0xA110F0`, `cmp dword [rbx+8], 0; jle`), therefore
    /// never needed to ask us to create one.
    ///
    /// Verified against the client's own object codec, not by reading the bit-packer:
    /// ```text
    /// d1-oracle object --tag 0x80801803 --decode e880000000000000c1d100000000000201000000
    ///   -> +0x0000 0xd100000000000001 ; +0x0068 0xd100000000000201        (count field ABSENT/0)
    /// d1-oracle object --tag 0x80801803 --decode e880000000000000c40000000000000001d100000000000201000000
    ///   -> +0x0000 … ; +0x0008 0x0000000000000001 ; +0x0068 …            (count == 1)
    /// ```
    #[test]
    fn an_empty_character_list_encodes_count_zero() {
        let acct = 0xD100_0000_0000_0001u64;
        let prof = 0xD100_0000_0000_0201u64;

        let empty = crate::account_rsat(acct, &[], prof);
        let one_zero = crate::account_rsat(acct, &[0], prof);
        let one_real = crate::account_rsat(acct, &[0xD100_0000_0000_0101], prof);

        // The codec-verified payloads, byte for byte.
        assert_eq!(hex(&empty), "e880000000000000c1d100000000000201000000");
        assert_eq!(
            hex(&one_zero),
            "e880000000000000c40000000000000001d100000000000201000000"
        );
        assert_eq!(
            hex(&one_real),
            "e880000000000000c74400000000000405d100000000000201000000"
        );

        // NEGATIVE CONTROL - "an empty list" and "a list holding SOID 0" must never be the same
        // bytes. They differ by 16 bytes of payload and by the client's count field.
        assert_ne!(empty, one_zero, "a list of [0] is NOT an empty list");
        assert_eq!(empty.len(), 20);
        assert_eq!(one_zero.len(), 28, "one more SOID = 64 more bits = 8 more bytes");
        assert_eq!(one_real.len(), 28);
    }

    #[test]
    fn the_transcript_contains_the_raw_bytes() {
        let payload = unhex(VECTORS[1].1);
        let d = CreateCharacterRequest::decode(&payload).unwrap();
        let t = d.transcript(&payload);
        assert!(t.contains(&hex(&payload)), "the raw hex must be in the transcript");
        assert!(t.contains("Guardian"));
        assert!(t.contains("0x811c9dc5"));
        assert_eq!(d.name_str(), "Guardian");
    }
}
