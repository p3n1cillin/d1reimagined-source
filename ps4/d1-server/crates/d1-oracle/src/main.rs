// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! `d1-oracle` - run the client's own RSAT codecs, offline.
//!
//! ```text
//! d1-oracle control                                          # both positive controls, PASS/FAIL
//! d1-oracle vectors                                          # every pinned vector, reproduced
//! d1-oracle object --tag 0x80801803 --decode e88000…         # QueueZ server object
//! d1-oracle object --tag 0x80801951 --roster-key 0xD1…01 --soid 0xD1…0101
//! d1-oracle message --tag 0x80801845 --msgid 504 --obj 00…   # investment message body
//! ```
//!
//! ⭐ **Content tables, generated here since 2026-08-11** - no Python on the runtime path:
//!
//! ```text
//! d1-oracle oodle controls                                   # the client's own Oodle decoder
//! d1-oracle oodle entry --tag 0x80A64196 -o def.bin          # a content entry's bytes
//! d1-oracle item-index write                                 # capture-rig/item-content-index.tsv
//! d1-oracle art-arrangement write                            # capture-rig/item-art-arrangement.tsv
//! d1-oracle item-index diff --against <a python-written one> # the executed golden diff
//! ```
//!
//! Those four need the owned-media package dump (`dumps/patch-01.29-CUSA00219/packages`, or
//! `$D1_PKGDIR`). The tables themselves are committed, so a clone without the dump can still *read*
//! them; it just cannot regenerate or extend them.
//!
//! Exit status **2 means a positive control failed** and nothing printed may be cited.
//! ⚠ Picking `object` when you meant `message` (or the reverse) yields a wrong answer with no
//! error - QueueZ object ⇒ `object`; investment message ⇒ `message`.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use d1_oracle::control::{self, hex, hexs};
use d1_oracle::object::pack_bits;
use d1_oracle::{MessageOracle, ObjectOracle, ACCOUNT_SOID, CHARACTER_SOID, PROFILE_SOID};

fn parse_int(s: &str) -> std::result::Result<u64, String> {
    let t = s.trim();
    let r = if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        u64::from_str_radix(h, 16)
    } else {
        t.parse::<u64>()
    };
    r.map_err(|e| format!("{t:?}: {e}"))
}

#[derive(Parser)]
#[command(name = "d1-oracle", about = "Run the Destiny 1 client's own RSAT codecs under Unicorn")]
struct Cli {
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// The positive controls. Exit 2 on failure.
    Control,
    /// Rebuild and re-verify every pinned payload.
    Vectors,
    /// The OBJECT codec (0x2C9000 / 0x2C8FF0) - QueueZ server objects.
    Object {
        #[arg(long, value_parser = parse_int)]
        tag: u64,
        /// Hex payload to decode.
        #[arg(long)]
        decode: Option<String>,
        /// Hex bytes for the decoded C object (encode direction).
        #[arg(long)]
        obj: Option<String>,
        /// A type a kind-0x22 typed value in the object names (repeatable), e.g. 0x8080279a.
        #[arg(long, value_parser = parse_int)]
        value_type: Vec<u64>,
        /// Convenience: roster key at +0x00, count at +0x08, SOIDs from +0x10.
        #[arg(long, value_parser = parse_int)]
        roster_key: Option<u64>,
        #[arg(long, value_parser = parse_int)]
        soid: Vec<u64>,
        #[arg(long, value_parser = parse_int, default_value = "36864")]
        capacity: u64,
    },
    /// The family-4/type-1 character PROFILE (0x80801743) - build one that is not empty.
    #[command(subcommand)]
    Profile(ProfileCmd),
    /// The family-3/type-1 character-SELECT CARD (0x80801954) - level, Light and the emblem slot.
    #[command(subcommand)]
    Character(CharacterCmd),
    /// ⭐ The family-2 ORBIT ROSTER CARD (0x808018B7 + 0x80801810) - what the orbit nameplate reads.
    #[command(subcommand)]
    Roster(RosterCmd),
    /// ⭐⭐ The family-4/type-3 INVENTORY ELEMENT (0x80801732) - the item the client can equip.
    #[command(subcommand)]
    Inventory(InventoryCmd),
    /// ⭐ The ACTIVITY-HOST payload codec (0x2A4090 / 0x2A4140) - BAP 8/100 `user_defined_type`
    /// bodies. `layout` derives every schema tag; `controls` is the gate on citing any of it.
    #[command(subcommand)]
    ActivityHost(AhCmd),
    /// ⭐⭐ The udt 5 `sensor_auth_update` payload - the wire message that writes the activity
    /// lifetime phase (`record+0x1AC`) the player-spawn gate reads. See [`d1_oracle::sensor`].
    #[command(subcommand)]
    Sensor(SensorCmd),
    /// ⭐ CONTENT BYTES - the client's own Oodle decoder under Unicorn, plus the `.pkg` entry walk.
    /// This is the Rust replacement for `capture-rig/oodle/oodle-decompress.py` and
    /// `extract-entry.py`; nothing here implements a compression algorithm.
    #[command(subcommand)]
    Oodle(OodleCmd),
    /// ⭐⭐ THE ITEM CONTENT INDEX - generate `capture-rig/item-content-index.tsv` from the
    /// packages, in process. Replaces `capture-rig/oodle/item-index.py --write`.
    #[command(subcommand)]
    ItemIndex(ItemIndexCmd),
    /// ⭐ THE ART-ARRANGEMENT TABLE - generate `capture-rig/item-art-arrangement.tsv` from the
    /// packages. Replaces `capture-rig/oodle/art-arrangement.py --staged --write`.
    #[command(subcommand)]
    ArtArrangement(ArtCmd),
    /// ⭐ THE ACTIVITY MODE TABLE - generate `capture-rig/activity-modes.tsv` from block 1063.
    /// The data behind d1-ghost's destination selector (`design/d1-ghost.md` §4.1).
    #[command(subcommand)]
    ActivityModes(ActivityModesCmd),
    /// The MESSAGE codec (0x261BE0 / 0x261FB0) - investment message bodies.
    Message {
        #[arg(long, value_parser = parse_int)]
        tag: u64,
        #[arg(long, value_parser = parse_int, default_value = "503")]
        msgid: u64,
        #[arg(long, value_parser = parse_int, default_value = "0x80801726")]
        request_tag: u64,
        #[arg(long)]
        obj: Option<String>,
        #[arg(long)]
        span: Option<String>,
    },
}

/// `d1-oracle oodle …` - content bytes. See [`d1_oracle::oodle`].
#[derive(Subcommand)]
enum OodleCmd {
    /// The decoder's controls, one of them INVERTED. Exit 2 on failure.
    Controls,
    /// Decompress one block and write it (or just report it).
    Block {
        #[arg(long)]
        pkg: String,
        #[arg(long, value_parser = parse_int)]
        block: u64,
        #[arg(long, short)]
        out: Option<PathBuf>,
    },
    /// The decompressed payload of one entry, spanning blocks as needed. Give `--tag`, or
    /// `--pkg` with `--index`.
    Entry {
        #[arg(long, value_parser = parse_int)]
        tag: Option<u64>,
        #[arg(long)]
        pkg: Option<String>,
        #[arg(long, value_parser = parse_int)]
        index: Option<u64>,
        #[arg(long, short)]
        out: Option<PathBuf>,
        /// Bytes of head hex to print.
        #[arg(long, default_value_t = 64)]
        hex: usize,
    },
}

/// `d1-oracle item-index …` - see [`d1_oracle::item_index`].
#[derive(Subcommand)]
enum ItemIndexCmd {
    /// The shipped table's controls (pins + the two NEGATIVE checks). Exit 2 on failure.
    Controls,
    /// ⭐ Read the table out of the packages and write the TSV. **Exit 2** if the result would not
    /// load through [`d1_oracle::item_index::IndexTable`], so a bad table is never left on disk.
    Write {
        /// Default: the selected build's path (`item_index::source().path`).
        #[arg(long, short)]
        out: Option<PathBuf>,
        /// Render and report without touching the file.
        #[arg(long)]
        dry_run: bool,
    },
    /// ⭐ THE GOLDEN DIFF, executed. Render the table from the packages and compare it, byte for
    /// byte, with a file the Python wrote. Exit 2 on any difference outside the `# GENERATED by`
    /// line, which differs by design.
    Diff {
        #[arg(long, default_value = d1_oracle::item_index::DEFAULT_PATH)]
        against: PathBuf,
    },
}

/// `d1-oracle activity-modes …` - see [`d1_oracle::activity_modes`].
#[derive(Subcommand)]
enum ActivityModesCmd {
    /// The table's controls (21 launchable, band 393-413, 407 Elimination, one INVERTED). Needs
    /// the owned-media dump. Exit 2 on failure.
    Controls,
    /// ⭐ Read the table out of block 1063 and write the TSV. **Exit 2** if the rendered table does
    /// not re-parse, so a bad table is never left on disk.
    Write {
        #[arg(long, short, default_value = d1_oracle::activity_modes::DEFAULT_PATH)]
        out: PathBuf,
        /// Render and report without touching the file.
        #[arg(long)]
        dry_run: bool,
    },
}

/// `d1-oracle art-arrangement …` - see [`d1_oracle::art_arrangement`].
#[derive(Subcommand)]
enum ArtCmd {
    /// The walk's controls, two of them INVERTED. Exit 2 on failure.
    Controls,
    /// Resolve one or more content indices and print everything their definitions say.
    Show {
        #[arg(long = "index", value_parser = parse_int)]
        indices: Vec<u64>,
    },
    /// ⭐ Resolve every content index in the staged inventory (plus the emblem) and write the TSV.
    /// **Exit 2** if the result would not load through [`d1_oracle::item_index::ArtTable`].
    Write {
        /// Default: the selected build's art path (`item_index::source().art_path`).
        #[arg(long, short)]
        out: Option<PathBuf>,
        #[arg(long, default_value = "capture-rig/inventory")]
        inventory: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// ⭐ THE GOLDEN DIFF, executed, against a file the Python wrote. Exit 2 on any difference
    /// outside the `# GENERATED by` line.
    Diff {
        #[arg(long, default_value = d1_oracle::art_arrangement::DEFAULT_PATH)]
        against: PathBuf,
        #[arg(long, default_value = "capture-rig/inventory")]
        inventory: PathBuf,
    },
}

/// `d1-oracle activity-host …` - see [`d1_oracle::activity_host`] for the derivation and controls.
#[derive(Subcommand)]
enum AhCmd {
    /// Every Activity-Host message's schema tag, derived from the eboot. Nothing is transcribed.
    Layout,
    /// Every control, including the negatives. **Exit 2 on failure.**
    Controls,
    /// ⭐⭐ The LEAK control: encode the whole hot-path message set in a loop and require the bump
    /// arena's high-water mark to be **flat**. Runs its own inversion. **Exit 2 on failure.**
    ///
    /// The axis, in words: *this proves the encoder can run unbounded without arena growth.*
    Leak {
        /// Full passes over the five hot-path udts. At the 5,125 ms heartbeat, 2000 ≈ 2.8 hours.
        #[arg(long, default_value_t = 2000)]
        cycles: usize,
    },
    /// Per-udt bump-arena accounting after one pass - scaffolding, used, high water, guest allocs.
    Arena {
        #[arg(long, default_value_t = 1)]
        cycles: usize,
    },
    /// Decode a BAP 8/100 payload for a `user_defined_type`.
    Decode {
        #[arg(long, value_parser = parse_int)]
        udt: u64,
        /// Hex payload. Defaults to the captured live join_request when `--udt 3`.
        #[arg(long)]
        wire: Option<String>,
    },
    /// Encode a struct image for a `user_defined_type`. The image must be exactly the struct size.
    Encode {
        #[arg(long, value_parser = parse_int)]
        udt: u64,
        /// Hex struct image. Omitted = all zeroes of the right size.
        #[arg(long)]
        obj: Option<String>,
    },
}

/// `d1-oracle sensor …` - see [`d1_oracle::sensor`] for the frame grammar and the executed proof.
#[derive(Subcommand)]
enum SensorCmd {
    /// Synthetic controls for the 56578 objective-list handover (no game input needed).
    ObjectiveControls,
    /// Build a 56578 visible objective handover in a sensor list; does not publish it.
    Objective {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, value_parser = parse_int)]
        group: u64,
        #[arg(long)]
        active: u16,
        #[arg(long, required = true)]
        clear: Vec<u16>,
        #[arg(long)]
        marker: u16,
        #[arg(long)]
        machine: u8,
    },
    /// Every control, including the negatives and the byte-exact reproduction of the audited
    /// payloads through the client's own codec. **Exit 2 on failure.**
    Controls,
    /// Build one `sensor_auth_update` payload and print its hex. All name fields are inputs - the
    /// per-load hash is NOT derived here.
    Build {
        /// Replication generation; must equal the client's `world+0x1D`.
        #[arg(long, default_value_t = 0)]
        gen: u8,
        /// FNV-1 namespace-group hash. Per-load. Default = the in-world pvp_moonbase group 0.
        #[arg(long, value_parser = parse_int, default_value = "0xC2DACAF7")]
        name_hash: u64,
        /// The name struct's `+4` byte. Must not be -1.
        #[arg(long, value_parser = parse_int, default_value = "0x10")]
        name_i8: u64,
        /// The record index within the group.
        #[arg(long, default_value_t = 3)]
        name_sub: i16,
        /// The lifetime phase to write: 3 or 6 pass the spawn gate; -1 is NONE.
        #[arg(long, default_value_t = 3)]
        phase: i8,
        /// Also set the two `FUN_0010CB60` spawn-precondition fields `record+0x674`/`+0x678`.
        #[arg(long)]
        tail_id: Option<String>,
        #[arg(long)]
        tail_hash: Option<String>,
    },
}

