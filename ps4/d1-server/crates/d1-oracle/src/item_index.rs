// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! ⭐ The **item content index** - the `u16` this client keys every item by.
//!
//! # What this module is
//!
//! Both halves, since 2026-08-11: the **consumer** ([`IndexTable`], [`ArtTable`]) and the
//! **measurement** ([`SourceTable`], and [`crate::art_arrangement`] for the art table). It used to
//! be the consumer only, with the measurement in `capture-rig/oodle/item-index.py`, because reading
//! the table needs the client's own Oodle decoder under Unicorn - which [`crate::oodle`] now drives
//! from Rust. **No Python is on the runtime path of any shipped binary.**
//!
//! ```text
//! d1-oracle item-index controls                       # the shipped table's pins, exit 2 on failure
//! d1-oracle item-index write                          # regenerate it from the packages
//! d1-oracle item-index diff --against <a python one>  # the executed golden diff
//! ```
//!
//! ⚠ The consumer still refuses a table whose provenance header does not say what it must say, and
//! that check is unchanged: a regenerated table that moved an index fails **here** rather than
//! silently re-skinning every fixture.
//!
//! # Where the number comes from  `[C]` content · `[O]` execution
//!
//! `ps4_investment_globals_012f_3.pkg` entry **8126** (tag `0x80A5FFBE`, class `0x80801933`) is a
//! **7,544 × 24 B** array of `{u32 item content hash, u32 0, u64 0, u32 world tag, u32 0}`
//! (element class `0x808016BD`). **The content index is the element index**, and that is not an
//! inference: it is the argument of the client's own definition lookup
//!
//! ```text
//! 0x25ADE0  = investment_globals->vtbl[0x410](u16 idx)      // 0x265EF0's gear loop calls it with
//!                                                           // the inventory element's +0x24
//!    rax = this->vtbl[0x168]()                              // resolve the table's content tag
//!    if ((i32)D[0] <= (i16)idx) return 0                    // the bound == 7544
//!    rax = D + D[8] + 0x18 ; rcx = idx*3 ; rdx = rcx*8      // ⭐ 24-byte stride
//!    ebx = [rax + rcx*8 + 0x10]                             // element+0x10 = the item's world tag
//! ```
//!
//! Replayed on `runs/snapshots/orbit-all.snap`, `0x25ADE0(globals, 6299)` computes element address
//! `0x3809889A8`, and the `u32` there is `0xD471D331` - Gjallarhorn. The 181,056 element bytes at
//! the base the client computes are **byte-identical** to the package entry.
//!
//! # ⛔ THE ROUTE IN `THE-EMPTY-PROFILE.md` §8.5 IS REFUTED - do not use it
//!
//! *"content index `N` → world tag `0x80A60297 + N`, so `N = tag - 0x80A60297`"* holds for the
//! first **112** indices and then stops. `0x54C090`'s table has **633 discontinuities** and only
//! 6,750 distinct tags for 7,544 indices. The live-measured tag `0x80A6035A` is index **197**, not
//! the 195 that route gives. Executed both ways in `item-index.py --controls`
//! (`world-tag-anchor`), where the wrong answer is the discriminator.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::error::{OracleError, Result};

/// `0xFFFF` is *absent*. `0x0000` is a real item (`0x1056640: cmp word[rsi],0xFFFF; setne`).
pub const EMPTY_CONTENT_INDEX: u16 = 0xFFFF;

/// What the shipped table must say about itself, or this module refuses it.
pub const SOURCE_PACKAGE: &str = "ps4_investment_globals_012f_3";
pub const SOURCE_ENTRY: u32 = 8126;
pub const ELEMENT_CLASS: u32 = 0x8080_16BD;
pub const COUNT: usize = 7544;

/// The default location of the generated table, relative to the repo root.
pub const DEFAULT_PATH: &str = "capture-rig/item-content-index.tsv";

/// Where one build's item table comes from and where its generated TSVs live.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Source {
    pub package: &'static str,
    pub entry: u32,
    pub count: usize,
    pub path: &'static str,
    /// The art-arrangement TSV for this build.
    pub art_path: &'static str,
    /// `(content index, arrangement)` the art table must reproduce for class 1 - see
    /// [`PINNED_ARRANGEMENT`]. Content indices differ per build, so this is per build.
    pub art_pin: (u16, u16),
    /// `(content index, curve offset)` - see [`PINNED_CURVE_OFFSET`]. `None` when no staged item of
    /// this build carries a non-zero curve to discriminate with.
    pub curve_pin: Option<(u16, i16)>,
}

/// 01.29: the pinned constants above, unchanged.
pub const SOURCE_0129: Source = Source {
    package: SOURCE_PACKAGE,
    entry: SOURCE_ENTRY,
    count: COUNT,
    path: DEFAULT_PATH,
    art_path: ART_DEFAULT_PATH,
    art_pin: PINNED_ARRANGEMENT,
    curve_pin: Some(PINNED_CURVE_OFFSET),
};

/// Build 56578 (package 01.33). The same entry (tag `0x80A5FFBE`, class `0x80801933`, index 8126)
/// is in every generation `012f_2`..`012f_6`; the client loads the highest, `012f_6` (206,664
/// bytes = 0x30 header + 8,609 x 24). Generations `0130`-`0133` do not carry it. Measured 13 Oct
/// with `d1-pkg entries` over the 56578 estate.
pub const SOURCE_56578: Source = Source {
    package: "ps4_investment_globals_012f_6",
    entry: 8126,
    count: 8609,
    path: "capture-rig/builds/later-56578/item-content-index.tsv",
    art_path: "capture-rig/builds/later-56578/item-art-arrangement.tsv",
    // The 01.29 anchors, re-found by item HASH on 56578 (13 Oct, `art-arrangement write` over the
    // 012f_6 definitions, identity-checked at def+0x78): Gjallarhorn 0xD471D331 moved ci 6299 ->
    // 6323 and still names arrangement 1229; Oryx Mask 0x74CB6B9B moved ci 6488 -> 6514 and still
    // carries curve offset 70. Every staged item's equip ordinal matches the 01.29 table.
    art_pin: (6323, 1229),
    curve_pin: Some((6514, 70)),
};