/// `d1-oracle profile …` - see [`d1_oracle::profile`] for why each default is what it is.
#[derive(Subcommand)]
enum ProfileCmd {
    /// The offset table, re-derived from the eboot's descriptor tree. Nothing is transcribed.
    Layout,
    /// Every profile control. **Exit 2 on failure** - including the NEGATIVE one.
    Controls {
        /// The empty fixture this replaces; its two-non-zero-fields decode is a control.
        #[arg(long, default_value = "capture-rig/profiles/profile-old-russia.bin")]
        baseline: String,
    },
    /// Build the profile and write the wire payload. Encoded by the CLIENT'S OWN encoder.
    Build {
        #[arg(long, default_value = "capture-rig/profiles/profile-populated.bin")]
        out: String,
        #[arg(long, value_parser = parse_int, default_value_t = d1_oracle::profile::DEFAULT_SOID)]
        soid: u64,
        /// `{race, gender, class}`. Default `0,0,1` - the triple every `capture-rig/characters/`
        /// fixture carries. It must agree with the family-3 character object.
        #[arg(long, default_value = "0,0,1")]
        identity: String,
        /// `last_orbited_destination`. ⚠ Orbit depends on this; `0` reads as "unset".
        #[arg(long, value_parser = parse_int,
              default_value_t = u64::from(d1_oracle::profile::DESTINATION_OLD_RUSSIA))]
        destination: u64,
        /// Light, via the `0x265EF0` override - no gear required. `0` omits the override entirely.
        #[arg(long, default_value_t = d1_oracle::profile::LIGHT_ROI_CAP)]
        light: u32,
        /// The Light stat index. `[O]` = 0, from a snapshot replay with a passing positive control.
        #[arg(long, default_value_t = d1_oracle::profile::LIGHT_STAT_INDEX)]
        light_index: u8,
        /// Extra `INDEX:VALUE` stat overrides, after Light. Max 7 entries in total.
        #[arg(long = "stat", value_name = "INDEX:VALUE")]
        stats: Vec<String>,
        /// Directory (or file) of staged family-4/3 elements; their SOIDs become the equipped list.
        /// ⚠ The same path must be served as `D1_INVENTORY` or the SOIDs name nothing.
        #[arg(long, default_value = "capture-rig/inventory")]
        inventory: String,
        /// Equip these item-instance SOIDs instead of reading `--inventory`. Repeatable.
        #[arg(long = "equip", value_parser = parse_int)]
        equip: Vec<u64>,
        /// Equip nothing.
        #[arg(long)]
        no_equip: bool,
        /// `INDEX:PROGRESS` progression entries. ⛔ `character_level`'s `u16` ordinal is `[U]`.
        #[arg(long = "progression", value_name = "INDEX:PROGRESS")]
        progressions: Vec<String>,
        /// Leave the 340 inventory slots ZERO - i.e. claim 340 copies of content index 0, which is
        /// what every profile before 2026-08-08 said. Only for bisecting against the old fixture.
        #[arg(long)]
        phantom_item_slots: bool,
        /// Ship the 340-slot array all-`0xFFFF` - i.e. name NO items at all, the pre-2026-08-09
        /// profile. Only for bisecting; the populated array is the whole point.
        #[arg(long)]
        no_content_indices: bool,
        /// ⭐ Set the client's own all-content override bools at `+0x6E4C`/`+0x6E4D`: every
        /// investment requirement reports SATISFIED. Bypasses rung 33's `missing_dlc` and
        /// `missing_permissions` gates and unfilters the requirement iterators.
        /// ⛔ Does NOT grant offer-key ownership - see `historicaldocs/design/entitlement-and-content-rights.md`.
        /// ⚠ It suppresses the two context strings rung 33 would otherwise print, so a run with it
        /// on cannot tell you whether content gating was the blocker. Bisect with it off.
        #[arg(long)]
        all_content: bool,
        /// ⭐ Field `[11]` completion subindices to set to 2 (unlock-flag storage kind 3),
        /// comma-separated. Repeatable.
        #[arg(long = "complete", value_delimiter = ',')]
        complete: Vec<u16>,
        /// Unlock-flag INDICES to serve as completions, comma-separated: each is looked up in
        /// `--flag-table` and must be storage kind 3, whose `sub` is then set. A non-kind-3 index is
        /// an error, not a skip - it has no home in this field.
        #[arg(long = "complete-flags", value_delimiter = ',')]
        complete_flags: Vec<u16>,
        /// The master unlock table as TSV (`index hash kind sub`), e.g. from `d1-flagtable.py`.
        #[arg(long, default_value = "capture-rig/builds/later-56578/unlock-flags.tsv")]
        flag_table: String,
        /// ⭐ A pending world-loot record `CONTENT_INDEX:SOURCE_KIND:KEY:NONCE` (repeatable), e.g.
        /// the starter Khvostov `3598:3:1:0x...` that the Breach loot drop (incident 1504) names.
        #[arg(long)]
        pending_loot: Vec<String>,
        /// Print the payload and the decode; write nothing.
        #[arg(long)]
        dry_run: bool,
    },
}

/// `d1-oracle character …` - see [`d1_oracle::character`] for why each default is what it is.
#[derive(Subcommand)]
enum CharacterCmd {
    /// The offset table, re-derived from the eboot's descriptor tree. Nothing is transcribed.
    Layout,
    /// Every card control. **Exit 2 on failure** - including the two NEGATIVE ones.
    Controls {
        /// The identity-only fixture this replaces; its empty equipment/stat regions are a control.
        #[arg(long, default_value = "capture-rig/characters/char-lvl40-probe.bin")]
        baseline: String,
    },
    /// Build the card and write the wire payload. Encoded by the CLIENT'S OWN encoder.
    Build {
        #[arg(long, default_value = "capture-rig/characters/char-lvl40-geared.bin")]
        out: String,
        /// ⚠ Must equal the SOID `d1-responder` puts in the QueueZ object header.
        #[arg(long, value_parser = parse_int, default_value_t = d1_oracle::character::DEFAULT_SOID)]
        soid: u64,
        /// `{race, gender, class}`. ⚠ Must agree with `d1-oracle profile build --identity`.
        #[arg(long, default_value = "0,0,1")]
        identity: String,
        /// ⭐ `C+0x68` - the level the card shows. The server states it; nothing derives it.
        #[arg(long, default_value_t = d1_oracle::character::LEVEL_CAP)]
        level: u32,
        /// `C+0x70`, floored and compared `setg` against `--level`. Defaults to `--level`.
        #[arg(long)]
        derived_level: Option<f32>,
        /// ⭐ Light, written as stat-table entry 0. `0` omits it.
        #[arg(long, default_value_t = d1_oracle::profile::LIGHT_ROI_CAP)]
        light: u32,
        /// The Light stat index. `[C]`: 0, from a replay of `0x281F50` and from the stat table.
        #[arg(long, default_value_t = d1_oracle::character::LIGHT_STAT_INDEX)]
        light_index: u8,
        /// Extra `INDEX:VALUE` stat entries after Light. Max 16 in total. `Defense` = 1, `Attack` = 11.
        #[arg(long = "stat", value_name = "INDEX:VALUE")]
        stats: Vec<String>,
        /// Directory (or file) of staged family-4/3 elements; their SOIDs fill the equipment
        /// summary's element `+0x00` and their **measured content indices** element `+0x08`, in
        /// filename order, skipping the reserved emblem slot 12.
        #[arg(long, default_value = "capture-rig/inventory")]
        inventory: String,
        /// Leave all 20 equipment slots empty.
        #[arg(long)]
        no_equip: bool,
        /// ⭐ `SLOT:INDEX` - the emblem. Defaults to the **measured** slot 12
        /// (`investment_globals->vtbl[0x160]()[0x18]`, replayed = `0x0C`) and content index 1512
        /// ("The Reflective Proof", `0x5508AC2A`, from `capture-rig/item-content-index.tsv`).
        #[arg(long, value_name = "SLOT:INDEX")]
        emblem: Option<String>,
        /// Ship `0xFFFF` in every equipment slot and no emblem - the pre-2026-08-09 card, for
        /// bisecting. Do not combine with another change in one run.
        #[arg(long)]
        no_content_indices: bool,
        /// Also sentinel-fill (`0xFF`) the three `[U]` sibling stat tables at +0x8F0/+0x970/+0x9F0.
        #[arg(long)]
        sentinel_all_stat_tables: bool,
        /// Leave unused stat keys ZERO - i.e. claim 16 copies of stat index 0, which is what every
        /// card before today said. Only for bisecting; `0x266740` then overwrites Light with 0.
        #[arg(long)]
        zero_stat_keys: bool,
        /// Print the payload and the decode; write nothing.
        #[arg(long)]
        dry_run: bool,
    },
    /// ⭐⭐ Build the **family-0/type-1 PEER VIEW** (`0x808017FD`) - the object
    /// `c_cui_investment_character_state_component::vfunc[14]` (`0x9A3C30`) actually reads for the
    /// character screen's level and Light. It carries the SAME 2,568-byte block as the card, at
    /// `+0x30` instead of `+0x68`. `0xA110F0` is the only function in the binary that reads
    /// family 3, and it builds the *select tiles* - so a card alone can never fill this screen.
    ///
    /// Writes `f0t0-<contact>.bin` + `f0t1-<card>.bin` into `--out-dir`, the naming
    /// `D1_ROSTER_DIR` already consumes.
    BuildPeer {
        /// Directory for the fixtures. Point `D1_ROSTER_DIR` at it.
        #[arg(long, default_value = "capture-rig/rosters-f0")]
        out_dir: String,
        /// ⭐ Family 0/0 `+0x00`. The peer-subscription key; the account SOID, by analogy with
        /// family 2's `roster_store+0x28`. `[I]` - the discriminator is a run.
        #[arg(long, value_parser = parse_int, default_value_t = d1_oracle::ACCOUNT_SOID)]
        contact_key: u64,
        /// ⭐ Family 0/0 `+0x08` → family 0/1 `+0x00`. `roster.rs`'s `PEER_TAG` note records that
        /// `0xA3DB55` looks family 0/1 up with **the same key as family 2/1**, so this defaults to
        /// the shipped `f2t1-` fixture's key.
        #[arg(long, value_parser = parse_int, default_value_t = 0xD100_0000_0000_0501u64)]
        card_key: u64,
        #[arg(long, default_value = "D1Guardian")]
        name: String,
        #[arg(long, default_value = "")]
        clan: String,
        #[arg(long, default_value = "0,0,1")]
        identity: String,
        #[arg(long, default_value_t = d1_oracle::character::LEVEL_CAP)]
        level: u32,
        #[arg(long, default_value_t = d1_oracle::profile::LIGHT_ROI_CAP)]
        light: u32,
        #[arg(long, default_value_t = d1_oracle::character::LIGHT_STAT_INDEX)]
        light_index: u8,
        /// Extra `INDEX:VALUE` stat entries after Light. `Defense` = 1, `Attack` = 11.
        #[arg(long = "stat", value_name = "INDEX:VALUE")]
        stats: Vec<String>,
        #[arg(long, default_value = "capture-rig/inventory")]
        inventory: String,
        #[arg(long)]
        no_equip: bool,
        /// ⭐ Leave the emblem slot (12) at `0xFFFF` too, so **all twenty** equipment content
        /// indices are the empty sentinel. The world-tag loader then reaches state 5 without any
        /// family-0/3 item lookup - the hybrid fixture for the biped-spawn experiment
        /// (`historicaldocs/design/player-in-world-membership-fix.md` §5).
        #[arg(long)]
        no_emblem: bool,
        /// Also write the family-2 pair into the same directory, so one `D1_ROSTER_DIR` serves both.
        #[arg(long, default_value_t = true)]
        with_family2: bool,
        #[arg(long)]
        dry_run: bool,
    },
}

/// `d1-oracle inventory …` - see [`d1_oracle::inventory`] for why each default is what it is.
#[derive(Subcommand)]
enum InventoryCmd {
    /// The offset table, re-derived from the eboot's descriptor tree. Nothing is transcribed.
    Layout,
    /// Every element control. **Exit 2 on failure** - including the four NEGATIVE ones.
    Controls,
    /// ⭐⭐ Re-encode every staged element with the fields the client actually reads.
    ///
    /// Reads each `NN-<bucket>.bin`, takes its SOID and the item hash it carries at `+0x18`,
    /// resolves the **real** content index for `+0x24`, and rewrites the element with a sane level,
    /// a **zero** equip minimum, and `0xFFFF` in every index field that means "none".
    Rebuild {
        #[arg(long, default_value = "capture-rig/inventory")]
        dir: String,
        /// Write here instead of in place.
        #[arg(long)]
        out_dir: Option<String>,
        /// ⭐ `+0x18`, the item level.
        #[arg(long, default_value_t = d1_oracle::inventory::LEVEL_CAP)]
        level: i32,
        /// ⭐ `+0x1C`, the equip minimum. **`0` is what clears the padlock** - the test is
        /// `character_stat >= minimum`.
        #[arg(long, default_value_t = 0)]
        equip_min: u32,
        /// ⛔ Reproduce the pre-2026-08-10 element instead (hash at `+0x18`, bucketTypeHash at
        /// `+0x1C`, content index `1`). Only for bisecting; `to_object` refuses it, so this exists
        /// to show you the refusal.
        #[arg(long)]
        legacy: bool,
        #[arg(long)]
        dry_run: bool,
        /// The item-index TSV the staged elements were BUILT against, when that is another build's
        /// (e.g. `capture-rig/item-content-index.tsv` = 01.29 when rebuilding for 56578). Each
        /// element's stored content index is turned back into its item hash through THAT table,
        /// then re-resolved in the selected build's. Without it a moved index names the wrong
        /// item (13 Oct: 01.29 ci 6299 Gjallarhorn came back as 56578 ci 6299 = 0x7EAF157D).
        #[arg(long)]
        from_index: Option<String>,
    },
}

/// `d1-oracle roster …` - see [`d1_oracle::roster`] for why each default is what it is.
#[derive(Subcommand)]
enum RosterCmd {
    /// Both family-2 offset tables plus the three instance checksums, re-derived from the eboot.
    Layout,
    /// Every roster control. **Exit 2 on failure** - including the four NEGATIVE ones.
    Controls,
    /// Build the contact + card and write both payloads. Encoded by the CLIENT'S OWN encoder.
    Build {
        /// Directory for the two fixtures, named `f<family>t<type>-<key>.bin`.
        #[arg(long, default_value = "capture-rig/rosters")]
        out: String,
        /// ⭐ Must equal the roster store's contact key (`roster_store+0x28`), i.e. the account SOID.
        #[arg(long, value_parser = parse_int,
              default_value_t = d1_oracle::roster::DEFAULT_CONTACT_KEY)]
        contact_key: u64,
        /// ⭐ The 2/0 `+0x08` → 2/1 `+0x00` link. Any stable non-zero value resolves the two-hop.
        #[arg(long, value_parser = parse_int,
              default_value_t = d1_oracle::roster::DEFAULT_CARD_KEY)]
        card_key: u64,
        /// Up to 26 UTF-16 characters. Longer is refused, never truncated.
        #[arg(long, default_value = "D1Guardian")]
        name: String,
        /// Up to 4 UTF-16 characters.
        #[arg(long, default_value = "")]
        clan: String,
        /// ⭐ `+0x10`. `0xA2DE67` → `nameplate+0x188`. 40 also lights the at-cap marker.
        #[arg(long, default_value_t = d1_oracle::roster::LEVEL_CAP)]
        level: u32,
        /// ⭐ `+0x14`, RAW. `0x281FA0` scales it before display, so the number shown may differ.
        #[arg(long, default_value_t = d1_oracle::profile::LIGHT_ROI_CAP)]
        light: u32,
        /// `+0x18` - the XP bar fraction.
        #[arg(long, default_value_t = 0.0)]
        xp: f32,
        /// ⛔ `+0x20`, the emblem content index. **`[U]` on this build** - no item content index has
        /// ever been measured, and a fabricated one renders as something and looks like success.
        /// The default `0xFFFF` keeps the emblem BLACK on purpose.
        #[arg(long, value_parser = parse_int,
              default_value_t = u64::from(d1_oracle::roster::EMPTY_CONTENT_INDEX))]
        emblem: u64,
        /// Print the payloads and the decodes; write nothing.
        #[arg(long)]
        dry_run: bool,
    },
}