/// The selected build's source (`$D1_BUILD`, else the registry default).
pub fn source() -> &'static Source {
    match d1_build::selected().id.as_str() {
        "later-56578" => &SOURCE_56578,
        _ => &SOURCE_0129,
    }
}

/// ⭐ The public-manifest catalogue, relative to the repo root - the **positive control** for
/// [`controls`]'s hash-space check.
///
/// ⛔ RESTORED 2026-08-11 by the adversarial verify of the Python port. `item-index.py --controls`
/// asserted `not present and hits > 4000`: the three non-item hashes must be ABSENT *and* four
/// thousand-odd real item rows must RESOLVE. The Rust port carried only the negative half, leaving
/// a scan with no positive control in the same window - a table that resolved nothing at all would
/// have passed it. The Python failed when the catalogue was missing (`hits` stayed 0), and so does
/// this: a control that cannot run is not a measurement.
pub const CATALOGUE_PATH: &str = "catalogue/d1-catalogue.json";

/// The floor `item-index.py --controls` used, kept verbatim.
pub const CATALOGUE_MIN_HITS: usize = 4000;

/// ⭐ The perturbation the catalogue control inverts with, and **why it is not `+1`**.
///
/// Measured 2026-08-11: the item hash space is **not uniform**. 2,592 of the 7,544 table hashes
/// have `h+1` also in the table, so perturbing the catalogue by one still resolves 2,143 of 5,293
/// rows (`+2` resolves 1,165). An off-by-one inversion over item hashes therefore proves nothing -
/// it looks like a broken table and is a real property of the content. A wide XOR resolves **0**,
/// which is what an inversion has to look like to be evidence.
pub const CATALOGUE_INVERSION_MASK: u32 = 0x5A5A_5A5A;

/// ⭐ The **emblem's equipment slot** = 12. `[O]`
///
/// `0xA11618 movsx rax, byte [rax+0x18]` where `rax = investment_globals->vtbl[0x160]()`; then
/// `0xA11623 shl rax,6` and `0xA11627 mov ax,[r13+rax+0x258]` - i.e. the card's equipment element
/// `slot`'s `+0x08` `u16`, handed to the emblem colour resolver `0xA81B00`. **Eight independent
/// call sites read that same `+0x18` byte** and every one feeds an emblem path (`0x9A44CB`,
/// `0x9A45CE`, `0x9BD9A9`, `0xA1160D`, `0xA2DB5F`, `0xA56713`, `0xA59E45`, `0xA71DF7`).
///
/// Replayed on `runs/snapshots/orbit-all.snap`: `0x25DA40(globals)` returns `0x380A2EDA0` and the
/// byte at `+0x18` is **`0x0C`**.
///
/// ⚠ The *other* slots' meanings are `[U]`. This module reserves slot 12 and otherwise keeps the
/// staged inventory's own file order, which is what every fixture before it did.
pub const EMBLEM_SLOT: usize = 12;

/// A real emblem: **The Reflective Proof**, `0x5508AC2A` → content index **1512**, world tag
/// `0x80A62EB7`. Chosen because it is D1's starter emblem and it is *present in 01.29 content*;
/// the index is measured, not chosen. `[C]`
pub const DEFAULT_EMBLEM_HASH: u32 = 0x5508_AC2A;

/// The measured table: item content hash → `{content index, world tag}`.
#[derive(Debug, Clone)]
pub struct IndexTable {
    pub path: PathBuf,
    pub source_package: String,
    pub source_entry: u32,
    pub element_class: u32,
    pub declared_count: usize,
    pub source_sha256: String,
    by_hash: BTreeMap<u32, (u16, u32)>,
}

fn hexnum(s: &str) -> Option<u64> {
    let s = s.trim();
    if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u64::from_str_radix(h, 16).ok()
    } else {
        s.parse::<u64>().ok()
    }
}

impl IndexTable {
    /// Load and validate the generated table. Every provenance field is checked, because a table
    /// with the wrong provenance is a table of numbers with no evidence behind them.
    pub fn load(path: &Path) -> Result<IndexTable> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| OracleError::Shape(format!("{}: {e}", path.display())))?;
        let mut t = IndexTable {
            path: path.to_path_buf(),
            source_package: String::new(),
            source_entry: 0,
            element_class: 0,
            declared_count: 0,
            source_sha256: String::new(),
            by_hash: BTreeMap::new(),
        };
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("# source: ") {
                let f: Vec<&str> = rest.split_whitespace().collect();
                if let Some(p) = f.first() {
                    t.source_package = p.trim_end_matches(".pkg").to_string();
                }
                if let Some(i) = f.iter().position(|x| *x == "entry") {
                    t.source_entry = f.get(i + 1).and_then(|s| hexnum(s)).unwrap_or(0) as u32;
                }
                continue;
            }
            if let Some(rest) = line.strip_prefix("# element class ") {
                t.element_class = hexnum(rest.split(',').next().unwrap_or("")).unwrap_or(0) as u32;
                if let Some(i) = rest.find("count ") {
                    t.declared_count = rest[i + 6..].trim().parse().unwrap_or(0);
                }
                continue;
            }
            if let Some(rest) = line.strip_prefix("# sha256(entry payload) = ") {
                t.source_sha256 = rest.trim().to_string();
                continue;
            }
            if line.starts_with('#') || line.starts_with("item_hash") || line.trim().is_empty() {
                continue;
            }
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() < 3 {
                return Err(OracleError::Shape(format!(
                    "{}: malformed row {line:?}", path.display())));
            }
            let (h, i, tag) = (
                hexnum(f[0]).ok_or_else(|| OracleError::Shape(format!("bad hash {:?}", f[0])))?,
                hexnum(f[1]).ok_or_else(|| OracleError::Shape(format!("bad index {:?}", f[1])))?,
                hexnum(f[2]).ok_or_else(|| OracleError::Shape(format!("bad tag {:?}", f[2])))?,
            );
            t.by_hash.insert(h as u32, (i as u16, tag as u32));
        }
        let src = source();
        if t.source_package != src.package
            || t.source_entry != src.entry
            || t.element_class != ELEMENT_CLASS
            || t.declared_count != src.count
            || t.source_sha256.len() != 64
        {
            return Err(OracleError::Shape(format!(
                "{}: provenance header says package {:?} entry {} element class {:#010x} count {} \
                 sha256 {:?}; this build requires {:?} / {} / {ELEMENT_CLASS:#010x} / {} and a \
                 64-hex-digit sha256. Regenerate with `d1-oracle item-index write`",
                path.display(), t.source_package, t.source_entry, t.element_class,
                t.declared_count, t.source_sha256, src.package, src.entry, src.count)));
        }
        if t.by_hash.len() != src.count {
            return Err(OracleError::Shape(format!(
                "{}: {} distinct hashes, expected {} - a duplicate makes an index ambiguous",
                path.display(), t.by_hash.len(), src.count)));
        }
        Ok(t)
    }

    /// Load from [`DEFAULT_PATH`], searched upward from the current directory so the CLI works from
    /// the repo root or from `d1-server/`.
    pub fn load_default() -> Result<IndexTable> {
        let mut dir = std::env::current_dir()
            .map_err(|e| OracleError::Shape(format!("cwd: {e}")))?;
        loop {
            let p = dir.join(source().path);
            if p.exists() {
                return IndexTable::load(&p);
            }
            if !dir.pop() {
                return Err(OracleError::Shape(format!(
                    "{} not found in this directory or any parent. Generate it from the \
                     packages: `d1-oracle item-index write` (needs the selected build's \
                     packages, or $D1_PKGDIR)", source().path)));
            }
        }
    }

    pub fn len(&self) -> usize {
        self.by_hash.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_hash.is_empty()
    }

    /// The `u16` the client keys this item by, or `None` if the hash is not an item in this content.
    pub fn index_of(&self, item_hash: u32) -> Option<u16> {
        self.by_hash.get(&item_hash).map(|(i, _)| *i)
    }

    /// The item's own world tag - what `0x54C090` returns and `0x555730` asks the resourcerer to
    /// load. Kept so a caller can say *which* content a claimed index actually names.
    pub fn world_tag_of(&self, item_hash: u32) -> Option<u32> {
        self.by_hash.get(&item_hash).map(|(_, t)| *t)
    }

    /// ⭐ The **reverse** lookup: which item hash a content index names.
    ///
    /// This exists because of a trap the crate walked straight into: `inventory rebuild` reads an
    /// element's `+0x18` as the item hash, which is true only of a *legacy* element - a rebuilt one
    /// carries a LEVEL there, so a second run fed `40` to [`Self::require`] and failed with
    /// *"item hash 0x00000028 is not in the table"*, which reads like a broken content table and is
    /// actually **the tool eating its own output** (`status/LESSONS.md` #4). With this, a rebuilt
    /// element is re-derived from the field the client actually reads.
    pub fn hash_of(&self, index: u16) -> Option<u32> {
        self.by_hash.iter().find(|(_, (i, _))| *i == index).map(|(h, _)| *h)
    }

    /// `(item hash, world tag)` for a content index - the direction the art-arrangement generator
    /// needs, since a definition is found by **world tag** and identity-checked by **hash**.
    pub fn by_index(&self, index: u16) -> Option<(u32, u32)> {
        self.by_hash.iter().find(|(_, (i, _))| *i == index).map(|(h, (_, t))| (*h, *t))
    }

    /// The index, or a refusal that names the hash. **Never a fabricated number**: a wrong index
    /// renders as the wrong gear and reads as success.
    pub fn require(&self, item_hash: u32) -> Result<u16> {
        self.index_of(item_hash).ok_or_else(|| OracleError::Shape(format!(
            "item hash {item_hash:#010x} is not in {} ({} rows). It is not an item in 01.29 \
             content - refusing to invent a content index",
            self.path.display(), self.by_hash.len())))
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// ⭐ THE GENERATOR - read the table out of the packages, in process, with no Python
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// The entry's own content tag, restated in the TSV header.
pub const SOURCE_ENTRY_TAG: u32 = 0x80A5_FFBE;
/// The entry's class tag - refused if the container disagrees.
pub const SOURCE_ENTRY_CLASS: u32 = 0x8080_1933;
/// The array header ahead of the elements.
pub const SOURCE_HEADER: usize = 0x30;
/// One element.
pub const ELEMENT_SIZE: usize = 24;
/// The `u32` at `+0x1C` of the array header on this table. Asserted, because it is the cheapest
/// evidence that the header layout is being read the way the client lays it out.
pub const SOURCE_MARK: u32 = 0x8080_0184;
/// `investment_globals->vtbl[0x410](u16 idx)` - the client's own definition lookup, whose argument
/// **is** the element index. Quoted in the TSV header so the file carries its own justification.
pub const DEF_LOOKUP: u32 = 0x0025_ADE0;

/// What this build writes into the `# GENERATED by` line.
pub const GENERATOR: &str = "d1-oracle item-index --write";

/// ⭐ Generators whose output [`IndexTable`]/[`ArtTable`] will accept. The Python entries stay
/// because the committed tables were written by them and are still valid measurements; what the
/// list refuses is a table with **no** generator line, i.e. a hand-written one.
pub const KNOWN_GENERATORS: [&str; 4] = [
    "capture-rig/oodle/item-index.py",
    "capture-rig/oodle/art-arrangement.py",
    "d1-oracle item-index",
    "d1-oracle art-arrangement",
];

/// The raw table as it sits in the package entry.
#[derive(Debug, Clone)]
pub struct SourceTable {
    /// The whole entry payload, which is what the `sha256` in the header covers.
    pub raw: Vec<u8>,
    /// `+0x00` - the entry's self-length. Equals `raw.len()`.
    pub total: u64,
    /// `+0x08` - the element count.
    pub count: usize,
    /// `+0x10` - always 16 on this table.
    pub sixteen: u64,
    /// `+0x1C`.
    pub mark: u32,
    /// `+0x20` - the count, again.
    pub count2: u64,
    /// `+0x28`.
    pub element_class: u32,
    /// `(item content hash, the middle qword, world tag)` per element, in element order.
    pub rows: Vec<(u32, u64, u32)>,
}

impl SourceTable {
    /// ⭐ Read the table out of the packages with the client's own Oodle decoder.
    ///
    /// Every refusal here is one of `item-index.py`'s, kept: a wrong class tag, a header that does
    /// not describe the payload it sits in front of, a count that disagrees with itself, or a
    /// duplicate hash (which would make an index ambiguous) all stop the tool rather than produce a
    /// table of plausible numbers.
    pub fn read(content: &mut crate::oodle::Content) -> Result<SourceTable> {
        let src = source();
        let (class_tag, raw) = content
            .entry_bytes(src.package, src.entry)
            .map_err(|e| OracleError::Shape(format!("{} entry {}: {e}", src.package, src.entry)))?;
        if class_tag != SOURCE_ENTRY_CLASS {
            return Err(OracleError::Shape(format!(
                "{} entry {} is class {class_tag:#010x}, not {SOURCE_ENTRY_CLASS:#010x} - that is \
                 a different structure, refusing to read it as the item table",
                src.package, src.entry
            )));
        }
        SourceTable::parse(raw)
    }

    /// Parse an already-extracted entry payload. Split out so the shape guards are testable without
    /// the owned-media dump.
    pub fn parse(raw: Vec<u8>) -> Result<SourceTable> {
        if raw.len() < SOURCE_HEADER {
            return Err(OracleError::Shape(format!(
                "the entry is {} bytes, shorter than its {SOURCE_HEADER}-byte array header",
                raw.len()
            )));
        }
        let u64_at = |o: usize| u64::from_le_bytes(raw[o..o + 8].try_into().unwrap());
        let u32_at = |o: usize| u32::from_le_bytes(raw[o..o + 4].try_into().unwrap());
        let total = u64_at(0x00);
        let count = u64_at(0x08) as usize;
        let sixteen = u64_at(0x10);
        let mark = u32_at(0x1C);
        let count2 = u64_at(0x20);
        let element_class = u32_at(0x28);

        if total as usize != raw.len() {
            return Err(OracleError::Shape(format!(
                "array header self-length {total} but the container handed over {} bytes - the \
                 entry did not span correctly",
                raw.len()
            )));
        }
        if count != count2 as usize {
            return Err(OracleError::Shape(format!(
                "the header states the count twice and disagrees with itself: {count} vs {count2}"
            )));
        }
        if raw.len() != SOURCE_HEADER + count * ELEMENT_SIZE {
            return Err(OracleError::Shape(format!(
                "{} bytes is not {SOURCE_HEADER} + {count}x{ELEMENT_SIZE} - the stride or the \
                 header size is wrong, and every index read out of it would be shifted",
                raw.len()
            )));
        }
        if sixteen != 16 || mark != SOURCE_MARK {
            return Err(OracleError::Shape(format!(
                "array header +0x10 = {sixteen} (want 16), +0x1C = {mark:#010x} (want \
                 {SOURCE_MARK:#010x}) - this is not the array header this tool was measured on"
            )));
        }
        if element_class != ELEMENT_CLASS {
            return Err(OracleError::Shape(format!(
                "element class {element_class:#010x}, want {ELEMENT_CLASS:#010x}"
            )));
        }

        let mut rows = Vec::with_capacity(count);
        let mut seen = std::collections::BTreeSet::new();
        let mut dups = 0usize;
        for i in 0..count {
            let o = SOURCE_HEADER + i * ELEMENT_SIZE;
            let hash = u32_at(o);
            let mid = u64_at(o + 8);
            let tag = u32_at(o + 0x10);
            if !seen.insert(hash) {
                dups += 1;
            }
            rows.push((hash, mid, tag));
        }
        if dups != 0 {
            return Err(OracleError::Shape(format!(
                "{dups} duplicate item hash(es) among {count} elements - a duplicate makes an index \
                 ambiguous and the consumer refuses the table anyway"
            )));
        }
        Ok(SourceTable { raw, total, count, sixteen, mark, count2, element_class, rows })
    }

    /// `sha256` of the entry payload - the provenance digest the header carries.
    pub fn source_sha256(&self) -> String {
        d1_pkg::manifest::sha256_hex(&self.raw)
    }

    /// ⭐ Render the TSV, byte for byte as it is shipped.
    ///
    /// ⚠ **One line differs from the Python's output on purpose**: `# GENERATED by`. Everything
    /// else - the other six header lines, the column header, and all 7,544 rows - is identical,
    /// which is what makes the diff between the two generators a one-line diff and therefore
    /// readable. Writing "GENERATED by …item-index.py" out of a Rust binary would be a lie in the
    /// one line whose entire job is provenance.
    pub fn render_tsv(&self, generator: &str) -> String {
        let mut s = String::with_capacity(self.count * 32 + 512);
        s.push_str("# \u{2b50} item content hash -> the u16 CONTENT INDEX the client keys items by.\n");
        s.push_str(&format!("# GENERATED by {generator} - do not hand-edit.\n"));
        s.push_str(&format!(
            "# source: {}.pkg entry {} tag {SOURCE_ENTRY_TAG:#010x} class \
             {SOURCE_ENTRY_CLASS:#010x}\n",
            source().package,
            source().entry
        ));
        s.push_str(&format!(
            "# element class {ELEMENT_CLASS:#010x}, {ELEMENT_SIZE} B, count {}\n",
            self.count
        ));
        s.push_str(&format!("# sha256(entry payload) = {}\n", self.source_sha256()));
        s.push_str("# the index is the ELEMENT INDEX, and it is the argument of the client's own\n");
        s.push_str("# definition lookup investment_globals->vtbl[0x410](u16); entry point is build-specific.\n");
        s.push_str("item_hash\tcontent_index\tworld_tag\n");
        for (i, (h, _mid, tag)) in self.rows.iter().enumerate() {
            s.push_str(&format!("{h:#010x}\t{i}\t{tag:#010x}\n"));
        }
        s
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// ⭐ The ART-ARRANGEMENT table - equipment element +0x0C, the u16 that names the 3D model
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// `0xFFFF` is "no arrangement": `FUN_0054B530` sign-extends its `short` argument, so `-1` fails
/// the `index < count` bound and returns 0 - the clean "no geometry" skip in `FUN_0054CF70`.
/// ⛔ **`0` is IN RANGE** and silently names arrangement #0 - which is what every card fixture
/// before 2026-08-10 shipped in all 20 slots.
pub const EMPTY_ARRANGEMENT: u16 = 0xFFFF;

/// The default location of the generated arrangement table, relative to the repo root.
pub const ART_DEFAULT_PATH: &str = "capture-rig/item-art-arrangement.tsv";

/// ⭐ The pinned anchor the generator writes and this loader re-checks when the row is present:
/// Gjallarhorn (content index 6299) → arrangement **1229**, from the wildcard-class entry of the
/// gear record at `def+0x60` of package entry `0x80A64196` (which carries Gjallarhorn's own hash
/// `0xD471D331` at `+0x78` - the identity check). `[C]` package bytes + transliterated walk.
pub const PINNED_ARRANGEMENT: (u16, u16) = (6299, 1229);

/// ⭐ The curve-offset anchor: Oryx Mask (content index 6488) carries **70** at `rec30+8` of its
/// `def+0x30` self-relative record - the u16 the client's own element constructor `FUN_0026E0E0`
/// stores at element `+0x20`, where `FUN_002816F0` samples the stat-group curve at `+0x20 + level`.
/// It is the ONE staged item whose value is nonzero, so it discriminates the extractor from both
/// "absent → 0" and a shifted base. `[C]` package bytes, measured 2026-08-11.
pub const PINNED_CURVE_OFFSET: (u16, i16) = (6488, 70);

/// The measured arrangement table: `(content index, class)` → arrangement index.
///
/// A **consumer** of [`crate::art_arrangement`]'s output (and of the Python generator that
/// preceded it), exactly as [`IndexTable`] is of [`SourceTable`]'s. The generator performs the
/// client's own `FUN_00289E80` walk over the definition's gear record (`def+0x60`, self-relative)
/// extracted from the packages, and identity-checks every entry against the item hash the entry
/// itself carries at `+0x78`.
#[derive(Debug, Clone)]
pub struct ArtTable {
    pub path: PathBuf,
    /// `(content_index, class 0|1|2)` → arrangement. [`EMPTY_ARRANGEMENT`] rows are stored too, so
    /// "measured as none" and "never measured" stay distinguishable.
    rows: BTreeMap<(u16, u8), u16>,
    /// ⭐ content_index → the item's **equipped-array ordinal** (`def_client+0x18`, u8; `0xFF` =
    /// none). `0x269080` derefs `character + 0x2B80 + slot*8` with THIS byte - the item's own
    /// definition names which of the 20 equipped/equipment entries it occupies, so filling those
    /// arrays in file order (what every fixture before 2026-08-10 did) makes the consumer look up
    /// the WRONG entry for every item whose ordinal differs from its file position.
    slots: BTreeMap<u16, u8>,
    /// ⭐ content_index → element `+0x20`'s **stat-curve index offset** - `*(u16*)(rec30+8)` of the
    /// definition's `def+0x30` record, exactly what the client's own constructor `FUN_0026E0E0`
    /// stores. Absent from the map = the (older) table never measured it.
    curve_offsets: BTreeMap<u16, i16>,
    /// ⭐ content_index → the definition's **stat list** at `def+0x68` - the `{u16 stat index,
    /// u32 value}` pairs `FUN_0028B6E0` copies into element `+0x88`/`+0x98`. `Some(vec![])` is
    /// **measured none** (a NULL or empty record - the constructor writes nothing either way);
    /// absent from the map is *never measured*. The two must stay distinguishable.
    stats: BTreeMap<u16, Vec<(u16, u32)>>,
    /// ⭐ content_index → the definition's **weapon object index** (`def_client+0x1C8`), the value
    /// equipment element `+0x0A` carries ([`crate::art_arrangement::weapon_object`]). Absent from
    /// the map = never measured (a table older than column 9).
    weapon_objects: BTreeMap<u16, u16>,
}

impl ArtTable {
    pub fn load(path: &Path) -> Result<ArtTable> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| OracleError::Shape(format!("{}: {e}", path.display())))?;
        // ⭐ WIDENED 2026-08-11. This used to be a literal match on
        // `"GENERATED by capture-rig/oodle/art-arrangement.py"`, which meant a Rust generator
        // writing an HONEST provenance line was rejected by the shipped binary - the project rule
        // ("a reusable tool is a Rust tool") enforced backwards. What the check is actually for is
        // unchanged: a table with **no recognised generator** is a table of guesses, and is still
        // refused. The Python byline stays valid because the committed table is still its output.
        if !KNOWN_GENERATORS.iter().any(|g| text.contains(&format!("GENERATED by {g}"))) {
            return Err(OracleError::Shape(format!(
                "{}: no recognised generator provenance line - a hand-written arrangement table is \
                 a table of guesses. Regenerate with `d1-oracle art-arrangement --staged --write` \
                 (known generators: {KNOWN_GENERATORS:?})",
                path.display())));
        }
        let mut rows = BTreeMap::new();
        let mut slots = BTreeMap::new();
        let mut curve_offsets = BTreeMap::new();
        let mut stats: BTreeMap<u16, Vec<(u16, u32)>> = BTreeMap::new();
        let mut weapon_objects = BTreeMap::new();
        for line in text.lines() {
            if line.starts_with('#') || line.starts_with("content_index") || line.trim().is_empty() {
                continue;
            }
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() < 6 {
                return Err(OracleError::Shape(format!(
                    "{}: malformed row {line:?} (want 6 columns incl. equip_slot - regenerate with \
                     art-arrangement.py)", path.display())));
            }
            let ci = hexnum(f[0])
                .ok_or_else(|| OracleError::Shape(format!("bad content index {:?}", f[0])))?;
            let cls = hexnum(f[3])
                .ok_or_else(|| OracleError::Shape(format!("bad class {:?}", f[3])))?;
            let arr = hexnum(f[4])
                .ok_or_else(|| OracleError::Shape(format!("bad arrangement {:?}", f[4])))?;
            let slot = hexnum(f[5])
                .ok_or_else(|| OracleError::Shape(format!("bad equip slot {:?}", f[5])))?;
            rows.insert((ci as u16, cls as u8), arr as u16);
            slots.insert(ci as u16, slot as u8);
            // Columns 7/8 (curve_offset, stats) exist from 2026-08-11 on; a 6-column table still
            // loads, with both left UNMEASURED - the consumers then change nothing.
            if let Some(c) = f.get(6) {
                let v = hexnum(c).ok_or_else(|| {
                    OracleError::Shape(format!("bad curve offset {c:?}"))
                })?;
                curve_offsets.insert(ci as u16, v as i16);
            }
            if let Some(s) = f.get(7) {
                let list = if s.trim() == "-" {
                    Vec::new()
                } else {
                    let mut out = Vec::new();
                    for pair in s.split(',') {
                        let (k, v) = pair.split_once(':').ok_or_else(|| OracleError::Shape(
                            format!("bad stat pair {pair:?} (want index:value)")))?;
                        out.push((
                            hexnum(k).ok_or_else(|| OracleError::Shape(
                                format!("bad stat index {k:?}")))? as u16,
                            hexnum(v).ok_or_else(|| OracleError::Shape(
                                format!("bad stat value {v:?}")))? as u32,
                        ));
                    }
                    out
                };
                stats.insert(ci as u16, list);
            }
            // Column 9 (weapon_object) exists from 2026-10-14 on; older tables leave it unmeasured.
            if let Some(w) = f.get(8) {
                let v = hexnum(w).ok_or_else(|| {
                    OracleError::Shape(format!("bad weapon object {w:?}"))
                })?;
                weapon_objects.insert(ci as u16, v as u16);
            }
        }
        let t = ArtTable {
            path: path.to_path_buf(),
            rows,
            slots,
            curve_offsets,
            stats,
            weapon_objects,
        };
        // ⭐ The equip-slot anchor: the emblem definition must name slot 12, the byte a SNAPSHOT
        // REPLAY of `investment_globals->vtbl[0x160]()[0x18]` independently returned (EMBLEM_SLOT).
        // Two instruments, one byte; a generator drift fails here, not on screen.
        if let Some(&s) = t.slots.get(&1512) {
            if s as usize != EMBLEM_SLOT {
                return Err(OracleError::Shape(format!(
                    "{}: emblem ci 1512 carries equip_slot {s}, the replayed emblem slot is \
                     {EMBLEM_SLOT} - the generator's def_client walk has drifted",
                    t.path.display())));
            }
        }
        // The anchor, when its row is present: a regenerated table that moved Gjallarhorn's model
        // fails here rather than silently re-skinning every fixture.
        let pin = source().art_pin;
        if let Some(a) = t.arrangement_for(pin.0, 1) {
            if a != pin.1 {
                return Err(OracleError::Shape(format!(
                    "{}: content index {} resolves arrangement {a}, the pinned measurement is {} - \
                     the table and the anchor disagree",
                    t.path.display(), pin.0, pin.1)));
            }
        }
        // The curve-offset anchor, when its column is present: Oryx Mask is the one staged item
        // whose value is nonzero, so a generator drift (shifted base, wrong record) fails here.
        if let Some(cp) = source().curve_pin {
            if let Some(&c) = t.curve_offsets.get(&cp.0) {
                if c != cp.1 {
                    return Err(OracleError::Shape(format!(
                        "{}: content index {} carries curve offset {c}, the pinned measurement is \
                         {} - the generator's def+0x30 walk has drifted",
                        t.path.display(), cp.0, cp.1)));
                }
            }
        }
        Ok(t)
    }

    /// Load from the selected build's art path, searched upward from the current directory.
    pub fn load_default() -> Result<ArtTable> {
        let mut dir = std::env::current_dir()
            .map_err(|e| OracleError::Shape(format!("cwd: {e}")))?;
        loop {
            let p = dir.join(source().art_path);
            if p.exists() {
                return ArtTable::load(&p);
            }
            if !dir.pop() {
                return Err(OracleError::Shape(format!(
                    "{} not found in this directory or any parent. Generate it from the \
                     packages: `d1-oracle art-arrangement write` (the selected build's packages, \
                     or $D1_PKGDIR)", source().art_path)));
            }
        }
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The arrangement for `(content index, class)`, or `None` when the item has no arrangement
    /// (measured [`EMPTY_ARRANGEMENT`]) **or** was never measured. Both answers mean *write
    /// `0xFFFF`* - the caller must never turn "unknown" into a number.
    pub fn arrangement_for(&self, content_index: u16, class: u8) -> Option<u16> {
        match self.rows.get(&(content_index, class)) {
            Some(&a) if a != EMPTY_ARRANGEMENT => Some(a),
            _ => None,
        }
    }

    /// Whether the table carries a measurement (even a "none") for this index at any class.
    pub fn measured(&self, content_index: u16) -> bool {
        (0u8..3).any(|c| self.rows.contains_key(&(content_index, c)))
    }

    /// ⭐ The item's **equipped-array ordinal** (`def_client+0x18`), or `None` when the definition
    /// carries none (`0xFF`) or was never measured. This is the index `0x269080` uses into the
    /// profile's `+0x2B80` equipped list, and the slot `FUN_0054CF70` iterates in the card's
    /// equipment summary - file order is NOT it.
    pub fn equip_slot_for(&self, content_index: u16) -> Option<usize> {
        match self.slots.get(&content_index) {
            Some(&s) if s != 0xFF => Some(s as usize),
            _ => None,
        }
    }

    /// ⭐ Element `+0x20`'s **stat-curve index offset** as the client's own constructor
    /// (`FUN_0026E0E0`) would set it: `*(u16*)(rec30+8)` of the definition's `def+0x30` record,
    /// 0 when the record is absent. `None` = the table never measured this item (an old,
    /// 6-column table) - the caller must then leave its default alone, never invent a value.
    pub fn curve_offset_for(&self, content_index: u16) -> Option<i16> {
        self.curve_offsets.get(&content_index).copied()
    }

    /// ⭐ The definition's **stat list** at `def+0x68` - what `FUN_0028B6E0` copies into element
    /// `+0x88`/`+0x98`. `Some(&[])` is **measured none** (NULL or empty record; the client's own
    /// constructor writes nothing into the element either way - so an empty element stat array is
    /// content-correct, not a defect). `None` = never measured.
    pub fn stats_for(&self, content_index: u16) -> Option<&[(u16, u32)]> {
        self.stats.get(&content_index).map(|v| v.as_slice())
    }

    /// ⭐ The definition's **weapon object index** (`def_client+0x1C8`) for equipment element
    /// `+0x0A`, or `None` when the definition has no client record (`0xFFFF`) or the table never
    /// measured it - the caller then leaves the element's `0` alone.
    pub fn weapon_object_for(&self, content_index: u16) -> Option<u16> {
        match self.weapon_objects.get(&content_index) {
            Some(&w) if w != 0xFFFF => Some(w),
            _ => None,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Controls
// ─────────────────────────────────────────────────────────────────────────────────────────────────

use crate::profile::Control;

/// ⭐ Hashes that must be **ABSENT**. They are a progression, a stat and an inventory *bucket* -
/// three hash spaces that are not the item space. If any of them resolved, the shipped table would
/// be a generic hash pool and every index in it would be meaningless.
pub const NOT_ITEMS: [(u32, &str); 3] = [
    (0x6650_C0F9, "progression character_level"),
    (0x8E8B_4E10, "stat Light"),
    (0xC3C9_6257, "bucket Titan Subclass"),
];

/// Items whose index is quoted in `historicaldocs/design/item-content-indices.md`. Each is re-derived here, so a
/// regenerated table that moved them fails rather than silently reindexing every fixture.
pub const PINNED: [(u32, u16, &str); 4] = [
    (0x925C_DEEA, 710, "Striker (subclass)"),
    (0xD471_D331, 6299, "Gjallarhorn (heavy)"),
    (0x5508_AC2A, 1512, "The Reflective Proof (emblem)"),
    (0x7D79_A673, 5348, "Duck Horn"),
];

fn ok(name: &'static str, detail: impl Into<String>) -> Control {
    Control { name, pass: true, detail: detail.into() }
}
fn no(name: &'static str, detail: impl Into<String>) -> Control {
    Control { name, pass: false, detail: detail.into() }
}

/// Every `entries[].hash` in [`CATALOGUE_PATH`], found by ascending from the current directory the
/// same way [`IndexTable::load_default`] finds the table. The error names the file, because "the
/// control did not run" and "the control failed" must never read the same.
fn catalogue_hashes() -> Result<(PathBuf, Vec<u32>)> {
    let mut dir = std::env::current_dir().map_err(|e| OracleError::Shape(format!("cwd: {e}")))?;
    let path = loop {
        let p = dir.join(CATALOGUE_PATH);
        if p.exists() {
            break p;
        }
        if !dir.pop() {
            return Err(OracleError::Shape(format!(
                "{CATALOGUE_PATH} not found in this directory or any parent"
            )));
        }
    };
    let text = std::fs::read_to_string(&path)
        .map_err(|e| OracleError::Shape(format!("{}: {e}", path.display())))?;
    let v: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| OracleError::Shape(format!("{}: {e}", path.display())))?;
    let entries = v.get("entries").and_then(|e| e.as_array()).ok_or_else(|| {
        OracleError::Shape(format!("{}: no `entries` array", path.display()))
    })?;
    let hashes: Vec<u32> =
        entries.iter().filter_map(|e| e.get("hash")?.as_u64()).map(|h| h as u32).collect();
    if hashes.is_empty() {
        return Err(OracleError::Shape(format!(
            "{}: `entries` carries no `hash` field", path.display()
        )));
    }
    Ok((path, hashes))
}

pub fn controls() -> Result<Vec<Control>> {
    let mut out = Vec::new();
    let t = IndexTable::load_default()?;
    out.push(ok("table-provenance", format!(
        "{}: {} rows from {}.pkg entry {} (class {:#010x}), sha256 {}…",
        t.path.display(), t.len(), t.source_package, t.source_entry, t.element_class,
        &t.source_sha256[..16])));

    let mut lines = Vec::new();
    let mut bad = 0;
    for (h, want, name) in PINNED {
        match t.index_of(h) {
            Some(i) if i == want => lines.push(format!("{name} {h:#010x} -> {i}")),
            Some(i) => {
                bad += 1;
                lines.push(format!("{name} {h:#010x} -> {i}, DOC SAYS {want}"));
            }
            None => {
                bad += 1;
                lines.push(format!("{name} {h:#010x} -> ABSENT"));
            }
        }
    }
    out.push(if bad == 0 {
        ok("pinned-indices", lines.join("; "))
    } else {
        no("pinned-indices", lines.join("; "))
    });

    let present: Vec<&str> = NOT_ITEMS.iter()
        .filter(|(h, _)| t.index_of(*h).is_some())
        .map(|(_, n)| *n)
        .collect();
    out.push(if present.is_empty() {
        ok("hash-space-is-items-NEGATIVE",
           "a progression hash, a stat hash and a bucket hash all resolve to nothing - the table is \
            the ITEM space, not a hash pool")
    } else {
        no("hash-space-is-items-NEGATIVE",
           format!("these must be absent and are not: {present:?}"))
    });

    // ⭐ THE POSITIVE CONTROL, in the same window as the negative one above. Without it, a table
    // that resolved NOTHING would pass `hash-space-is-items-NEGATIVE` cleanly. See [`CATALOGUE_PATH`].
    match catalogue_hashes() {
        Ok((cat, hashes)) => {
            let hits = hashes.iter().filter(|h| t.index_of(**h).is_some()).count();
            // INVERTED, in the same window: the same hashes, perturbed, must NOT resolve. A table
            // that answered whatever it was asked would score the same on both.
            // ⚠ See CATALOGUE_INVERSION_MASK for why the perturbation is a wide XOR and not `+1`.
            let perturbed = hashes
                .iter()
                .filter(|h| t.index_of(*h ^ CATALOGUE_INVERSION_MASK).is_some())
                .count();
            out.push(if hits > CATALOGUE_MIN_HITS && perturbed * 100 < hits {
                ok("catalogue-coverage-and-INVERTED", format!(
                    "{}: {hits}/{} public-manifest item rows resolve (floor {CATALOGUE_MIN_HITS}); \
                     the same hashes XOR {CATALOGUE_INVERSION_MASK:#010x} resolve {perturbed} - the \
                     table answers ITEMS, not whatever it is asked",
                    cat.display(), hashes.len()))
            } else {
                no("catalogue-coverage-and-INVERTED", format!(
                    "{}: {hits}/{} rows resolve (want more than {CATALOGUE_MIN_HITS}) and the \
                     perturbed control resolved {perturbed} (want under 1% of {hits})",
                    cat.display(), hashes.len()))
            });
        }
        Err(e) => out.push(no("catalogue-coverage-and-INVERTED", format!(
            "{e} - the positive half of the hash-space check DID NOT RUN, so the negative half \
             below it is a scan with no control in the same window"))),
    }

    // The emblem slot is a measured byte, not a choice. Assert it is inside the card's array and
    // that the shipped emblem hash resolves; both are load-bearing for `character build`.
    let emblem = t.index_of(DEFAULT_EMBLEM_HASH);
    out.push(match emblem {
        Some(i) => ok("emblem", format!(
            "slot {EMBLEM_SLOT} (investment_globals->vtbl[0x160]()[0x18], replayed = 0x0C); \
             {DEFAULT_EMBLEM_HASH:#010x} 'The Reflective Proof' -> content index {i}, world tag \
             {:#010x}", t.world_tag_of(DEFAULT_EMBLEM_HASH).unwrap_or(0))),
        None => no("emblem", format!("{DEFAULT_EMBLEM_HASH:#010x} is not in the table")),
    });

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_table_loads_and_its_pins_hold() {
        let t = match IndexTable::load_default() {
            Ok(t) => t,
            // The table is generated; a checkout without it must not fail the whole suite.
            Err(_) => return,
        };
        assert_eq!(t.len(), COUNT);
        for (h, want, name) in PINNED {
            assert_eq!(t.index_of(h), Some(want), "{name}");
        }
        for (h, name) in NOT_ITEMS {
            assert_eq!(t.index_of(h), None, "{name} must NOT be an item");
        }
    }

    /// ⭐ The positive control the Rust port dropped, and its inversion. Skips when the (untracked)
    /// catalogue is absent so a clone without it still runs the suite - the `controls` subcommand
    /// does NOT skip, exactly as `item-index.py --controls` did not.
    #[test]
    fn the_catalogue_positive_control_holds_and_inverts() {
        let Ok(t) = IndexTable::load_default() else { return };
        let Ok((_p, hashes)) = catalogue_hashes() else {
            eprintln!("SKIP: no {CATALOGUE_PATH}");
            return;
        };
        let hits = hashes.iter().filter(|h| t.index_of(**h).is_some()).count();
        assert!(hits > CATALOGUE_MIN_HITS, "only {hits} of {} rows resolve", hashes.len());
        let perturbed = hashes
            .iter()
            .filter(|h| t.index_of(*h ^ CATALOGUE_INVERSION_MASK).is_some())
            .count();
        assert!(
            perturbed * 100 < hits,
            "the perturbed hashes resolve {perturbed} times against {hits} - the table is answering \
             whatever it is asked, so the coverage number proves nothing"
        );
        // ⭐ The paid-for lesson, asserted so it cannot be forgotten: `+1` is NOT a valid
        // perturbation here. If this ever stops holding, the hash space became sparse and the
        // comment on CATALOGUE_INVERSION_MASK needs rewriting.
        let off_by_one = hashes.iter().filter(|h| t.index_of(h.wrapping_add(1)).is_some()).count();
        assert!(
            off_by_one * 4 > hits,
            "off-by-one now resolves only {off_by_one} of {hits}; the item hash space is no longer \
             dense in +1 neighbours and CATALOGUE_INVERSION_MASK's justification has gone stale"
        );
    }

    #[test]
    fn a_table_with_the_wrong_provenance_is_refused() {
        let dir = std::env::temp_dir().join(format!("d1-item-index-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("bad.tsv");
        std::fs::write(&p, "# source: some_other_package.pkg entry 1\n\
                            # element class 0xdeadbeef, 24 B, count 7544\n\
                            # sha256(entry payload) = 00\n\
                            item_hash\tcontent_index\tworld_tag\n\
                            0x1\t0\t0x2\n").unwrap();
        assert!(IndexTable::load(&p).is_err());
        let _ = std::fs::remove_file(&p);
    }
}