fn run_sensor(_cli: &Cli, cmd: &SensorCmd) -> anyhow::Result<ExitCode> {
    use d1_oracle::sensor::{self, SensorPhaseUpdate};

    match cmd {
        SensorCmd::ObjectiveControls => {
            if let Err(error) = d1_oracle::sensor_objective::controls() {
                eprintln!("FAIL objective controls: {error:#}");
                return Ok(ExitCode::from(2));
            }
            println!("PASS synthetic objective handover and negative controls; no native/codec claim");
            Ok(ExitCode::SUCCESS)
        }
        SensorCmd::Objective { input, out, group, active, clear, marker, machine } => {
            anyhow::ensure!(
                d1_build::selected().id == "later-56578",
                "objective layout is audited only for later-56578; set D1_BUILD and D1_BUILDS explicitly"
            );
            d1_oracle::sensor_objective::controls()?;
            let text = std::fs::read_to_string(input)?;
            let text = d1_oracle::sensor_objective::handover(
                &text, u32::try_from(*group)?, *active, clear, *marker, *machine,
            )?;
            anyhow::ensure!(input != out, "use a separate output list");
            std::fs::write(out, text)?;
            println!("wrote {}: visible objective {}, cleared {:?}; publish separately and verify native readback", out.display(), active, clear);
            Ok(ExitCode::SUCCESS)
        }
        SensorCmd::Controls => {
            let cs = sensor::run();
            let mut ok = true;
            for c in &cs {
                ok &= c.passed;
                println!("{}  {:<72} {}", if c.passed { "PASS" } else { "FAIL" }, c.name, c.detail);
            }
            println!(
                "\n{}",
                if ok {
                    "ALL SENSOR (udt 5) CONTROLS PASS"
                } else {
                    "*** A CONTROL FAILED - the udt-5 payload may not be cited ***"
                }
            );
            Ok(if ok { ExitCode::SUCCESS } else { ExitCode::from(2) })
        }
        SensorCmd::Build { gen, name_hash, name_i8, name_sub, phase, tail_id, tail_hash } => {
            let tail = match (tail_id, tail_hash) {
                (Some(i), Some(h)) => Some((
                    parse_int(i).map_err(anyhow::Error::msg)? as u32,
                    parse_int(h).map_err(anyhow::Error::msg)? as u32,
                )),
                (None, None) => None,
                _ => anyhow::bail!("--tail-id and --tail-hash must be given together"),
            };
            let m = SensorPhaseUpdate {
                gen: *gen,
                name_hash: *name_hash as u32,
                name_i8: *name_i8 as u8,
                name_sub: *name_sub,
                phase: *phase,
                tail,
                clients: Vec::new(),
                authority: None,
            };
            let payload = m.encode()?;
            println!(
                "udt 5 sensor_auth_update  {} bytes  phase {}  name {{hash {:#010x}, i8 {:#x}, sub {}}}",
                payload.len(),
                m.phase,
                m.name_hash,
                m.name_i8,
                m.name_sub
            );
            println!("{}", hexs(&payload));
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn run_roster(cli: &Cli, cmd: &RosterCmd) -> anyhow::Result<ExitCode> {
    use d1_eboot::descriptors;
    use d1_eboot::image::Image;
    use d1_oracle::roster::{self, RosterSpec};

    match cmd {
        RosterCmd::Layout => {
            let image = Image::open_default()?;
            let idx = descriptors::index(&image)?;
            let l = roster::derive_layout(&image, &idx)?;
            if cli.json {
                println!("{}", serde_json::json!({
                    "contact": {"tag": roster::CONTACT_TAG, "size": l.contact_size,
                                "instance_type": l.contact_instance_type,
                                "key": l.contact_key, "card_key": l.contact_card_key,
                                "name": {"base": l.name.base, "count": l.name.count,
                                         "elem": l.name.elem_size, "tag": l.name.tag},
                                "clan": {"base": l.clan.base, "count": l.clan.count,
                                         "elem": l.clan.elem_size, "tag": l.clan.tag}},
                    "card": {"tag": roster::CARD_TAG, "size": l.card_size,
                             "instance_type": l.card_instance_type,
                             "key": l.card_key, "level": l.card_level, "light": l.card_light,
                             "xp": l.card_xp, "emblem": l.card_emblem, "colour": l.card_colour},
                    "peer": {"tag": roster::PEER_TAG, "size": l.peer_size,
                             "instance_type": l.peer_instance_type},
                }));
            } else {
                println!("family 2 / type 0  {:#010x}  struct {} bytes   instance_type {:#010x}",
                         roster::CONTACT_TAG, l.contact_size, l.contact_instance_type);
                println!("  +{:#05x}  ⭐ contact key  - must equal roster_store+0x28 (0x57F6F0 hop 1)",
                         l.contact_key);
                println!("  +{:#05x}  ⭐ card key     - the ONLY producer of container+0x08 (0xA3F0C1)",
                         l.contact_card_key);
                println!("  +{:#05x}  display name  {} x {} B   {:#010x}",
                         l.name.base, l.name.count, l.name.elem_size, l.name.tag);
                println!("  +{:#05x}  clan tag      {} x {} B   {:#010x}",
                         l.clan.base, l.clan.count, l.clan.elem_size, l.clan.tag);
                println!("\nfamily 2 / type 1  {:#010x}  struct {} bytes   instance_type {:#010x}",
                         roster::CARD_TAG, l.card_size, l.card_instance_type);
                println!("  +{:#05x}  ⭐ key         - hop 2 scans this; == contact +{:#x}",
                         l.card_key, l.contact_card_key);
                println!("  +{:#05x}  ⭐ LEVEL       0xA2DE67 -> nameplate+0x188", l.card_level);
                println!("  +{:#05x}  ⭐ LIGHT raw   0xA2DE56 -> 0x281FA0 -> nameplate+0x180", l.card_light);
                println!("  +{:#05x}  XP fraction   0xA3DD72 / 0xA2DEC6 -> nameplate+0x190", l.card_xp);
                println!("  +{:#05x}  [U] u32       0xA3DD82 -> widget+0x1B0", l.card_u32);
                println!("  +{:#05x}  ⭐ EMBLEM u16  0xA2DF72 -> 0xA2DBC0   (0xFFFF = none)", l.card_emblem);
                println!("  +{:#05x}  packed colour bitfield - left ZERO, see the module docs", l.card_colour);
                println!("\nfamily 0 / type 1  {:#010x}  struct {} (={:#x}) bytes   instance_type {:#010x}",
                         roster::PEER_TAG, l.peer_size, l.peer_size, l.peer_instance_type);
                println!("  the RICH peer view. NOT built here - 0xA3DD16 blanks on family 2, and");
                println!("  family 0 only chooses which fill runs (0xA2D9F0 vs 0xA2DE30).");
            }
            Ok(ExitCode::SUCCESS)
        }

        RosterCmd::Controls => {
            let cs = roster::controls()?;
            let mut ok = true;
            for c in &cs {
                ok &= c.pass;
                println!("{}  {:<28} {}", if c.pass { "PASS" } else { "FAIL" }, c.name, c.detail);
            }
            println!("\n{}", if ok {
                "ALL ROSTER CONTROLS PASS"
            } else {
                "*** A CONTROL FAILED - nothing this tool produced may be cited ***"
            });
            Ok(if ok { ExitCode::SUCCESS } else { ExitCode::from(2) })
        }

        RosterCmd::Build { out, contact_key, card_key, name, clan, level, light, xp, emblem, dry_run } => {
            if *emblem > u64::from(u16::MAX) {
                anyhow::bail!("--emblem {emblem} does not fit the u16 at +0x20");
            }
            let emblem = *emblem as u16;
            if emblem != roster::EMPTY_CONTENT_INDEX {
                eprintln!(
                    "⚠ --emblem {emblem:#06x} - item content indices are [U] on this build. A \
                     fabricated index renders as SOMETHING and looks like success."
                );
            }
            let spec = RosterSpec {
                contact_key: *contact_key,
                card_key: *card_key,
                display_name: name.clone(),
                clan_tag: clan.clone(),
                level: *level,
                light: *light,
                xp_fraction: *xp,
                emblem,
                ..RosterSpec::default()
            };
            let b = roster::build(&spec)?;

            if cli.json {
                println!("{}", serde_json::json!({
                    "objects": b.objects().iter().map(|o| serde_json::json!({
                        "file": o.file_name(), "tag": o.tag, "family": o.family,
                        "type": o.obj_type, "instance_type": o.instance_type,
                        "key": format!("{:#018x}", o.key), "bytes": o.payload.len(),
                        "bits": o.bits, "hex": hexs(&o.payload),
                        "nonzero": o.nonzero().iter()
                            .map(|(k, v)| (format!("{k:#x}"), format!("{v:#x}")))
                            .collect::<Vec<_>>(),
                    })).collect::<Vec<_>>(),
                }));
            } else {
                for o in b.objects() {
                    println!("{:#010x}  family {}/type {}  instance {:#010x}  key {:#018x}",
                             o.tag, o.family, o.obj_type, o.instance_type, o.key);
                    println!("  {} B / {} bits  round trip: object IDENTICAL", o.payload.len(), o.bits);
                    println!("  {}", hexs(&o.payload));
                    for (off, v) in o.nonzero() {
                        println!("    +{off:#05x}  {v:#018x}");
                    }
                }
                println!(
                    "\n  two-hop: 2/0 +{:#x} == 2/1 +{:#x} == {:#018x}   ⇒ container+0x10 should \
                     read 0x02, not 0xFF",
                    roster::OFF_CONTACT_CARD_KEY, roster::OFF_CARD_KEY, b.card.key);
                if emblem == roster::EMPTY_CONTENT_INDEX {
                    println!("  ⛔ emblem = 0xFFFF: expect level and Light to appear with the \
                              emblem still BLACK. That is not a failure of this object.");
                }
            }
            if !*dry_run {
                let dir = std::path::Path::new(out);
                let paths = roster::write_fixtures(dir, &b)?;
                // Re-read what we wrote, through the loader the responder patch uses. Writing a
                // file is not evidence it can be loaded back under the key it claims.
                let loaded = roster::load_fixture_dir(dir)?;
                for p in &paths {
                    println!("\nwrote {}", p.display());
                }
                println!("  {} fixture(s) reload and re-decode under their own filename keys",
                         loaded.len());
                println!("  D1_ROSTER_DIR={out}");
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn parse_pair(s: &str, what: &str) -> anyhow::Result<(u64, u64)> {
    let (a, b) = s
        .split_once(':')
        .ok_or_else(|| anyhow::anyhow!("{what} wants INDEX:VALUE, got {s:?}"))?;
    Ok((
        parse_int(a).map_err(anyhow::Error::msg)?,
        parse_int(b).map_err(anyhow::Error::msg)?,
    ))
}

fn run_activity_host(cli: &Cli, cmd: &AhCmd) -> anyhow::Result<ExitCode> {
    use d1_eboot::descriptors;
    use d1_eboot::image::Image;
    use d1_oracle::activity_host::{self, AhCodec};

    match cmd {
        AhCmd::Layout => {
            let image = Image::open_default()?;
            let index = descriptors::index(&image)?;
            let all = activity_host::derive_all(&image, &index)?;
            if cli.json {
                let rows: Vec<_> = all
                    .iter()
                    .map(|s| {
                        serde_json::json!({
                            "udt": s.user_defined_type,
                            "descriptor_global": format!("{:#x}", s.descriptor_global),
                            "runtime_record": format!("{:#x}", s.runtime_record),
                            "table_slot": format!("{:#x}", s.table_slot),
                            "checksum": format!("{:#010x}", s.checksum),
                            "static_record": format!("{:#x}", s.static_record),
                            "anchor": format!("{:#x}", s.anchor),
                            "tag": format!("{:#010x}", s.tag),
                            "struct_size": s.struct_size,
                            "field_count": s.field_count,
                            "call_site_size": s.call_site_size,
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string(&rows)?);
            } else {
                println!(
                    "Activity-Host schema tags - decode FUN_{:06X}, encode FUN_{:06X}\n\
                     udt  name                                    tag         struct  flds  call site",
                    activity_host::DECODE_FN,
                    activity_host::ENCODE_FN
                );
                for s in &all {
                    let name = d1_activity_host_name(s.user_defined_type);
                    let cs = match s.call_site_size {
                        Some(w) if w == s.struct_size => format!("{w:#07x} ✓"),
                        Some(w) => format!("{w:#07x} ** MISMATCH"),
                        None => "        -".to_string(),
                    };
                    println!(
                        "{:>3}  {name:<38}  {:#010x}  {:#07x}  {:>3}   {cs}",
                        s.user_defined_type, s.tag, s.struct_size, s.field_count
                    );
                }
                println!(
                    "\nchain per row: global -> reloc -> BSS record -> sorted checksum table -> \
                     checksum -> static record -> anchor+0x50 -> tag"
                );
            }
            Ok(ExitCode::SUCCESS)
        }

        AhCmd::Controls => {
            let cs = activity_host::controls()?;
            let mut ok = true;
            for c in &cs {
                ok &= c.pass;
                println!("{}  {:<72} {}", if c.pass { "PASS" } else { "FAIL" }, c.name, c.detail);
            }
            println!(
                "\n{}",
                if ok {
                    "ALL ACTIVITY-HOST CODEC CONTROLS PASS"
                } else {
                    "*** A CONTROL FAILED - nothing this tool produced may be cited ***"
                }
            );
            Ok(if ok { ExitCode::SUCCESS } else { ExitCode::from(2) })
        }

        AhCmd::Leak { cycles } => {
            let t0 = std::time::Instant::now();
            let cs = activity_host::leak_control(*cycles)?;
            let mut ok = true;
            for c in &cs {
                ok &= c.pass;
                println!("{}  {:<72} {}", if c.pass { "PASS" } else { "FAIL" }, c.name, c.detail);
            }
            println!(
                "\n{} ({} cycles in {:.1} s)",
                if ok {
                    "⭐ THE ENCODER CAN RUN UNBOUNDED - arena high-water flat, inversion red"
                } else {
                    "*** THE LEAK CONTROL FAILED - a long run will die of `scratch heap exhausted` ***"
                },
                cycles,
                t0.elapsed().as_secs_f64()
            );
            Ok(if ok { ExitCode::SUCCESS } else { ExitCode::from(2) })
        }

        AhCmd::Arena { cycles } => {
            let m = activity_host::measure_arena(*cycles, true)?;
            println!("cycles {}  guest allocations {}", m.cycles, m.guest_allocs);
            for (udt, first, last) in &m.high_water {
                println!(
                    "  udt {udt:>2}  high water after cycle 1 {first:#x}  after {} cycles {last:#x}  \
                     {}",
                    m.cycles,
                    if first == last { "FLAT" } else { "** GROWING **" }
                );
            }
            if let Some(s) = &m.stopped {
                println!("  STOPPED: {s}");
            }
            Ok(if m.is_flat() { ExitCode::SUCCESS } else { ExitCode::from(2) })
        }

        AhCmd::Decode { udt, wire } => {
            let udt = *udt as u32;
            let bytes = match wire {
                Some(h) => hex(h)?,
                None if udt == 3 => hex(activity_host::CAPTURED_JOIN_REQUEST)?,
                None => anyhow::bail!("--wire is required for udt {udt}"),
            };
            let mut codec = AhCodec::new();
            let d = codec.decode(udt, &bytes)?;
            println!(
                "udt {udt}  tag {:#010x}  {} bytes wire / {} bits -> {}-byte struct",
                d.tag,
                bytes.len(),
                d.bits,
                d.object.len()
            );
            for (off, v) in d.nonzero_qwords() {
                println!("  +{off:#06x}  {v:#018x}");
            }
            Ok(ExitCode::SUCCESS)
        }

        AhCmd::Encode { udt, obj } => {
            let udt = *udt as u32;
            let mut codec = AhCodec::new();
            let size = codec.struct_size(udt)?;
            let mut image = vec![0u8; size];
            if let Some(h) = obj {
                let raw = hex(h)?;
                if raw.len() != size {
                    anyhow::bail!("--obj is {} bytes, udt {udt}'s struct is {size}", raw.len());
                }
                image.copy_from_slice(&raw);
            }
            let e = codec.encode(udt, &image)?;
            println!("udt {udt}  tag {:#010x}  {} bytes / {} bits", e.tag, e.payload.len(), e.bits);
            println!("{}", hexs(&e.payload));
            Ok(ExitCode::SUCCESS)
        }
    }
}

/// The 49-entry `user_defined_type` name table lives in `d1-activity-host`, which this crate must
/// not depend on (it depends on `d1-bap` and tokio). Only the rows we derive are named here, and
/// only for the human-readable listing.
fn d1_activity_host_name(udt: u32) -> &'static str {
    match udt {
        0 => "entity_slots_allocated",
        1 => "global_activity_state",
        3 => "join_request",
        4 => "join_result",
        19 => "allocate_entity_indices",
        20 => "free_entity_indices",
        22 => "client_identity_update",
        35 => "connectivity_failure",
        36 => "membership_acknowledgement",
        _ => "",
    }
}

fn run_profile(cli: &Cli, cmd: &ProfileCmd) -> anyhow::Result<ExitCode> {
    use d1_eboot::descriptors;
    use d1_eboot::image::Image;
    use d1_oracle::profile::{self, ProfileSpec, Progression};

    match cmd {
        ProfileCmd::Layout => {
            let image = Image::open_default()?;
            let idx = descriptors::index(&image)?;
            let l = profile::derive_layout(&image, &idx)?;
            if cli.json {
                println!("{}", serde_json::json!({
                    "struct_size": l.struct_size,
                    "soid": l.soid, "identity": l.identity, "customization": l.customization,
                    "last_orbited_destination": l.last_orbited_destination,
                    "item_slots": {"base": l.item_slots.base, "count": l.item_slots.count,
                                   "elem": l.item_slots.elem_size, "tag": l.item_slots.tag},
                    "equipped": {"base": l.equipped.base, "count": l.equipped.count,
                                 "elem": l.equipped.elem_size, "tag": l.equipped.tag},
                    "progression_count": l.progression_count,
                    "progressions": {"base": l.progressions.base, "count": l.progressions.count,
                                     "elem": l.progressions.elem_size, "tag": l.progressions.tag},
                    "stat_block": l.stat_block,
                    "stat_override_present": l.stat_override_present,
                    "stat_overrides": {"base": l.stat_overrides.base, "count": l.stat_overrides.count,
                                       "elem": l.stat_overrides.elem_size, "tag": l.stat_overrides.tag},
                }));
            } else {
                println!("profile {:#010x}  struct {} bytes", profile::TAG, l.struct_size);
                println!("  +{:#07x}  SOID", l.soid);
                println!("  +{:#07x}  identity {{race, gender, class}}", l.identity);
                println!("  +{:#07x}  customization block, 12 fields", l.customization);
                println!("  +{:#07x}  last_orbited_destination  ⚠ NOT `personality`", l.last_orbited_destination);
                for (what, a) in [
                    ("inventory slots", l.item_slots),
                    ("equipped item SOIDs", l.equipped),
                    ("progressions", l.progressions),
                    ("stat overrides", l.stat_overrides),
                ] {
                    println!("  +{:#07x}  {:<20} {} x {} B   {:#010x}",
                             a.base, what, a.count, a.elem_size, a.tag);
                }
                println!("  +{:#07x}  progression count (7 bits - it GATES the table)", l.progression_count);
                println!("  +{:#07x}  stat-override present flag", l.stat_override_present);
            }
            Ok(ExitCode::SUCCESS)
        }

        ProfileCmd::Controls { baseline } => {
            let p = std::path::Path::new(baseline);
            let cs = profile::controls(p.exists().then_some(p))?;
            let mut ok = true;
            for c in &cs {
                ok &= c.pass;
                println!("{}  {:<28} {}", if c.pass { "PASS" } else { "FAIL" }, c.name, c.detail);
            }
            println!("\n{}", if ok {
                "ALL PROFILE CONTROLS PASS".to_string()
            } else {
                "*** A CONTROL FAILED - nothing this tool produced may be cited ***".to_string()
            });
            Ok(if ok { ExitCode::SUCCESS } else { ExitCode::from(2) })
        }

        ProfileCmd::Build {
            out, soid, identity, destination, light, light_index, stats, inventory, equip,
            no_equip, progressions, phantom_item_slots, no_content_indices, all_content, complete,
            complete_flags, flag_table, pending_loot, dry_run,
        } => {
            let id: Vec<u8> = identity
                .split(',')
                .map(|s| parse_int(s).map(|v| v as u8).map_err(anyhow::Error::msg))
                .collect::<anyhow::Result<_>>()?;
            if id.len() != 3 {
                anyhow::bail!("--identity wants race,gender,class, got {identity:?}");
            }

            let mut overrides: Vec<(u8, u32)> = Vec::new();
            if *light > 0 {
                overrides.push((*light_index, *light));
            }
            for s in stats {
                let (k, v) = parse_pair(s, "--stat")?;
                overrides.push((k as u8, v as u32));
            }

            // ⭐ The 340-slot array and the 20-slot equipped list are built from the SAME staged
            // elements, so they cannot disagree about which item is in which slot - and slot 12 is
            // skipped in both, because the client reads exactly that slot as the emblem.
            let table = if *no_content_indices {
                None
            } else {
                Some(d1_oracle::item_index::IndexTable::load_default()?)
            };
            let mut item_slots: Vec<d1_oracle::profile::ItemSlot> = Vec::new();
            let equipped = if *no_equip {
                Vec::new()
            } else if !equip.is_empty() {
                equip.clone()
            } else {
                let dir = std::path::Path::new(inventory);
                if dir.exists() {
                    // ⭐⭐ Placement is by BUCKET, not by enumeration order.
                    // `cui::c_investment_backpack_item::ctor` (0x9F2CC0) passes the SLOT INDEX to
                    // `FUN_0026DB60`, which returns the bucket from a 39-record content table -
                    // the item is not an input. Writing item `n` at slot `n` put all thirteen in
                    // slots 0-12, i.e. bucket 0x05 MATERIALS, and that is what the screen showed.
                    // The bucket comes from the staged file's own `NN-<bucket>.bin` name, so the
                    // element and its placement cannot disagree.
                    let staged = d1_oracle::inventory::read_staged(dir)?;
                    // ⭐⭐ The +0x2B80 equipped list is ORDINAL-INDEXED: `0x269080` derefs
                    // `character + 0x2B80 + slot*8` where slot = the item DEFINITION's own
                    // `def_client+0x18` byte (character-screen-consumers.md §5, measured offline
                    // by art-arrangement.py; the emblem's definition says 12, agreeing with the
                    // replayed emblem slot). File order - what every profile before 2026-08-10
                    // used - makes the consumer resolve the WRONG entry for every item whose
                    // ordinal differs from its file position.
                    let art = d1_oracle::item_index::ArtTable::load_default().ok();
                    if art.is_none() {
                        eprintln!("⚠ no art-arrangement table - equipped list falls back to FILE \
                                   ORDER, which 0x269080 resolves wrongly for most items");
                    }
                    let mut eq: Vec<u64> = if art.is_some() {
                        vec![0u64; d1_oracle::profile::EQUIPPED_SLOTS]
                    } else {
                        Vec::new()
                    };
                    let mut nth: std::collections::BTreeMap<String, u16> = Default::default();
                    let mut unplaced: Vec<String> = Vec::new();
                    for it in &staged {
                        // ⚠ A REBUILT element carries a LEVEL at +0x18, not a hash - so take the
                        // content index the element already states, and only fall back to the
                        // hash→index table for a legacy element.
                        let ci = if it.content_index != profile::EMPTY_INDEX && it.content_index != 1 {
                            it.content_index
                        } else {
                            match table.as_ref() {
                                Some(t) => t.require(it.item_hash)?,
                                None => profile::EMPTY_INDEX,
                            }
                        };
                        match art.as_ref().and_then(|t| {
                            (ci != profile::EMPTY_INDEX).then(|| t.equip_slot_for(ci)).flatten()
                        }) {
                            Some(s) if s < d1_oracle::profile::EQUIPPED_SLOTS => {
                                if eq[s] != 0 {
                                    anyhow::bail!(
                                        "two staged items claim equipped ordinal {s} - 0x269080 \
                                         can only see one of them");
                                }
                                eq[s] = it.soid;
                            }
                            Some(s) => anyhow::bail!("equipped ordinal {s} is out of range"),
                            None if art.is_some() => {
                                eprintln!("⚠ content index {ci} has no measured equipped ordinal \
                                           - NOT in the +0x2B80 equipped list");
                            }
                            None => {
                                while eq.len() == d1_oracle::item_index::EMBLEM_SLOT {
                                    eq.push(0);
                                }
                                eq.push(it.soid);
                            }
                        }
                        if ci == profile::EMPTY_INDEX {
                            continue;
                        }
                        let n = nth.entry(it.bucket.clone()).or_insert(0);
                        match d1_oracle::inventory::slot_for_bucket(&it.bucket, *n) {
                            Some(slot) => {
                                *n += 1;
                                item_slots.push(d1_oracle::profile::ItemSlot {
                                    content_index: ci,
                                    item_soid: it.soid,
                                    slot,
                                });
                            }
                            // ⛔ No determined bucket id ⇒ leave it OUT of the 340-slot backpack
                            //    rather than dropping it into Materials. It still exists in family
                            //    4/3 and is still named by the 20-entry equipped list.
                            None => unplaced.push(it.bucket.clone()),
                        }
                    }
                    if !unplaced.is_empty() {
                        eprintln!(
                            "⚠ {} item(s) have no determined bucket id and are NOT in the 340-slot \
                             backpack array: {unplaced:?}\n  They are among the five equippable \
                             count-10 buckets 0x19/0x1B/0x1C/0x1E/0x24; nothing has determined \
                             which is which. They remain equipped via +0x2B80.",
                            unplaced.len());
                    }
                    eq
                } else {
                    eprintln!("note: {inventory} does not exist - equipping nothing");
                    Vec::new()
                }
            };

            let progs: Vec<Progression> = progressions
                .iter()
                .map(|s| {
                    let (i, p) = parse_pair(s, "--progression")?;
                    Ok(Progression { progress: p as u32, daily: 0, weekly: 0, index: i as u16 })
                })
                .collect::<anyhow::Result<_>>()?;

            let spec = ProfileSpec {
                soid: *soid,
                identity: [id[0], id[1], id[2]],
                last_orbited_destination: *destination as u32,
                stat_overrides: overrides,
                equipped,
                progressions: progs,
                item_slots,
                empty_item_slots: !*phantom_item_slots,
                all_content: *all_content,
                completions: {
                    let mut subs = complete.clone();
                    if !complete_flags.is_empty() {
                        let text = std::fs::read_to_string(flag_table)
                            .map_err(|e| anyhow::anyhow!("--flag-table {flag_table}: {e}"))?;
                        let mut rows = std::collections::HashMap::new();
                        for line in text.lines().skip(1) {
                            let f: Vec<&str> = line.split('\t').collect();
                            if f.len() == 4 {
                                rows.insert(f[0].parse::<u16>()?, (f[2].parse::<u8>()?, f[3].parse::<u16>()?));
                            }
                        }
                        for i in complete_flags {
                            match rows.get(i) {
                                Some((3, sub)) => subs.push(*sub),
                                Some((k, _)) => anyhow::bail!("unlock flag {i} is storage kind {k}, not 3 - it has no home in field [11]"),
                                None => anyhow::bail!("unlock flag {i} is not in {flag_table}"),
                            }
                        }
                    }
                    subs.sort_unstable();
                    subs.dedup();
                    subs
                },
                pending_loot: pending_loot.iter().map(|v| {
                    let f: Vec<&str> = v.split(':').collect();
                    let num = |t: &str| t.strip_prefix("0x").map_or_else(|| t.parse::<u64>().ok(), |h| u64::from_str_radix(h, 16).ok());
                    (|| match f.as_slice() {
                        [i, k, key, n] => Some(profile::PendingLoot {
                            content_index: num(i)? as u16, source_kind: num(k)? as u8, key: num(key)? as u32, nonce: num(n)?,
                        }),
                        _ => None,
                    })().ok_or_else(|| anyhow::anyhow!("--pending-loot {v:?}: want CONTENT_INDEX:SOURCE_KIND:KEY:NONCE"))
                }).collect::<Result<Vec<_>, _>>()?,
            };

            let mut oracle = ObjectOracle::new(profile::TAG)?;
            let b = profile::build(&mut oracle, &spec)?;
            let nz = b.nonzero();

            if cli.json {
                println!("{}", serde_json::json!({
                    "out": if *dry_run { String::new() } else { out.clone() },
                    "bytes": b.payload.len(), "bits": b.bits,
                    "nonzero": nz.iter().map(|(k, v)| (format!("{k:#x}"), format!("{v:#x}")))
                        .collect::<Vec<_>>(),
                }));
            } else {
                println!("profile {:#010x}  {} B / {} bits  round trip: object IDENTICAL",
                         profile::TAG, b.payload.len(), b.bits);
                println!("  equipped {} slot(s); {} stat override(s); {} progression(s); {} completion flag(s)",
                         spec.equipped.len(), spec.stat_overrides.len(), spec.progressions.len(),
                         spec.completions.len());
                println!("  ⭐ inventory slots NAMED: {} of {} (the array 0x1087660 walks); the rest {}",
                         spec.item_slots.len(), d1_oracle::profile::ITEM_SLOTS,
                         if spec.empty_item_slots { "0xFFFF = EMPTY" } else { "zero = 340 phantom items" });
                println!("     emblem slot {} of the equipped list is reserved (soid {:#x})",
                         d1_oracle::item_index::EMBLEM_SLOT,
                         spec.equipped.get(d1_oracle::item_index::EMBLEM_SLOT).copied().unwrap_or(0));
                if spec.all_content {
                    println!("  ⭐ all-content override SET (+0x6E4C/+0x6E4D) - every investment \
                              requirement reports satisfied; offer-key ownership UNCHANGED");
                }
                println!("  {} non-zero qwords:", nz.len());
                for (o, v) in nz.iter().take(40) {
                    println!("    +{o:#07x}  {v:#018x}");
                }
                if nz.len() > 40 {
                    println!("    … {} more (use --json for all)", nz.len() - 40);
                }
            }
            if !*dry_run {
                if let Some(d) = std::path::Path::new(out).parent() {
                    std::fs::create_dir_all(d)?;
                }
                std::fs::write(out, &b.payload)?;
                println!("\nwrote {out}  ({} bytes)\n  D1_PROFILE_FILE={out}", b.payload.len());
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}

/// The measured emblem content index, for the warning text when `--emblem` overrides it.
fn t_index(t: Option<&d1_oracle::item_index::IndexTable>) -> Option<u16> {
    t.and_then(|t| t.index_of(d1_oracle::item_index::DEFAULT_EMBLEM_HASH))
}

fn run_character(cli: &Cli, cmd: &CharacterCmd) -> anyhow::Result<ExitCode> {
    use d1_eboot::descriptors;
    use d1_eboot::image::Image;
    use d1_oracle::character::{self, CharacterSpec};

    match cmd {
        CharacterCmd::Layout => {
            let image = Image::open_default()?;
            let idx = descriptors::index(&image)?;
            let l = character::derive_layout(&image, &idx)?;
            if cli.json {
                println!("{}", serde_json::json!({
                    "struct_size": l.struct_size,
                    "soid": l.soid, "identity": l.identity, "customization": l.customization,
                    "stats_block": l.stats_block, "level": l.level,
                    "derived_level": l.derived_level,
                    "equipment": {"base": l.equipment.base, "count": l.equipment.count,
                                  "elem": l.equipment.elem_size, "tag": l.equipment.tag,
                                  "element_tag": l.equip_elem_tag,
                                  "item_soid": l.equip_item_soid,
                                  "content_index": l.equip_content_index},
                    "stats": {"base": l.stats.base, "count": l.stats.count,
                              "elem": l.stats.elem_size, "tag": l.stats.tag,
                              "element_tag": l.stat_elem_tag,
                              "key": l.stat_key, "value": l.stat_value},
                    "stats_siblings": l.stats_siblings,
                }));
            } else {
                println!("character card {:#010x}  struct {} bytes", character::TAG, l.struct_size);
                println!("  +{:#07x}  SOID", l.soid);
                println!("  +{:#07x}  identity {{race, gender, class}}  ⚠ must match the profile", l.identity);
                println!("  +{:#07x}  customization block, 12 fields", l.customization);
                println!("  +{:#07x}  ⭐ LEVEL (u32, stated by the server)      0xA11555", l.level);
                println!("  +{:#07x}  derived level (f32, floor()'d)          0xA1151C", l.derived_level);
                println!("  +{:#07x}  ⭐ EQUIPMENT  {} x {} B   {:#010x} / element {:#010x}",
                         l.equipment.base, l.equipment.count, l.equipment.elem_size,
                         l.equipment.tag, l.equip_elem_tag);
                println!("             element +{:#04x} item SOID (u64)   +{:#04x} ⭐ CONTENT INDEX (u16) -> the emblem, 0xA11627",
                         l.equip_item_soid, l.equip_content_index);
                println!("  +{:#07x}  ⭐ STATS      {} x {} B   {:#010x} / element {:#010x}",
                         l.stats.base, l.stats.count, l.stats.elem_size, l.stats.tag, l.stat_elem_tag);
                println!("             element +{:#04x} stat index (u8, 0xFF = ABSENT)   +{:#04x} value (u32)   0x2666F0 / 0x266740",
                         l.stat_key, l.stat_value);
                println!("  +{:#07x} +{:#07x} +{:#07x}  three sibling stat tables  [U] - no reader identified",
                         l.stats_siblings[0], l.stats_siblings[1], l.stats_siblings[2]);
            }
            Ok(ExitCode::SUCCESS)
        }

        CharacterCmd::Controls { baseline } => {
            let p = std::path::Path::new(baseline);
            let cs = character::controls(p.exists().then_some(p))?;
            let mut ok = true;
            for c in &cs {
                ok &= c.pass;
                println!("{}  {:<34} {}", if c.pass { "PASS" } else { "FAIL" }, c.name, c.detail);
            }
            println!("\n{}", if ok {
                "ALL CHARACTER-CARD CONTROLS PASS"
            } else {
                "*** A CONTROL FAILED - nothing this tool produced may be cited ***"
            });
            Ok(if ok { ExitCode::SUCCESS } else { ExitCode::from(2) })
        }

        CharacterCmd::Build {
            out, soid, identity, level, derived_level, light, light_index, stats, inventory,
            no_equip, emblem, no_content_indices, sentinel_all_stat_tables, zero_stat_keys, dry_run,
        } => {
            let table = if *no_content_indices {
                None
            } else {
                Some(d1_oracle::item_index::IndexTable::load_default()?)
            };
            let id: Vec<u8> = identity
                .split(',')
                .map(|s| parse_int(s).map(|v| v as u8).map_err(anyhow::Error::msg))
                .collect::<anyhow::Result<_>>()?;
            if id.len() != 3 {
                anyhow::bail!("--identity wants race,gender,class, got {identity:?}");
            }

            let mut stat_entries: Vec<(u8, u32)> = Vec::new();
            if *light > 0 {
                stat_entries.push((*light_index, *light));
            }
            for s in stats {
                let (k, v) = parse_pair(s, "--stat")?;
                stat_entries.push((k as u8, v as u32));
            }

            // ⭐ The art-arrangement table fills equipment element +0x0C - the u16 that names each
            // item's 3D model. Absent table = 0xFFFF everywhere (no geometry, honestly), with a
            // warning, because a silently-empty +0x0C is exactly the pre-2026-08-10 card.
            let art = match d1_oracle::item_index::ArtTable::load_default() {
                Ok(t) => Some(t),
                Err(e) => {
                    eprintln!("⚠ no art-arrangement table - every +0x0C stays 0xFFFF (no 3D \
                               model). {e}");
                    None
                }
            };
            let equipment = if *no_equip {
                Vec::new()
            } else {
                let dir = std::path::Path::new(inventory);
                if dir.exists() {
                    character::equipment_from_inventory_with(
                        dir, table.as_ref(), art.as_ref().map(|t| (t, id[2])))?
                } else {
                    eprintln!("note: {inventory} does not exist - equipping nothing");
                    Vec::new()
                }
            };

            let mut spec = CharacterSpec {
                soid: *soid,
                identity: [id[0], id[1], id[2]],
                level: *level,
                derived_level: derived_level.unwrap_or(*level as f32),
                stats: stat_entries,
                equipment,
                sentinel_stat_keys: !*zero_stat_keys,
                sentinel_all_stat_tables: *sentinel_all_stat_tables,
                empty_equipment_indices: true,
            };
            match (emblem, table.as_ref()) {
                (Some(e), _) => {
                    let (slot, index) = parse_pair(e, "--emblem")?;
                    eprintln!(
                        "⚠ --emblem {slot}:{index} is an OVERRIDE. The measured pair is slot {}:{} \
                         - anything else is a claim you must be able to defend.",
                        character::EMBLEM_SLOT,
                        t_index(table.as_ref()).map(|i| i.to_string()).unwrap_or("?".into()));
                    spec.emblem(slot as usize, index as u16)?;
                }
                (None, Some(t)) => {
                    let index = t.require(d1_oracle::item_index::DEFAULT_EMBLEM_HASH)?;
                    spec.emblem(character::EMBLEM_SLOT, index)?;
                }
                (None, None) => {}
            }

            let mut oracle = ObjectOracle::new(character::TAG)?;
            let b = d1_oracle::character::build(&mut oracle, &spec)?;
            let nz = b.nonzero();

            if cli.json {
                println!("{}", serde_json::json!({
                    "out": if *dry_run { String::new() } else { out.clone() },
                    "bytes": b.payload.len(), "bits": b.bits,
                    "nonzero": nz.iter().map(|(k, v)| (format!("{k:#x}"), format!("{v:#x}")))
                        .collect::<Vec<_>>(),
                }));
            } else {
                println!("character card {:#010x}  {} B / {} bits  round trip: object IDENTICAL",
                         character::TAG, b.payload.len(), b.bits);
                println!("  level {}  derived {}  stats {}  equipment slots used {} of {}",
                         spec.level, spec.derived_level, spec.stats.len(),
                         spec.equipment.len(), character::EQUIPMENT_SLOTS);
                let set = spec.equipment.iter()
                    .filter(|e| e.content_index != character::EMPTY_CONTENT_INDEX).count();
                println!("  content indices set: {set} of {} slots; emblem slot {} = {}",
                         spec.equipment.len(), character::EMBLEM_SLOT,
                         spec.equipment.get(character::EMBLEM_SLOT)
                             .map(|e| format!("{:#06x}", e.content_index))
                             .unwrap_or_else(|| "(absent)".into()));
                println!("  {} non-zero qwords:", nz.len());
                for (o, v) in nz.iter().take(60) {
                    println!("    +{o:#07x}  {v:#018x}");
                }
                if nz.len() > 60 {
                    println!("    … {} more (use --json for all)", nz.len() - 60);
                }
            }
            if !*dry_run {
                if let Some(d) = std::path::Path::new(out).parent() {
                    std::fs::create_dir_all(d)?;
                }
                std::fs::write(out, &b.payload)?;
                println!("\nwrote {out}  ({} bytes)\n  D1_CHARACTER_FILE={out}", b.payload.len());
            }
            Ok(ExitCode::SUCCESS)
        }

        CharacterCmd::BuildPeer {
            out_dir, contact_key, card_key, name, clan, identity, level, light, light_index,
            stats, inventory, no_equip, no_emblem, with_family2, dry_run,
        } => {
            use d1_oracle::character as ch;
            let id: Vec<u8> = identity
                .split(',')
                .map(|s| parse_int(s).map(|v| v as u8).map_err(anyhow::Error::msg))
                .collect::<anyhow::Result<_>>()?;
            if id.len() != 3 {
                anyhow::bail!("--identity wants race,gender,class, got {identity:?}");
            }
            let mut stat_entries: Vec<(u8, u32)> = Vec::new();
            if *light > 0 {
                stat_entries.push((*light_index, *light));
            }
            for s in stats {
                let (k, v) = parse_pair(s, "--stat")?;
                stat_entries.push((k as u8, v as u32));
            }
            let table = d1_oracle::item_index::IndexTable::load_default().ok();
            let art = match d1_oracle::item_index::ArtTable::load_default() {
                Ok(t) => Some(t),
                Err(e) => {
                    eprintln!("⚠ no art-arrangement table - every +0x0C stays 0xFFFF (no 3D \
                               model). {e}");
                    None
                }
            };
            let equipment = if *no_equip {
                Vec::new()
            } else {
                let dir = std::path::Path::new(inventory);
                if dir.exists() {
                    character::equipment_from_inventory_with(
                        dir, table.as_ref(), art.as_ref().map(|t| (t, id[2])))?
                } else {
                    eprintln!("note: {inventory} does not exist - equipping nothing");
                    Vec::new()
                }
            };
            // ⭐ THE PEER OBJECT'S +0x00 IS THE QUEUEZ LINK KEY, NOT A FREE SOID.
            // The responder headers f0t1 by its filename key (= card_key), and the client compares
            // that header id against the RSAT payload's leading qword (+0x00). If they differ it
            // rejects the object with `error.investment_server_object_queuez_id_mismatch_in_header_
            // and_rsat_payload` ("porcupine") before it ever enters the registry. So +0x00 MUST be
            // card_key - the same key f0t0's +0x08 points at. (Prior bug: this was CHARACTER_SOID.)
            let mut spec = CharacterSpec {
                soid: *card_key,
                identity: [id[0], id[1], id[2]],
                level: *level,
                derived_level: *level as f32,
                stats: stat_entries,
                equipment,
                sentinel_stat_keys: true,
                sentinel_all_stat_tables: false,
                empty_equipment_indices: true,
            };
            if !*no_emblem {
                if let Some(t) = table.as_ref() {
                    let index = t.require(d1_oracle::item_index::DEFAULT_EMBLEM_HASH)?;
                    spec.emblem(character::EMBLEM_SLOT, index)?;
                }
            }

            // The two-hop: family 0/0 contact -> family 0/1 peer view, exactly the family-2 shape.
            let rspec = d1_oracle::RosterSpec {
                contact_key: *contact_key,
                card_key: *card_key,
                display_name: name.clone(),
                clan_tag: clan.clone(),
                level: *level,
                light: *light,
                ..d1_oracle::RosterSpec::default()
            };
            let contact_obj = rspec.contact_object()?;
            let mut co = ObjectOracle::new(d1_oracle::roster::PEER_CONTACT_TAG)?;
            let (cenc, cdec) = co.round_trip(&contact_obj, 0x4000)?;
            if cdec.object != contact_obj {
                anyhow::bail!("family 0/0 contact did not round-trip identical");
            }

            let mut po = ObjectOracle::new(ch::PEER_TAG)?;
            let peer = ch::build_peer(&mut po, &spec)?;

            // ⭐ THE QUEUEZ IDENTITY MATRIX - decoded from the ENCODED payloads, not asserted from
            // the specs. A clean codec round-trip proves the payload is well-formed; it does NOT
            // prove its embedded +0x00 agrees with the QueueZ envelope key. Porcupine is exactly
            // that disagreement, so this checks it directly before anything is written.
            let f0t0_soid = u64::from_le_bytes(cdec.object[0x00..0x08].try_into().unwrap());
            let f0t0_link = u64::from_le_bytes(cdec.object[0x08..0x10].try_into().unwrap());
            let f0t1_soid = u64::from_le_bytes(peer.decoded.object[0x00..0x08].try_into().unwrap());
            println!("QueueZ identity matrix (decoded from the encoded payloads):");
            println!("           filename/header key    decoded +0x00         +0x08 link");
            println!("  f0t0     {contact_key:#018x}   {f0t0_soid:#018x}   {f0t0_link:#018x}");
            println!("  f0t1     {card_key:#018x}   {f0t1_soid:#018x}   -");
            let mut bad = Vec::new();
            if f0t0_soid != *contact_key { bad.push(format!("f0t0 +0x00 {f0t0_soid:#018x} != contact_key {contact_key:#018x}")); }
            if f0t0_link != *card_key    { bad.push(format!("f0t0 +0x08 {f0t0_link:#018x} != card_key {card_key:#018x}")); }
            if f0t1_soid != *card_key    { bad.push(format!("f0t1 +0x00 {f0t1_soid:#018x} != card_key {card_key:#018x} (⇒ PORCUPINE)")); }
            if !bad.is_empty() {
                anyhow::bail!("⛔ QueueZ identity mismatch - the client would reject these with \
                               porcupine:\n    {}", bad.join("\n    "));
            }
            println!("  ✅ header == filename == decoded +0x00 for both; f0t1 +0x00 == f0t0 +0x08 link");

            println!("family 0 peer pair (the object 0x9A3C30 reads - NOT the family-3 card)");
            println!("  f0t0 {:#010x}  {} B / {} bits  contact_key {:#018x} -> card_key {:#018x}",
                     d1_oracle::roster::PEER_CONTACT_TAG, cenc.payload.len(), cenc.bits,
                     contact_key, card_key);
            println!("  f0t1 {:#010x}  {} B / {} bits  soid {:#018x}  level {} @+{:#05x}  \
                      stat[0] {{{}, {}}} @+{:#05x}",
                     ch::PEER_TAG, peer.payload.len(), peer.bits, spec.soid, spec.level,
                     ch::PEER_BLOCK, spec.stats.first().map(|s| s.0).unwrap_or(0),
                     spec.stats.first().map(|s| s.1).unwrap_or(0),
                     ch::PEER_BLOCK + (ch::OFF_STATS - ch::OFF_STATS_BLOCK));
            let set = spec.equipment.iter()
                .filter(|e| e.content_index != character::EMPTY_CONTENT_INDEX).count();
            println!("       equipment slots used {} of {} ({set} with a content index)",
                     spec.equipment.len(), character::EQUIPMENT_SLOTS);

            if !*dry_run {
                std::fs::create_dir_all(out_dir)?;
                let c = format!("{out_dir}/f0t0-{contact_key:016x}.bin");
                let p = format!("{out_dir}/f0t1-{card_key:016x}.bin");
                std::fs::write(&c, &cenc.payload)?;
                std::fs::write(&p, &peer.payload)?;
                println!("\nwrote {c}\nwrote {p}");
                if *with_family2 {
                    let r = d1_oracle::roster::build(&rspec)?;
                    for o in r.objects() {
                        let f = format!("{out_dir}/f{}t{}-{:016x}.bin", o.family, o.obj_type, o.key);
                        std::fs::write(&f, &o.payload)?;
                        println!("wrote {f}");
                    }
                }
                println!("\n  D1_ROSTER_DIR={out_dir}");
                println!("  ⚠ family 0 SUBSTITUTES the rich nameplate fill 0xA2D9F0 for the compact \
                          0xA2DE30 - it is an upgrade path, but bisect against D1_ROSTER_DIR=\
                          capture-rig/rosters if the plate changes.");
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}

/// Resolve a repo-relative default path by ascending from the cwd until it exists, the same rule
/// `IndexTable::load_default` uses. Without this, running from `d1-server/` writes a *second*
/// `capture-rig/item-content-index.tsv` one directory down and leaves the real one stale - a
/// generator that silently writes to the wrong place is worse than one that refuses.
/// An absolute path, or a relative one that already resolves, is returned untouched.
fn resolve_repo_path(p: &std::path::Path) -> PathBuf {
    if p.is_absolute() || p.exists() {
        return p.to_path_buf();
    }
    if let Ok(mut dir) = std::env::current_dir() {
        loop {
            let c = dir.join(p);
            if c.exists() {
                return c;
            }
            if !dir.pop() {
                break;
            }
        }
    }
    p.to_path_buf()
}

/// Print a control list the way every other `controls` subcommand here does, and map it to the exit
/// code the project's convention requires: **2 means do not cite anything above this line**.
fn report_controls(cs: &[d1_oracle::profile::Control], banner: &str) -> ExitCode {
    let mut ok = true;
    for c in cs {
        ok &= c.pass;
        println!("{}  {:<32} {}", if c.pass { "PASS" } else { "FAIL" }, c.name, c.detail);
    }
    println!(
        "\n{}",
        if ok {
            banner.to_string()
        } else {
            "*** A CONTROL FAILED - nothing this tool produced may be cited ***".to_string()
        }
    );
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    }
}

fn run_oodle(cli: &Cli, cmd: &OodleCmd) -> anyhow::Result<ExitCode> {
    use d1_oracle::oodle;
    match cmd {
        OodleCmd::Controls => Ok(report_controls(&oodle::controls(), "ALL OODLE CONTROLS PASS")),

        OodleCmd::Block { pkg, block, out } => {
            let mut c = oodle::Content::open_default()?;
            let (stored, compressed) = c.stored_block(pkg, *block as u32)?;
            let plain = c.plain_block(pkg, *block as u32)?.to_vec();
            let sha = d1_pkg::manifest::sha256_hex(&plain);
            if cli.json {
                println!("{}", serde_json::json!({
                    "package": pkg, "block": block, "compressed": compressed,
                    "stored": stored.len(), "plain": plain.len(), "sha256": sha,
                }));
            } else {
                println!(
                    "{pkg} block {block}: {} stored -> {} plain ({}), sha256 {sha}",
                    stored.len(),
                    plain.len(),
                    if compressed { "Oodle-compressed" } else { "stored verbatim" }
                );
            }
            if let Some(p) = out {
                std::fs::write(p, &plain)?;
                println!("wrote {}", p.display());
            }
            Ok(ExitCode::SUCCESS)
        }

        OodleCmd::Entry { tag, pkg, index, out, hex } => {
            let mut c = oodle::Content::open_default()?;
            let (name, idx, class_tag, data) = match (tag, pkg, index) {
                (Some(t), _, _) => c.entry_bytes_for_tag(*t as u32)?,
                (None, Some(p), Some(i)) => {
                    let (class_tag, d) = c.entry_bytes(p, *i as u32)?;
                    (p.clone(), *i as u32, class_tag, d)
                }
                _ => anyhow::bail!("give --tag, or --pkg with --index"),
            };
            // The container's own cross-check, the one `extract-entry.py` prints: most payloads
            // open with a u64 length equal to the entry size.
            let declared = if data.len() >= 8 {
                Some(u64::from_le_bytes(data[0..8].try_into().unwrap()))
            } else {
                None
            };
            let head: String =
                data.iter().take(*hex).map(|b| format!("{b:02x}")).collect::<Vec<_>>().join("");
            if cli.json {
                println!("{}", serde_json::json!({
                    "package": name, "index": idx, "class_tag": format!("{class_tag:#010x}"),
                    "size": data.len(), "self_length": declared,
                    "self_length_matches": declared == Some(data.len() as u64),
                    "sha256": d1_pkg::manifest::sha256_hex(&data), "head": head,
                }));
            } else {
                println!("{name} entry {idx}: class {class_tag:#010x}, {} bytes", data.len());
                match declared {
                    Some(d) if d == data.len() as u64 => println!("self-length header: {d} MATCHES"),
                    Some(d) => println!("self-length header: {d} (not a length header)"),
                    None => println!("self-length header: entry is shorter than 8 bytes"),
                }
                println!("sha256 {}", d1_pkg::manifest::sha256_hex(&data));
                println!("head: {head}");
            }
            if let Some(p) = out {
                std::fs::write(p, &data)?;
                println!("wrote {}", p.display());
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn run_activity_modes(_cli: &Cli, cmd: &ActivityModesCmd) -> anyhow::Result<ExitCode> {
    use d1_oracle::activity_modes as am;
    match cmd {
        ActivityModesCmd::Controls => {
            Ok(report_controls(&am::controls()?, "ALL ACTIVITY-MODES CONTROLS PASS"))
        }
        ActivityModesCmd::Write { out, dry_run } => {
            let out = &resolve_repo_path(out);
            let mut c = d1_oracle::oodle::Content::open_default()?;
            let t = am::ModesTable::read(&mut c)?;
            let text = t.render_tsv(am::GENERATOR);
            println!(
                "{} block {}: {} rows, {} launchable, sha256(block) = {}",
                am::SOURCE_PACKAGE,
                am::SOURCE_BLOCK,
                t.rows.len(),
                t.launchable_indices().len(),
                &t.block_sha256[..16],
            );
            if *dry_run {
                println!("--dry-run: {} bytes NOT written to {}", text.len(), out.display());
                return Ok(ExitCode::SUCCESS);
            }
            // Write through a temporary and only rename after the shared parser accepts it - a
            // generator must never leave an unparseable table on disk.
            let tmp = out.with_extension("tsv.new");
            std::fs::write(&tmp, &text)?;
            match am::parse_tsv(&std::fs::read_to_string(&tmp)?) {
                Ok(rows) if rows.len() == am::TABLE_COUNT => {
                    std::fs::rename(&tmp, out)?;
                    println!("wrote {}: {} rows, re-parsed clean", out.display(), rows.len());
                    Ok(ExitCode::SUCCESS)
                }
                Ok(rows) => {
                    let _ = std::fs::remove_file(&tmp);
                    println!("*** rendered table re-parsed to {} rows, not {}", rows.len(), am::TABLE_COUNT);
                    Ok(ExitCode::from(2))
                }
                Err(e) => {
                    let _ = std::fs::remove_file(&tmp);
                    println!("*** the rendered table does NOT re-parse: {e}");
                    println!("*** {} left untouched", out.display());
                    Ok(ExitCode::from(2))
                }
            }
        }
    }
}

fn run_item_index(_cli: &Cli, cmd: &ItemIndexCmd) -> anyhow::Result<ExitCode> {
    use d1_oracle::item_index as ii;
    match cmd {
        ItemIndexCmd::Controls => {
            Ok(report_controls(&ii::controls()?, "ALL ITEM-INDEX CONTROLS PASS"))
        }

        ItemIndexCmd::Write { out, dry_run } => {
            let out = out.clone().unwrap_or_else(|| PathBuf::from(ii::source().path));
            let out = &resolve_repo_path(&out);
            if let Some(dir) = out.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let mut c = d1_oracle::oodle::Content::open_default()?;
            let t = ii::SourceTable::read(&mut c)?;
            let text = t.render_tsv(ii::GENERATOR);
            println!(
                "{}.pkg entry {}: {} elements, sha256(entry payload) = {}",
                ii::source().package,
                ii::source().entry,
                t.count,
                t.source_sha256()
            );
            if *dry_run {
                println!("--dry-run: {} bytes NOT written to {}", text.len(), out.display());
                return Ok(ExitCode::SUCCESS);
            }
            // ⭐ Write through a temporary and only rename after the CONSUMER has accepted it. A
            // generator that leaves an unloadable table on disk has replaced a working file with a
            // broken one, and the failure surfaces in whatever runs next instead of here.
            let tmp = out.with_extension("tsv.new");
            std::fs::write(&tmp, &text)?;
            match ii::IndexTable::load(&tmp) {
                Ok(loaded) => {
                    std::fs::rename(&tmp, out)?;
                    println!("wrote {}: {} rows, accepted by IndexTable::load", out.display(), loaded.len());
                    Ok(ExitCode::SUCCESS)
                }
                Err(e) => {
                    let _ = std::fs::remove_file(&tmp);
                    println!("*** the rendered table does NOT load: {e}");
                    println!("*** {} left untouched", out.display());
                    Ok(ExitCode::from(2))
                }
            }
        }

        ItemIndexCmd::Diff { against } => {
            let against = &resolve_repo_path(against);
            let mut c = d1_oracle::oodle::Content::open_default()?;
            let t = ii::SourceTable::read(&mut c)?;
            let theirs = std::fs::read_to_string(against)?;
            // Render with THEIR generator line, so the comparison is of the measurement rather than
            // of the byline; the byline difference is then reported separately and explicitly.
            let their_gen = theirs
                .lines()
                .find_map(|l| l.strip_prefix("# GENERATED by "))
                .map(|s| s.trim_end_matches(" - do not hand-edit.").to_string())
                .unwrap_or_default();
            let ours_as_theirs = t.render_tsv(&their_gen);
            let ours = t.render_tsv(ii::GENERATOR);

            let same = ours_as_theirs == theirs;
            println!("golden diff: {} ({} bytes) vs this build ({} bytes)",
                     against.display(), theirs.len(), ours.len());
            println!("  their generator line : # GENERATED by {their_gen} - do not hand-edit.");
            println!("  ours                 : # GENERATED by {} - do not hand-edit.", ii::GENERATOR);
            println!("  sha256 theirs        : {}", d1_pkg::manifest::sha256_hex(theirs.as_bytes()));
            println!("  sha256 ours          : {}", d1_pkg::manifest::sha256_hex(ours.as_bytes()));
            println!("  sha256 ours, rendered with THEIR generator line: {}",
                     d1_pkg::manifest::sha256_hex(ours_as_theirs.as_bytes()));
            if same {
                println!(
                    "\nBYTE-IDENTICAL apart from the generator byline: {} lines, {} bytes.",
                    theirs.lines().count(),
                    theirs.len()
                );
                return Ok(ExitCode::SUCCESS);
            }
            let mut shown = 0;
            for (n, (a, b)) in ours_as_theirs.lines().zip(theirs.lines()).enumerate() {
                if a != b && shown < 10 {
                    println!("  line {}:\n    ours   {a}\n    theirs {b}", n + 1);
                    shown += 1;
                }
            }
            let (no, nt) = (ours_as_theirs.lines().count(), theirs.lines().count());
            if no != nt {
                println!("  line counts differ: ours {no}, theirs {nt}");
            }
            println!("\n*** THE TABLES DIFFER - one of the two generators is wrong ***");
            Ok(ExitCode::from(2))
        }
    }
}

fn run_art(_cli: &Cli, cmd: &ArtCmd) -> anyhow::Result<ExitCode> {
    use d1_oracle::art_arrangement as art;
    use d1_oracle::item_index as ii;
    match cmd {
        ArtCmd::Controls => {
            Ok(report_controls(&art::controls(), "ALL ART-ARRANGEMENT CONTROLS PASS"))
        }

        ArtCmd::Show { indices } => {
            let table = ii::IndexTable::load_default()?;
            let mut c = d1_oracle::oodle::Content::open_default()?;
            for ci in indices {
                let d = art::resolve(&mut c, *ci as u16, &table)?;
                println!(
                    "ci {:5}  hash {:#010x}  tag {:#010x}  equip_slot {:?}  gear {}  entries {:?}  \
                     per-class {:?}  curve {:?}  stats {:?}",
                    d.content_index, d.item_hash, d.world_tag, d.equip_slot,
                    d.gear.map(|g| format!("@+{g:#x}")).unwrap_or_else(|| "NULL".into()),
                    d.entries, d.per_class, d.curve_offset, d.stats
                );
            }
            Ok(ExitCode::SUCCESS)
        }

        ArtCmd::Write { out, inventory, dry_run } => {
            let out = out.clone().unwrap_or_else(|| PathBuf::from(ii::source().art_path));
            let (out, inventory) = (&resolve_repo_path(&out), &resolve_repo_path(inventory));
            let table = ii::IndexTable::load_default()?;
            let sha16 = art::index_tsv_sha16(&table.path)?;
            let mut c = d1_oracle::oodle::Content::open_default()?;
            let rows = art::staged_rows(&mut c, &table, inventory, true)?;
            let text = art::render_tsv(&rows, &sha16, art::GENERATOR);
            if *dry_run {
                println!("\n--dry-run: {} rows, {} bytes NOT written to {}",
                         rows.len(), text.len(), out.display());
                return Ok(ExitCode::SUCCESS);
            }
            let tmp = out.with_extension("tsv.new");
            std::fs::write(&tmp, &text)?;
            match ii::ArtTable::load(&tmp) {
                Ok(loaded) => {
                    std::fs::rename(&tmp, out)?;
                    println!("\nwrote {}: {} rows ({} measured (index, class) pairs), accepted by \
                              ArtTable::load", out.display(), rows.len(), loaded.len());
                    Ok(ExitCode::SUCCESS)
                }
                Err(e) => {
                    let _ = std::fs::remove_file(&tmp);
                    println!("\n*** the rendered table does NOT load: {e}");
                    println!("*** {} left untouched", out.display());
                    Ok(ExitCode::from(2))
                }
            }
        }

        ArtCmd::Diff { against, inventory } => {
            let (against, inventory) = (&resolve_repo_path(against), &resolve_repo_path(inventory));
            let table = ii::IndexTable::load_default()?;
            let sha16 = art::index_tsv_sha16(&table.path)?;
            let mut c = d1_oracle::oodle::Content::open_default()?;
            let rows = art::staged_rows(&mut c, &table, inventory, false)?;
            let theirs = std::fs::read_to_string(against)?;
            let their_gen = theirs
                .lines()
                .find_map(|l| l.strip_prefix("# GENERATED by "))
                .map(|s| s.trim_end_matches(" - do not hand-edit.").to_string())
                .unwrap_or_default();
            let ours_as_theirs = art::render_tsv(&rows, &sha16, &their_gen);
            let ours = art::render_tsv(&rows, &sha16, art::GENERATOR);
            println!("golden diff: {} ({} bytes) vs this build ({} bytes)",
                     against.display(), theirs.len(), ours.len());
            println!("  their generator line : # GENERATED by {their_gen} - do not hand-edit.");
            println!("  ours                 : # GENERATED by {} - do not hand-edit.", art::GENERATOR);
            println!("  sha256 theirs        : {}", d1_pkg::manifest::sha256_hex(theirs.as_bytes()));
            println!("  sha256 ours          : {}", d1_pkg::manifest::sha256_hex(ours.as_bytes()));
            println!("  sha256 ours, rendered with THEIR generator line: {}",
                     d1_pkg::manifest::sha256_hex(ours_as_theirs.as_bytes()));
            if ours_as_theirs == theirs {
                println!("\nBYTE-IDENTICAL apart from the generator byline: {} lines, {} bytes.",
                         theirs.lines().count(), theirs.len());
                return Ok(ExitCode::SUCCESS);
            }
            let mut shown = 0;
            for (n, (a, b)) in ours_as_theirs.lines().zip(theirs.lines()).enumerate() {
                if a != b && shown < 10 {
                    println!("  line {}:\n    ours   {a}\n    theirs {b}", n + 1);
                    shown += 1;
                }
            }
            let (no_, nt) = (ours_as_theirs.lines().count(), theirs.lines().count());
            if no_ != nt {
                println!("  line counts differ: ours {no_}, theirs {nt}");
            }
            println!("\n*** THE TABLES DIFFER - one of the two generators is wrong ***");
            Ok(ExitCode::from(2))
        }
    }
}

fn run_inventory(cli: &Cli, cmd: &InventoryCmd) -> anyhow::Result<ExitCode> {
    use d1_oracle::inventory as inv;
    match cmd {
        InventoryCmd::Layout => {
            let image = d1_eboot::image::Image::open_default()?;
            let idx = d1_eboot::descriptors::index(&image)?;
            let l = inv::derive_layout(&image, &idx)?;
            if cli.json {
                println!("{}", serde_json::json!({
                    "tag": format!("{:#010x}", inv::TAG), "struct_size": l.struct_size,
                    "level": l.level, "equip_min_stat": l.equip_min_stat,
                    "content_index": l.content_index, "extra_indices": l.extra_indices,
                    "extra_index_count": l.extra_index_count,
                    "progression_ordinal": l.progression_ordinal, "talent_grid": l.talent_grid,
                    "stat_keys": l.stat_keys, "stat_values": l.stat_values,
                }));
            } else {
                println!("inventory element {:#010x}  struct {} bytes", inv::TAG, l.struct_size);
                println!("  +{:#05x}  SOID (registry key)                       0x56CA70", l.soid);
                println!("  +{:#05x}  ⭐ item LEVEL (i32)                        0x26F492", l.level);
                println!("  +{:#05x}  ⭐ equip MINIMUM STAT (u32)                0x269971  ⛔ the PADLOCK",
                         l.equip_min_stat);
                println!("  +{:#05x}  stat-curve index offset (i16)             0x2816F0", l.curve_offset);
                println!("  +{:#05x}  ⭐⭐ CONTENT INDEX (u16) -> the DEFINITION  0x25ADE0 (vtbl+0x410)",
                         l.content_index);
                println!("  +{:#05x}  extra content indices  {} x u16           0x555730 (fixed 8-iteration loop)",
                         l.extra_indices, l.extra_index_count);
                println!("  +{:#05x}  progression XP (u32)                      0x284630", l.progress_points);
                println!("  +{:#05x}  progression ordinal (u16, 0xFFFF = none)  0x25B200 (vtbl+0x478)",
                         l.progression_ordinal);
                println!("  +{:#05x}  talent grid (u16, 0xFFFF = none)          0x25B960 (vtbl+0x4E0)",
                         l.talent_grid);
                println!("  +{:#05x}  stat keys    {} x u16                      0x28B6E0",
                         l.stat_keys, l.stat_key_count);
                println!("  +{:#05x}  stat values  {} x u32                      0x28CC70",
                         l.stat_values, l.stat_value_count);
                println!("\n⭐ THE TAB IS POSITIONAL - FUN_0026DB60(scope, slot) maps a SLOT INDEX to a \
                          bucket.\n   The item is not an input. All 39 records, measured twice \
                          (capture-rig/inventory-buckets.tsv):");
                for r in inv::BUCKET_TABLE {
                    let last = r.first_slot + r.slot_count - 1;
                    let name = inv::bucket_name(r.id).unwrap_or("-");
                    let note = if r.id == inv::MATERIALS_ID {
                        "  ⛔ everything we shipped landed here"
                    } else {
                        ""
                    };
                    println!("     id {:#04x}  scope {}  slots {:>3}..{:<3}  {:16}{note}",
                             r.id, r.scope, r.first_slot, last, name);
                }
            }
            Ok(ExitCode::SUCCESS)
        }

        InventoryCmd::Controls => {
            let table = d1_oracle::item_index::IndexTable::load_default().ok();
            let cs = inv::controls(table.as_ref());
            let mut ok = true;
            for c in &cs {
                if !c.pass {
                    ok = false;
                }
                println!("{}  {:38} {}", if c.pass { "PASS" } else { "FAIL" }, c.name, c.detail);
            }
            println!("\n{}", if ok {
                "ALL INVENTORY-ELEMENT CONTROLS PASS"
            } else {
                "*** A CONTROL FAILED - nothing this tool produced may be cited ***"
            });
            Ok(if ok { ExitCode::SUCCESS } else { ExitCode::from(2) })
        }

        InventoryCmd::Rebuild { dir, out_dir, level, equip_min, legacy, dry_run, from_index } => {
            let table = d1_oracle::item_index::IndexTable::load_default()?;
            // ci -> hash of the build the staged elements came from (plain TSV read: its
            // provenance is another build's, so the selected-build loader would refuse it).
            let from: Option<std::collections::HashMap<u16, u32>> = match from_index {
                Some(p) => {
                    let text = std::fs::read_to_string(resolve_repo_path(std::path::Path::new(p)))?;
                    let mut m = std::collections::HashMap::new();
                    for l in text.lines().filter(|l| l.starts_with("0x")) {
                        let f: Vec<&str> = l.split_whitespace().collect();
                        let h = u32::from_str_radix(f[0].trim_start_matches("0x"), 16)?;
                        let i: u16 = f[1].parse()?;
                        m.insert(i, h);
                    }
                    println!("--from-index {p}: {} items", m.len());
                    Some(m)
                }
                None => None,
            };
            // The art table also carries the def-derived +0x20 curve offset and the def+0x68 stat
            // list (both what the client's own constructor FUN_0026E0E0 would write). Optional:
            // without it every element keeps its defaults, exactly as before.
            let art = match d1_oracle::item_index::ArtTable::load_default() {
                Ok(t) => Some(t),
                Err(e) => {
                    eprintln!("⚠ no art-arrangement table - +0x20 stays 0 and +0x88/+0x98 stay \
                               empty on every element. {e}");
                    None
                }
            };
            let src = std::path::Path::new(dir);
            let staged = inv::read_staged(src)?;
            if staged.is_empty() {
                anyhow::bail!("{dir}: no *.bin elements to rebuild");
            }
            let dst = out_dir.clone().unwrap_or_else(|| dir.clone());
            let mut oracle = ObjectOracle::new(inv::TAG)?;
            let mut wrote = 0usize;
            let mut placements: Vec<(String, u64, u16, Option<u16>)> = Vec::new();

            println!("rebuilding {} element(s) from {dir}", staged.len());
            for (n, s) in staged.iter().enumerate() {
                // ⛔⛔ IDEMPOTENCE, and it is not optional: `d1-responder::stage_loadout` calls
                // this after every staging, so a second run must not explode. `+0x18` is the item
                // hash only on a LEGACY element - a rebuilt one carries a LEVEL, and feeding `40`
                // to `require()` fails with "item hash 0x00000028 is not in the table", which
                // reads like a broken content table and is the tool eating its own output.
                // ⭐ So: if the element already names a real content index, believe THAT and take
                // the hash back out of the table; only a legacy element (index 1 = 'Bronze Tier')
                // is re-derived from `+0x18`.
                let rebuilt = s.content_index != inv::EMPTY_INDEX
                    && s.content_index != d1_oracle::character::LEGACY_BRONZE_TIER_INDEX;
                let hash = if rebuilt && from.is_some() {
                    *from.as_ref().unwrap().get(&s.content_index).ok_or_else(|| anyhow::anyhow!(
                        "{}: content index {} is not in --from-index", s.file.display(),
                        s.content_index))?
                } else if rebuilt {
                    table.hash_of(s.content_index).ok_or_else(|| anyhow::anyhow!(
                        "{}: element states content index {} but the table has no item at that \
                         index - the element and the content table disagree",
                        s.file.display(), s.content_index))?
                } else {
                    s.item_hash
                };
                let spec = if *legacy {
                    inv::ItemSpec::legacy_broken(s.soid, hash, s.equip_min_stat)
                } else {
                    let mut sp = inv::ItemSpec::resolved(s.soid, hash, &s.bucket, &table)?;
                    sp.level = *level;
                    sp.equip_min_stat = *equip_min;
                    if let Some(a) = art.as_ref() {
                        // ⭐ +0x20 and +0x88/+0x98, as FUN_0026E0E0 itself would set them.
                        // `None` = the table never measured this item: leave the default alone.
                        if let Some(c) = a.curve_offset_for(sp.content_index) {
                            sp.curve_offset = c;
                        }
                        if let Some(list) = a.stats_for(sp.content_index) {
                            sp.stats = list.to_vec();
                        }
                    }
                    sp
                };
                let b = inv::build(&mut oracle, &spec)?;
                let slot = inv::slot_for_bucket(&spec.bucket, 0);
                println!(
                    "  {:20} soid {:#018x}  hash {:#010x} -> content index {:<5}  \
                     +0x18 {:<4} +0x1C {:<4} +0x20 {:<3} stats {:<2}  slot {}",
                    spec.bucket, spec.soid, spec.item_hash, spec.content_index, spec.level,
                    spec.equip_min_stat, spec.curve_offset, spec.stats.len(),
                    slot.map(|x| x.to_string()).unwrap_or_else(|| "- (bucket range UNREAD)".into())
                );
                placements.push((spec.bucket.clone(), spec.soid, spec.content_index, slot));
                if !*dry_run {
                    std::fs::create_dir_all(&dst)?;
                    let name = s.file.file_name().unwrap().to_string_lossy().to_string();
                    let _ = n;
                    std::fs::write(format!("{dst}/{name}"), &b.payload)?;
                    wrote += 1;
                }
            }
            let unplaced: Vec<&str> = placements.iter().filter(|p| p.3.is_none())
                .map(|p| p.0.as_str()).collect();
            if !unplaced.is_empty() {
                println!("\n⚠ {} item(s) have no READ bucket range and are therefore left OUT of the \
                          profile's 340-slot backpack array: {unplaced:?}\n  They still exist in \
                          family 4/3 and are still named by the profile's 20-entry equipped list.\n  \
                          Slots 133-142 are the unread gap; do not invent a range for them.",
                         unplaced.len());
            }
            if !*dry_run {
                println!("\nwrote {wrote} element(s) to {dst}\n  D1_INVENTORY={dst}");
                println!("  ⭐ NOW REBUILD THE PROFILE - the 340-slot placement lives there:\n\
                          \x20    d1-oracle profile build --inventory {dst} …");
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn print_controls() -> bool {
    let mut ok = true;
    match control::object_control() {
        Ok(c) => println!(
            "CONTROL object   tag {:#010x}  vector {}\n  soid {:#018x}  identity {}  bits {}  PASS",
            c.tag,
            c.vector,
            c.soid,
            hexs(&c.identity),
            c.bits
        ),
        Err(e) => {
            ok = false;
            println!("CONTROL object   *** FAIL - nothing below is trustworthy ***\n  {e}");
        }
    }
    match control::message_control() {
        Ok(c) => println!(
            "CONTROL message  503 {}\n                 206 {}  PASS",
            c.wire_503, c.wire_206
        ),
        Err(e) => {
            ok = false;
            println!("CONTROL message  *** FAIL - nothing below is trustworthy ***\n  {e}");
        }
    }
    ok
}

fn run(cli: &Cli) -> anyhow::Result<ExitCode> {
    match &cli.cmd {
        Cmd::Control => Ok(if print_controls() { ExitCode::SUCCESS } else { ExitCode::from(2) }),

        Cmd::Vectors => {
            if !print_controls() {
                return Ok(ExitCode::from(2));
            }
            println!();
            let mut ok = true;
            for (name, tag, payload, bits, nz) in pinned_decodes() {
                let mut o = ObjectOracle::new(tag)?;
                let d = o.decode(&hex(payload)?)?;
                let good = d.bits == bits && d.require_nonzero_qwords(&nz).is_ok();
                ok &= good;
                println!(
                    "{name:<22} {tag:#010x}  {:5} bits  {:6}B object  nz {:?}  {}",
                    d.bits,
                    d.object.len(),
                    d.nonzero_offsets(),
                    if good { "OK" } else { "*** DRIFT ***" }
                );
            }
            // Encode direction: the roster vector, rebuilt from the C object.
            let mut ros = ObjectOracle::new(0x8080_1951)?;
            let mut body = vec![0u8; ros.struct_size()];
            body[0x00..0x08].copy_from_slice(&ACCOUNT_SOID.to_le_bytes());
            body[0x08..0x0C].copy_from_slice(&1u32.to_le_bytes());
            body[0x10..0x18].copy_from_slice(&CHARACTER_SOID.to_le_bytes());
            let (enc, _) = ros.round_trip(&body, 0x9000)?;
            let want = format!("e880000000000000c74400000000000407{}{}", "0001".repeat(27), "8000".repeat(5));
            let good = hexs(&enc.payload) == want;
            ok &= good;
            println!(
                "roster FULL            {:#010x}  {:5} bits  {:6}B wire   {}",
                enc.tag,
                enc.bits,
                enc.payload.len(),
                if good { "OK" } else { "*** DRIFT ***" }
            );
            // Roster MINIMAL, built from the bit grammar rather than from the encoder.
            let mini = pack_bits(&format!(
                "1{:064b}1{:04b}{:064b}00",
                ACCOUNT_SOID, 1, CHARACTER_SOID
            ));
            let d = ros.decode(&mini)?;
            let good = hexs(&mini) == "e880000000000000c74400000000000404"
                && d.bits == 136
                && d.qword(0) == ACCOUNT_SOID
                && d.qword(0x10) == CHARACTER_SOID;
            ok &= good;
            println!(
                "roster MINIMAL         {:#010x}  {:5} bits  {:6}B wire   {}",
                d.tag,
                d.bits,
                mini.len(),
                if good { "OK" } else { "*** DRIFT ***" }
            );
            let mut pro = ObjectOracle::new(0x8080_1743)?;
            let pmini = pack_bits(&format!("1{PROFILE_SOID:064b}{}", "0".repeat(17)));
            let d = pro.decode(&pmini)?;
            let good = hexs(&pmini) == "e880000000000100800000"
                && d.bits == 82
                && d.qword(0) == PROFILE_SOID
                && d.nonzero_offsets() == vec![0];
            ok &= good;
            println!(
                "profile MINIMAL        {:#010x}  {:5} bits  {:6}B wire   {}",
                d.tag,
                d.bits,
                pmini.len(),
                if good { "OK" } else { "*** DRIFT ***" }
            );
            Ok(if ok { ExitCode::SUCCESS } else { ExitCode::from(2) })
        }

        Cmd::Object { tag, decode, obj, value_type, roster_key, soid, capacity } => {
            let tag = *tag as u32;
            let types: Vec<u32> = value_type.iter().map(|&t| t as u32).collect();
            let mut o = ObjectOracle::with_types(tag, &types)?;
            println!("tag {tag:#010x}  struct {} (={:#x}) bytes", o.struct_size(), o.struct_size());
            if let Some(h) = decode {
                let d = o.decode(&hex(h)?)?;
                if cli.json {
                    println!("{}", serde_json::json!({
                        "tag": tag, "bits": d.bits, "struct_size": d.object.len(),
                        "nonzero": d.nonzero_qwords().iter()
                            .map(|(k, v)| (format!("{k:#x}"), format!("{v:#x}")))
                            .collect::<Vec<_>>(),
                    }));
                } else {
                    println!("decoded {} bits -> {}-byte object", d.bits, d.object.len());
                    for (off, v) in d.nonzero_qwords() {
                        println!("  +{off:#06x}  {v:#018x}");
                    }
                }
                return Ok(ExitCode::SUCCESS);
            }
            let mut body = vec![0u8; o.struct_size()];
            if let Some(h) = obj {
                let raw = hex(h)?;
                let n = raw.len().min(body.len());
                body[..n].copy_from_slice(&raw[..n]);
            }
            if let Some(k) = roster_key {
                body[0x00..0x08].copy_from_slice(&k.to_le_bytes());
                body[0x08..0x0C].copy_from_slice(&(soid.len() as u32).to_le_bytes());
                for (i, s) in soid.iter().enumerate() {
                    let at = 0x10 + i * 8;
                    body[at..at + 8].copy_from_slice(&s.to_le_bytes());
                }
            }
            let (enc, dec) = o.round_trip(&body, *capacity)?;
            println!("payload {} bytes / {} bits\n  {}", enc.payload.len(), enc.bits, hexs(&enc.payload));
            println!("round trip: {} bits consumed, object IDENTICAL", dec.bits);
            Ok(ExitCode::SUCCESS)
        }

        Cmd::ActivityHost(a) => run_activity_host(cli, a),

        Cmd::Sensor(s) => run_sensor(cli, s),

        Cmd::Profile(p) => run_profile(cli, p),

        Cmd::Character(c) => run_character(cli, c),

        Cmd::Roster(r) => run_roster(cli, r),
        Cmd::Inventory(i) => run_inventory(cli, i),
        Cmd::Oodle(o) => run_oodle(cli, o),
        Cmd::ItemIndex(i) => run_item_index(cli, i),
        Cmd::ArtArrangement(a) => run_art(cli, a),
        Cmd::ActivityModes(a) => run_activity_modes(cli, a),

        Cmd::Message { tag, msgid, request_tag, obj, span } => {
            let tag = *tag as u32;
            let mut o = MessageOracle::new(tag, *msgid as u16, *request_tag as u32)?;
            let objb = obj.as_deref().map(hex).transpose()?.unwrap_or_default();
            let spanb = span.as_deref().map(hex).transpose()?.unwrap_or_default();
            let (h, _) = o.make_handle(&objb, &spanb)?;
            let w = o.encode(h, 0x20000)?;
            println!("wire  {}B  {}", w.len(), hexs(&w));
            let d = o.decode(&w, 0x20000)?;
            println!(
                "decode rc={} alloc={:#x} span_count={} span={}",
                d.rc,
                d.alloc,
                d.span_count,
                hexs(&d.span)
            );
            Ok(ExitCode::SUCCESS)
        }
    }
}

/// The pinned decode vectors, shared with `tests/vectors.rs`'s intent (kept here for the CLI).
fn pinned_decodes() -> Vec<(&'static str, u32, &'static str, u32, Vec<usize>)> {
    vec![
        ("character (CONTROL)", 0x8080_1954, control::CONTROL_VECTOR, 97, vec![0x00, 0x08]),
        (
            "account",
            0x8080_1803,
            "e880000000000000c74400000000000405d100000000000201000000",
            221,
            vec![0x00, 0x08, 0x10, 0x68],
        ),
        ("family 4/type 4", 0x8080_102E, "e88000000000000080", 67, vec![0x00]),
    ]
}
