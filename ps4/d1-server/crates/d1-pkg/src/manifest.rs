// SPDX-License-Identifier: GPL-2.0-or-later
//! The **CDN content manifest** - the protobuf body the client fetches and parses on every boot.
//!
//! # Why this lives in Rust, and in this crate
//!
//! This is a wire format the client parses, so by project rule it belongs in Rust and nowhere else.
//! It lives in `d1-pkg` because every value it emits is read out of a package header, and `d1-pkg`
//! is already the single description of that header. One crate therefore remains the only
//! implementer of the manifest format: reader, writer and estate scanner in one place.
//!
//! Supersedes the prototype generator `capture-rig/make-manifest.py`, whose three policies are
//! reproduced here byte for byte (see the `golden_*` controls).
//!
//! # This is a generator, not a fixture
//!
//! Ids, GUIDs, generations and locales are **read out of the packages we own**. That is what gives
//! the emitted values `observed-*` reach where a hand-written manifest would be `synthetic-generic`
//! forever. The **schema** is still `static-client-inference`
//! (`design/content-manifest-gate.md` 5.3, `design/package-registration.md`), so the manifest as a
//! whole is not a protocol contract.
//!
//! # What the engine does with it
//!
//! The manifest **resolves nothing by name.** It registers packages. Logical resource names
//! (`client_bootstrap_patchable`, `investment_globals`, ...) are then found by scanning the symbol
//! table inside each registered package, newest build timestamp winning. That is the patch-override
//! mechanism, and it is why zero-symbol packages must still be registered: they are byte stores for
//! the generation chain.
//!
//! # Header fields read  `[C]`, 1647/1647 files
//!
//! ```text
//! +0x04  u16  package id      equals the 4-hex id in the filename, 0 mismatches
//! +0x08  u64  GUID
//! +0x10  u32  build timestamp ascends with generation
//! +0x20  u16  generation      equals the `_N` suffix in the filename
//! +0x22  u16  locale          0 neutral, 1 en, 2 fr, 3 it, 4 de, 5 sp, 6 jpn, 7 pt
//! ```
//!
//! # Fields emitted
//!
//! ```text
//! top      f3  repeated Package   (cap PACKAGE_CAP - exceeding it is `array overflow`,
//!                                  and the WHOLE parse fails, not just the tail)
//! top      f5  string guid        must be PRESENT; must strncmp-match engine config key 101
//! Package  f1  offer id           NOT the package id. Always 0 - see L11 below.
//! Package  f2  string path stem   the engine opens `packages/<stem>.pkg`
//! Package  f3  u32 package id     header +0x04
//! Package  f4  u64 GUID           header +0x08
//! ```
//!
//! # L11: `Package.f1` and signon `f10` are an atomic pair
//!
//! The moment `f1` carries a real DLC offer id, `package_registration` calls `is_offer_key_owned()`
//! against the signon `f10` ownership list we have never sent, and packages vanish from the tag
//! space **with no diagnostic**. Ship both or neither. [`encode`] therefore refuses a nonzero `f1`
//! outright rather than trusting a caller to remember.
//!
//! # Provenance warning
//!
//! Serving this needs the lab manifest patch, which clears the manifest RSA-envelope check.
//! Anything reached through it is `synthetic_ancestor`.

use anyhow::{bail, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::Header;

/// Descriptor capacity for the repeated `Package` array. More than this is `array overflow` and the
/// entire manifest fails to parse - the client does not keep the first 1,325.
pub const PACKAGE_CAP: usize = 1325;

/// Size of engine configuration key 101 (`char[0x25]`, 36 characters plus NUL), and the length the
/// client's `strncmp` against the manifest `guid` runs over.
pub const CONFIG_KEY_101_SIZE: usize = 0x25;

/// The packages the engine registers **itself**, by hard-coded name (`"ps4"` plus `"%s_%s"`).
///
/// Listing them again in the manifest is at best redundant. On disk each of these three package ids
/// exists under two spellings, `_unp3_0` and `_unpatchable_0`; only the `_unp3_` spelling is named
/// here because that is the set the prototype generator dropped, and the legacy policies must stay
/// byte-identical to it. [`Policy::AllGenerations`] drops by **package id** instead, which catches
/// both spellings - see [`engine_ids`].
pub const ENGINE_REGISTERED: [&str; 3] = [
    "ps4_client_bootstrap_unp3_0",
    "ps4_ui_startup_unp3_0",
    "ps4_ui_bootflow_unp3_0",
];

/// [`ENGINE_REGISTERED`] without its platform prefix: the engine forms each name as `"%s_%s"` from
/// the platform's prefix ([`crate::Platform::prefix`]) and one of these.
pub const ENGINE_REGISTERED_SUFFIXES: [&str; 3] =
    ["client_bootstrap_unp3_0", "ui_startup_unp3_0", "ui_bootflow_unp3_0"];

/// Whether `stem` is one of the packages the engine registers itself, on any [`crate::Platform`]:
/// for a `ps4_` stem exactly the membership of [`ENGINE_REGISTERED`].
pub fn is_engine_registered(stem: &str) -> bool {
    crate::Platform::of_stem(stem)
        .is_some_and(|p| ENGINE_REGISTERED_SUFFIXES.contains(&&stem[p.prefix().len() + 1..]))
}

/// Locale 0: language-independent content. Always kept, whatever `--lang` says.
pub const LOCALE_NEUTRAL: u16 = 0;

/// Locale code to language tag, **derived from the estate, not from a spec**: every one of the 709
/// language-suffixed filenames agrees with its own header `+0x22`, and all 938 unsuffixed files are
/// locale 0. `sp` and `jpn` are the game's own spellings, not ISO codes.
pub const LANGUAGES: [(&str, u16); 7] =
    [("en", 1), ("fr", 2), ("it", 3), ("de", 4), ("sp", 5), ("jpn", 6), ("pt", 7)];

pub fn locale_code(tag: &str) -> Option<u16> {
    LANGUAGES.iter().find(|(t, _)| *t == tag).map(|(_, c)| *c)
}

pub fn locale_tag(code: u16) -> Option<&'static str> {
    if code == LOCALE_NEUTRAL {
        return Some("neutral");
    }
    LANGUAGES.iter().find(|(_, c)| *c == code).map(|(t, _)| *t)
}

// ─────────────────────────────────── the estate scan ────────────────────────────────

/// One package file, reduced to the header fields the manifest needs.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Row {
    pub path: PathBuf,
    pub stem: String,
    pub id: u16,
    pub guid: u64,
    pub timestamp: u32,
    pub gen: u16,
    pub locale: u16,
}

impl Row {
    /// **The supersession key**, and the one thing here that is not a plain field read.
    ///
    /// It is the 32-bit word at header `+0x20`, i.e. `gen | locale << 16`. The prototype generator
    /// read `+0x20` with `<I` and therefore used this without meaning to; every golden manifest in
    /// the project, `capture-rig/manifests/full_en.bin` included, is ordered by it.
    ///
    /// Keeping it is deliberate, not inertia. Package ids are **not** unique across locales, so
    /// `newest` and `all` do compare a `fr` generation against a `de` one, and there is no true
    /// answer to that comparison: nothing supersedes anything across a locale boundary. This key
    /// makes the arbitrary choice locale-major and, above all, stable. Where the question is real
    /// (one id, one locale) the low half is the generation and the key is exactly `gen`.
    ///
    /// The `all-generations` policy never depends on it, because after the language filter every
    /// row sharing an id also shares a locale. `controls` asserts both halves of that: identical
    /// output for `all-generations`, **different** output for `newest` and `all`, so the
    /// distinction is measured rather than assumed.
    pub fn sort_key(&self) -> u32 {
        self.gen as u32 | (self.locale as u32) << 16
    }
}

/// Every `*.pkg` in `dir`, header-only, sorted by path so the output is deterministic.
///
/// Files that are too short to carry a header are skipped, not fatal - the same tolerance the
/// prototype had, so a stray non-package in the directory cannot take the generator down.
pub fn scan(dir: impl AsRef<Path>) -> Result<Vec<Row>> {
    let dir = dir.as_ref();
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| anyhow::anyhow!("reading {}: {e}", dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "pkg"))
        .collect();
    paths.sort();

    let mut rows = Vec::with_capacity(paths.len());
    for p in paths {
        let Ok(h) = Header::read(&p) else { continue };
        let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        rows.push(Row {
            path: p,
            stem,
            id: h.id,
            guid: h.guid,
            timestamp: h.timestamp,
            gen: h.gen,
            locale: h.locale,
        });
    }
    Ok(rows)
}

/// Package ids the engine registers itself, resolved from the estate rather than hard-coded.
///
/// Resolving instead of hard-coding is what makes the `_unpatchable_` spelling fall out for free:
/// it shares its package id with the `_unp3_` file, so dropping the id drops both.
pub fn engine_ids(rows: &[Row]) -> BTreeSet<u16> {
    rows.iter().filter(|r| is_engine_registered(&r.stem)).map(|r| r.id).collect()
}

// ────────────────────────────────────── policies ────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Policy {
    /// Highest generation per package id, across every locale. 393 packages.
    Newest,
    /// Every generation of every package. 1,644 - **over [`PACKAGE_CAP`]**, so the client rejects
    /// the whole thing. Kept because it is what the original 7-entry probe used and because the
    /// golden control needs it.
    All,
    /// Only packages that DECLARE a symbol, newest generation each. 175 packages: the smallest set
    /// that can satisfy name resolution, and **not** a set that can boot - zero-symbol packages are
    /// real dependencies.
    SymbolsNewest,
    /// ⭐ The served policy (ledger W2.4): neutral plus one language, **all generations**, engine
    /// ids dropped by id. 1,033 packages with `--lang en`. This is the one that reproduces
    /// `capture-rig/manifests/full_en.bin`.
    AllGenerations,
}

impl Policy {
    /// The language `--lang` defaults to for this policy. Only `AllGenerations` filters by default;
    /// the three legacy policies must stay unfiltered or they stop matching the prototype.
    pub fn default_lang(&self) -> Option<&'static str> {
        match self {
            Policy::AllGenerations => Some("en"),
            _ => None,
        }
    }

    /// The `--policy` spelling, so a log line can be pasted straight back onto a command line.
    pub fn tag(&self) -> &'static str {
        match self {
            Policy::Newest => "newest",
            Policy::All => "all",
            Policy::SymbolsNewest => "symbols-newest",
            Policy::AllGenerations => "all-generations",
        }
    }
}

/// What to select out of the estate.
#[derive(Debug, Clone)]
pub struct Select {
    pub policy: Policy,
    /// Restrict to these package ids. Applied first, before any policy.
    pub only: Option<BTreeSet<u16>>,
    /// Keep locale 0 plus this locale. `None` means every locale.
    pub lang: Option<u16>,
}

/// Apply `sel` to a scanned estate.
///
/// ⚠ **Order is load-bearing and is the prototype's order**: `only` filter, then the symbol filter,
/// then the policy, then the engine drop, in that sequence. Dropping the engine packages *after*
/// the newest-generation grouping is what makes ids 66/73/377 disappear entirely rather than fall
/// back to their `_unpatchable_` twin, which is why `Newest` emits 393 and not 396.
pub fn select(rows: &[Row], sel: &Select) -> Vec<Row> {
    let mut rows: Vec<Row> = rows.to_vec();

    if let Some(only) = &sel.only {
        rows.retain(|r| only.contains(&r.id));
    }
    if let Some(lang) = sel.lang {
        rows.retain(|r| r.locale == LOCALE_NEUTRAL || r.locale == lang);
    }
    if sel.policy == Policy::SymbolsNewest {
        rows.retain(|r| {
            Header::read(&r.path)
                .and_then(|h| crate::read_symbol_names(&r.path, &h))
                .map(|s| !s.is_empty())
                .unwrap_or(false)
        });
    }

    match sel.policy {
        Policy::Newest | Policy::SymbolsNewest => {
            // Highest generation wins; ties break on the build timestamp so the rule matches the
            // engine's own newest-wins resolution. A full tie keeps the first in path order.
            let mut best: BTreeMap<u16, Row> = BTreeMap::new();
            for r in rows {
                match best.get(&r.id) {
                    Some(cur) if (cur.sort_key(), cur.timestamp) >= (r.sort_key(), r.timestamp) => {}
                    _ => {
                        best.insert(r.id, r);
                    }
                }
            }
            rows = best.into_values().collect(); // BTreeMap iterates ids ascending
        }
        Policy::All | Policy::AllGenerations => {
            // Stable, so a full tie keeps path order.
            rows.sort_by_key(|r| (r.id, r.sort_key()));
        }
    }

    match sel.policy {
        // By id: catches `_unp3_0` and its `_unpatchable_0` twin, which share a package id.
        Policy::AllGenerations => {
            let ids = engine_ids(&rows);
            rows.retain(|r| !ids.contains(&r.id));
        }
        // By stem: what the prototype did, and what the golden bytes encode.
        _ => rows.retain(|r| !is_engine_registered(&r.stem)),
    }
    rows
}

// ────────────────────────────────────── encoding ────────────────────────────────────

fn varint(out: &mut Vec<u8>, mut n: u64) {
    loop {
        let b = (n & 0x7F) as u8;
        n >>= 7;
        out.push(if n != 0 { b | 0x80 } else { b });
        if n == 0 {
            return;
        }
    }
}

fn pb_varint(out: &mut Vec<u8>, field: u32, v: u64) {
    varint(out, (field << 3) as u64);
    varint(out, v);
}

fn pb_bytes(out: &mut Vec<u8>, field: u32, v: &[u8]) {
    varint(out, ((field << 3) | 2) as u64);
    varint(out, v.len() as u64);
    out.extend_from_slice(v);
}

/// Does a manifest `guid` satisfy the client's check against engine config key 101?
///
/// The client `strncmp`s the two over [`CONFIG_KEY_101_SIZE`] bytes. Both default to `""`, which is
/// why `2a 00` - field 5 present, empty - is the minimal accepted manifest.
pub fn guid_matches_config(manifest_guid: &str, key101: &str) -> bool {
    let a = manifest_guid.as_bytes();
    let b = key101.as_bytes();
    let n = CONFIG_KEY_101_SIZE;
    match (a.len() >= n, b.len() >= n) {
        (true, true) => a[..n] == b[..n],
        _ => a == b,
    }
}

/// Serialise the manifest body.
///
/// **L11 guard.** `offer_id` is not a parameter: it is 0 for every package, always, and [`decode`]
/// re-reads the produced bytes to prove it. Introducing a real offer id here without also shipping
/// signon `f10` makes packages vanish from the tag space with no diagnostic.
pub fn encode(rows: &[Row], guid: &str) -> Result<Vec<u8>> {
    if guid.len() >= CONFIG_KEY_101_SIZE {
        bail!(
            "manifest guid is {} bytes; engine config key 101 is char[{:#x}] (36 chars plus NUL), \
             so a longer guid can never strncmp-match and the client would reject the manifest",
            guid.len(),
            CONFIG_KEY_101_SIZE
        );
    }
    let mut out = Vec::new();
    for r in rows {
        let mut p = Vec::new();
        pb_varint(&mut p, 1, 0); // offer id: 0, unconditionally. See L11.
        pb_bytes(&mut p, 2, r.stem.as_bytes());
        pb_varint(&mut p, 3, r.id as u64);
        pb_varint(&mut p, 4, r.guid);
        pb_bytes(&mut out, 3, &p);
    }
    pb_bytes(&mut out, 5, guid.as_bytes()); // PRESENT, empty matches the config default
    Ok(out)
}

// ────────────────────────────────────── decoding ────────────────────────────────────

/// One `Package` record as read back off the wire.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DecodedPackage {
    pub offer_id: u64,
    pub stem: String,
    pub id: u64,
    pub guid: u64,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct Decoded {
    pub packages: Vec<DecodedPackage>,
    pub guid: Option<String>,
}

fn get_varint(b: &[u8], i: &mut usize) -> Result<u64> {
    let (mut n, mut s) = (0u64, 0u32);
    loop {
        let c = *b.get(*i).ok_or_else(|| anyhow::anyhow!("truncated varint at {i}"))?;
        *i += 1;
        n |= ((c & 0x7F) as u64) << s;
        if c & 0x80 == 0 {
            return Ok(n);
        }
        s += 7;
        if s >= 64 {
            bail!("varint longer than 64 bits at {i}");
        }
    }
}

/// Parse a manifest body back into records.
///
/// This is the same format as [`encode`], written next to it on purpose: a decoder somewhere else
/// would be the format described twice, and having it here is what lets the L11 control read the
/// bytes actually produced instead of asserting on a constant it just wrote.
pub fn decode(b: &[u8]) -> Result<Decoded> {
    let mut out = Decoded::default();
    let mut i = 0usize;
    while i < b.len() {
        let key = get_varint(b, &mut i)?;
        let (field, wire) = ((key >> 3) as u32, (key & 7) as u8);
        match wire {
            0 => {
                get_varint(b, &mut i)?;
            }
            2 => {
                let len = get_varint(b, &mut i)? as usize;
                let end = i.checked_add(len).filter(|e| *e <= b.len()).ok_or_else(|| {
                    anyhow::anyhow!("length-delimited field {field} runs past the buffer")
                })?;
                let body = &b[i..end];
                i = end;
                match field {
                    3 => out.packages.push(decode_package(body)?),
                    5 => out.guid = Some(String::from_utf8_lossy(body).into_owned()),
                    _ => {}
                }
            }
            w => bail!("unsupported wire type {w} for field {field} at offset {i}"),
        }
    }
    Ok(out)
}

fn decode_package(b: &[u8]) -> Result<DecodedPackage> {
    let mut p = DecodedPackage { offer_id: 0, stem: String::new(), id: 0, guid: 0 };
    let mut i = 0usize;
    while i < b.len() {
        let key = get_varint(b, &mut i)?;
        let (field, wire) = ((key >> 3) as u32, (key & 7) as u8);
        match (field, wire) {
            (1, 0) => p.offer_id = get_varint(b, &mut i)?,
            (3, 0) => p.id = get_varint(b, &mut i)?,
            (4, 0) => p.guid = get_varint(b, &mut i)?,
            (_, 0) => {
                get_varint(b, &mut i)?;
            }
            (f, 2) => {
                let len = get_varint(b, &mut i)? as usize;
                let end = i
                    .checked_add(len)
                    .filter(|e| *e <= b.len())
                    .ok_or_else(|| anyhow::anyhow!("Package field {f} runs past the record"))?;
                if f == 2 {
                    p.stem = String::from_utf8_lossy(&b[i..end]).into_owned();
                }
                i = end;
            }
            (f, w) => bail!("unsupported wire type {w} for Package field {f}"),
        }
    }
    Ok(p)
}

/// L11 in one call: every emitted `Package.f1` must be 0. Returns the offending stems.
pub fn nonzero_offer_ids(body: &[u8]) -> Result<Vec<String>> {
    Ok(decode(body)?
        .packages
        .into_iter()
        .filter(|p| p.offer_id != 0)
        .map(|p| p.stem)
        .collect())
}

// ───────────────────────────────── build, end to end ────────────────────────────────

/// Everything a caller needs about one generated manifest.
pub struct Built {
    pub rows: Vec<Row>,
    pub body: Vec<u8>,
}

/// Scan, select, encode, and enforce both the offer-id guard and the descriptor cap.
///
/// `allow_overflow` emits a body the client would reject with `array overflow`. It exists only so
/// the `all` policy, which selects 1,644 packages, can be byte-compared against the prototype.
pub fn build(
    dir: impl AsRef<Path>,
    sel: &Select,
    guid: &str,
    key101: &str,
    allow_overflow: bool,
) -> Result<Built> {
    if !guid_matches_config(guid, key101) {
        bail!(
            "manifest guid {guid:?} does not strncmp-match engine config key 101 {key101:?} over \
             {CONFIG_KEY_101_SIZE:#x} bytes - the client would fail content_check. Set --config-guid \
             to the value the engine is configured with, or leave both empty."
        );
    }
    let all = scan(dir.as_ref())?;
    if all.is_empty() {
        bail!("no packages under {}", dir.as_ref().display());
    }
    let rows = select(&all, sel);
    let body = encode(&rows, guid)?;

    let bad = nonzero_offer_ids(&body)?;
    if !bad.is_empty() {
        bail!(
            "L11 violation: {} package(s) carry a nonzero offer id ({}). Package.f1 and signon f10 \
             are an atomic pair; emitting one without the other makes packages vanish from the tag \
             space with no diagnostic.",
            bad.len(),
            bad.join(", ")
        );
    }
    if rows.len() > PACKAGE_CAP && !allow_overflow {
        bail!(
            "{} packages exceeds the {PACKAGE_CAP} descriptor cap - the WHOLE parse would fail \
             with `array overflow`, not just the tail. Narrow with --only, --lang, or a smaller \
             --policy (--allow-overflow emits it anyway, for byte-comparison only).",
            rows.len()
        );
    }
    Ok(Built { rows, body })
}

// ──────────────────────────────────── controls ──────────────────────────────────────

/// A pinned expectation. `detail` is what was actually measured.
pub struct ControlRow {
    pub name: String,
    pub passed: bool,
    /// The control could not run because its input is absent. Reported as SKIP, not PASS, so a
    /// reader is never told a check succeeded when it never executed. A skipped row does not fail
    /// the suite **only** where another row already pins the same claim by digest.
    pub skipped: bool,
    pub detail: String,
}

pub fn sha256_hex(b: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    crate::hex(&Sha256::digest(b))
}

/// Package counts on the pinned 01.29 estate, **measured 2026-08-11 at port time**.
///
/// ⚠ These correct `design/SERVER-SURFACE-LEDGER.md` L10, which reads "178 / 396 / 1,644". Those
/// two numbers are pre-engine-drop and swapped: `symbols-newest` selects 178 and keeps 175,
/// `newest` selects 396 and keeps 393.
pub const PINNED_ESTATE_FILES: usize = 1647;
pub const PINNED_NEWEST: usize = 393;
pub const PINNED_SYMBOLS_NEWEST: usize = 175;
pub const PINNED_ALL: usize = 1644;
pub const PINNED_ALL_GENERATIONS_EN: usize = 1033;

/// SHA-256 of `capture-rig/make-manifest.py --policy <p> --guid ""` on the pinned estate, measured
/// once at port time. `all` was taken with the prototype's descriptor cap lifted, because the
/// prototype refuses to write 1,644 packages; nothing else about that body differs.
pub const GOLDEN_NEWEST: (usize, &str) =
    (20370, "adfbd8a423391d413b887536de5722b023fb5f6c9daa74a40ace0149d6a3dcd5");
pub const GOLDEN_SYMBOLS_NEWEST: (usize, &str) =
    (9387, "c70899fa019fd7069da931ee057c1377954e12c2940335292c674a4dd38e63ab");
pub const GOLDEN_ALL: (usize, &str) =
    (85649, "8fc47baf482f29ef0e2ee083455f258606ac71edb5a7c817798490d36e20472a");

/// The served artifact, `capture-rig/manifests/full_en.bin`. Pinned by digest so the control still
/// discriminates on a clone where that file is absent (it is Bungie-derived and never shipped).
pub const GOLDEN_FULL_EN: (usize, &str) =
    (52797, "0c5bba3c63e9b68021b536513639cfc8896ec736950323e4ab06c5d7ed0770be");

fn sel(policy: Policy, lang: Option<u16>) -> Select {
    Select { policy, only: None, lang }
}

/// Run every manifest control against `dir`. Callers map any failed row to exit 2.
///
/// `served` is the served artifact to byte-compare against; when it is absent the digest control
/// still discriminates, so the suite does not become uncheckable on a clone without the dump.
pub fn controls(dir: impl AsRef<Path>, served: Option<&Path>) -> Result<Vec<ControlRow>> {
    let dir = dir.as_ref();
    let mut rows: Vec<ControlRow> = Vec::new();
    // A macro, not a closure: a closure would hold `rows` borrowed for the whole function and the
    // one row that is built by hand (the SKIP below) could not be pushed alongside it.
    macro_rules! push {
        ($name:expr, $passed:expr, $detail:expr $(,)?) => {
            rows.push(ControlRow {
                name: ($name).to_string(),
                passed: $passed,
                skipped: false,
                detail: $detail,
            })
        };
    }

    let estate = scan(dir)?;
    push!(
        "estate guard: .pkg files under the pinned dump",
        estate.len() == PINNED_ESTATE_FILES,
        format!("{} (expected {PINNED_ESTATE_FILES})", estate.len()),
    );

    // (a) policy counts, and (b) golden byte-identity with the prototype.
    let cases: [(&str, Policy, Option<u16>, usize, Option<(usize, &str)>); 4] = [
        ("newest", Policy::Newest, None, PINNED_NEWEST, Some(GOLDEN_NEWEST)),
        (
            "symbols-newest",
            Policy::SymbolsNewest,
            None,
            PINNED_SYMBOLS_NEWEST,
            Some(GOLDEN_SYMBOLS_NEWEST),
        ),
        ("all", Policy::All, None, PINNED_ALL, Some(GOLDEN_ALL)),
        (
            "all-generations --lang en",
            Policy::AllGenerations,
            Some(1),
            PINNED_ALL_GENERATIONS_EN,
            Some(GOLDEN_FULL_EN),
        ),
    ];
    let mut bodies: BTreeMap<&str, Vec<u8>> = BTreeMap::new();
    for (name, policy, lang, want, golden) in cases {
        let picked = select(&estate, &sel(policy, lang));
        push!(
            &format!("policy {name}: package count"),
            picked.len() == want,
            format!("{} (expected {want})", picked.len()),
        );
        let body = encode(&picked, "")?;
        if let Some((wlen, wsha)) = golden {
            let got = sha256_hex(&body);
            push!(
                &format!("policy {name}: byte-identical to the pinned golden"),
                body.len() == wlen && got == wsha,
                format!("{} bytes sha256 {got}", body.len()),
            );
        }
        // (d) L11: read the offer ids back out of the bytes we just wrote.
        let bad = nonzero_offer_ids(&body)?;
        push!(
            &format!("policy {name}: L11 every Package.f1 == 0 (re-decoded, not assumed)"),
            bad.is_empty(),
            format!("{} nonzero of {} packages", bad.len(), decode(&body)?.packages.len()),
        );
        bodies.insert(name, body);
    }

    // (c) W2.4 acceptance against the served file itself, when it is present.
    //     `full_en.bin` is Bungie-derived and never shipped, so a clone will not have it. Its
    //     absence is a SKIP, not a pass and not a failure: the pinned-digest row above makes the
    //     identical claim without needing the file.
    let served_bytes = served.and_then(|p| std::fs::read(p).ok());
    match &served_bytes {
        Some(bytes) => push!(
            "W2.4: all-generations --lang en is byte-identical to the served full_en.bin",
            bodies.get("all-generations --lang en").is_some_and(|b| b == bytes),
            format!("served {} bytes", bytes.len()),
        ),
        None => rows.push(ControlRow {
            name: "W2.4: direct byte-compare against the served full_en.bin".into(),
            passed: true,
            skipped: true,
            detail: "file absent; the pinned-digest row above makes the same claim".into(),
        }),
    }

    // Row::sort_key is the u32 word at +0x20, not the u16 generation. Measure what that choice
    // costs instead of asserting it is harmless: it must NOT matter for the served policy, and it
    // MUST matter for the two legacy ones, or the pinned goldens above are not discriminating.
    {
        let by_gen16 = |policy, lang| {
            let mut v = select(&estate, &sel(policy, lang));
            v.sort_by_key(|r| (r.id, r.gen));
            v.into_iter().map(|r| r.stem).collect::<Vec<_>>()
        };
        let stems = |policy, lang| {
            select(&estate, &sel(policy, lang)).into_iter().map(|r| r.stem).collect::<Vec<_>>()
        };
        push!(
            "served policy does not depend on the sort key: all-generations is identical under \
             gen-u16 and the u32 word at +0x20",
            by_gen16(Policy::AllGenerations, Some(1)) == stems(Policy::AllGenerations, Some(1)),
            format!("{} rows", PINNED_ALL_GENERATIONS_EN),
        );
        push!(
            "INVERTED: the legacy policies DO depend on it - `all` reorders under gen-u16, so the \
             golden bytes are pinning a real choice",
            by_gen16(Policy::All, None) != stems(Policy::All, None),
            "differs, as it must".into(),
        );
    }

    // (f) guid must fail closed against engine config key 101.
    {
        let mismatched = build(dir, &sel(Policy::AllGenerations, Some(1)), "abc", "", false);
        let matched = build(dir, &sel(Policy::AllGenerations, Some(1)), "abc", "abc", false);
        push!(
            "guid mismatch with engine config key 101 fails closed",
            mismatched.is_err() && matched.is_ok(),
            format!("mismatch err={} match ok={}", mismatched.is_err(), matched.is_ok()),
        );
        let over_long = encode(&[], &"g".repeat(CONFIG_KEY_101_SIZE));
        push!(
            "guid too long for char[0x25] is refused",
            over_long.is_err(),
            format!("refused={}", over_long.is_err()),
        );
    }

    // (e) INVERTED CONTROL. Copy three real generations of one package into a temp estate, mutate a
    //     single header byte, and require the manifest to change. If it does not, the emitted
    //     values are not being read off disk and every number above is decoration.
    match inverted_control(&estate) {
        Ok(r) => rows.extend(r),
        Err(e) => push!("INVERTED: header mutation changes the manifest", false, format!("{e}")),
    }

    Ok(rows)
}

/// The inverted half of the count controls: prove the emitted values come off disk.
fn inverted_control(estate: &[Row]) -> Result<Vec<ControlRow>> {
    // A package id with at least three generations and small files, so the copy is cheap.
    let mut by_id: BTreeMap<u16, Vec<&Row>> = BTreeMap::new();
    for r in estate.iter().filter(|r| r.locale == LOCALE_NEUTRAL) {
        by_id.entry(r.id).or_default().push(r);
    }
    let chain = by_id
        .values()
        .filter(|v| v.len() >= 3)
        .min_by_key(|v| {
            v.iter().map(|r| std::fs::metadata(&r.path).map(|m| m.len()).unwrap_or(u64::MAX)).sum::<u64>()
        })
        .ok_or_else(|| anyhow::anyhow!("no package id with three generations in the estate"))?;

    let tmp = std::env::temp_dir().join(format!("d1-pkg-manifest-inverted-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp)?;
    for r in chain {
        std::fs::copy(&r.path, tmp.join(format!("{}.pkg", r.stem)))?;
    }

    let s = sel(Policy::Newest, None);
    let base = build(&tmp, &s, "", "", false)?;
    let newest = base.rows.last().cloned().ok_or_else(|| anyhow::anyhow!("empty temp estate"))?;
    let victim = tmp.join(format!("{}.pkg", newest.stem));

    let mut out = Vec::new();

    // Mutation 1: flip a GUID byte on the winning generation. The GUID is emitted verbatim, so the
    // body must change at exactly that value.
    let mut raw = std::fs::read(&victim)?;
    raw[8] ^= 0xFF;
    std::fs::write(&victim, &raw)?;
    let mutated = build(&tmp, &s, "", "", false)?;
    let guid_changed = mutated.body != base.body
        && mutated.rows.last().map(|r| r.guid) != base.rows.last().map(|r| r.guid);
    out.push(ControlRow {
        name: "INVERTED: flipping header +0x08 (GUID) changes the emitted manifest".into(),
        passed: guid_changed,
        skipped: false,
        detail: format!(
            "{} -> {}",
            base.rows.last().map(|r| r.guid).unwrap_or(0),
            mutated.rows.last().map(|r| r.guid).unwrap_or(0)
        ),
    });
    raw[8] ^= 0xFF;

    // Mutation 2: demote the winning generation. `newest` must now pick a different file, proving
    // the selection key is read and not assumed from the filename.
    raw[0x20] = 0;
    raw[0x21] = 0;
    std::fs::write(&victim, &raw)?;
    let demoted = build(&tmp, &s, "", "", false)?;
    let gen_changed = demoted.rows.last().map(|r| r.stem.clone()) != Some(newest.stem.clone());
    out.push(ControlRow {
        name: "INVERTED: demoting header +0x20 (generation) changes which file newest picks".into(),
        passed: gen_changed,
        skipped: false,
        detail: format!(
            "{} -> {}",
            newest.stem,
            demoted.rows.last().map(|r| r.stem.clone()).unwrap_or_default()
        ),
    });

    let _ = std::fs::remove_dir_all(&tmp);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DUMP: &str =
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../../dumps/patch-01.29-CUSA00219/packages");
    const SERVED: &str =
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../../capture-rig/manifests/full_en.bin");

    fn estate() -> Option<Vec<Row>> {
        Path::new(DUMP).is_dir().then(|| scan(DUMP).unwrap())
    }

    /// The encoder, against a hand-checked record. This one needs no dump, so it fails on any
    /// machine if the wire shape regresses.
    #[test]
    fn package_record_is_f1_f2_f3_f4_in_that_order() {
        let r = Row {
            path: PathBuf::new(),
            stem: "ps4_ui_core_000a_0".into(),
            id: 10,
            guid: 0x2B96_1B12_3456_7890,
            timestamp: 0,
            gen: 0,
            locale: 0,
        };
        let body = encode(&[r], "").unwrap();
        assert_eq!(&body[..2], &[0x1a, 0x22], "top-level f3, 0x22-byte Package");
        assert_eq!(&body[2..4], &[0x08, 0x00], "f1 offer id = 0");
        assert_eq!(&body[4..6], &[0x12, 0x12], "f2, 18-byte stem");
        assert_eq!(&body[body.len() - 2..], &[0x2a, 0x00], "f5 guid present and empty");

        let d = decode(&body).unwrap();
        assert_eq!(d.guid.as_deref(), Some(""));
        assert_eq!(
            d.packages[0],
            DecodedPackage {
                offer_id: 0,
                stem: "ps4_ui_core_000a_0".into(),
                id: 10,
                guid: 0x2B96_1B12_3456_7890
            }
        );
    }

    fn synthetic(stem: &str, id: u16) -> Row {
        Row { path: PathBuf::new(), stem: stem.into(), id, guid: 1, timestamp: 0, gen: 0, locale: 0 }
    }

    /// The PS4 names keep exactly their old membership, and the same three packages are recognised
    /// under the Xbox One prefix. Synthetic stems only.
    #[test]
    fn engine_registered_packages_follow_the_platform_prefix() {
        for s in ENGINE_REGISTERED {
            assert!(is_engine_registered(s), "{s}");
        }
        for s in ENGINE_REGISTERED_SUFFIXES {
            assert!(is_engine_registered(&format!("xboxone_{s}")), "xboxone_{s}");
        }
        for s in [
            "ps4_ui_core_000a_0",
            "ps4_client_bootstrap_unpatchable_0",
            "xboxone_client_bootstrap_unpatchable_0",
            "ps5_ui_startup_unp3_0",
            "xboxoneui_startup_unp3_0",
            "client_bootstrap_unp3_0",
        ] {
            assert!(!is_engine_registered(s), "{s}");
        }
    }

    /// An Xbox One estate drops its engine ids the same way a PS4 one does: by id under
    /// `AllGenerations` (catching the `_unpatchable_0` twin), by stem otherwise.
    #[test]
    fn xbox_one_engine_packages_are_dropped_like_ps4_ones() {
        let rows = vec![
            synthetic("xboxone_ui_core_000a_0", 10),
            synthetic("xboxone_client_bootstrap_unp3_0", 66),
            synthetic("xboxone_client_bootstrap_unpatchable_0", 66),
            synthetic("xboxone_ui_startup_unp3_0", 73),
        ];
        assert_eq!(engine_ids(&rows), BTreeSet::from([66, 73]));
        let all = select(&rows, &Select { policy: Policy::AllGenerations, only: None, lang: None });
        let stems: Vec<_> = all.iter().map(|r| r.stem.as_str()).collect();
        assert_eq!(stems, ["xboxone_ui_core_000a_0"]);
        let newest = select(&rows, &Select { policy: Policy::Newest, only: None, lang: None });
        assert!(newest.iter().all(|r| !is_engine_registered(&r.stem)));
    }

    /// The header's platform field: 7 is PS4, 8 Xbox One, anything else unknown. Synthetic header.
    #[test]
    fn header_platform_reads_the_tiger_platform_id() {
        use crate::Platform;
        let mut raw = [0u8; 0x140];
        raw[..2].copy_from_slice(&0x18u16.to_le_bytes());
        for (id, want) in [(7u16, Some(Platform::Ps4)), (8, Some(Platform::XboxOne)), (3, None)] {
            raw[2..4].copy_from_slice(&id.to_le_bytes());
            assert_eq!(Header::parse(&raw).platform(), want, "id {id}");
        }
        assert_eq!(Platform::of_stem("xboxone_ui_core_000a_0"), Some(Platform::XboxOne));
        assert_eq!(Platform::of_stem("ps4_ui_core_000a_0"), Some(Platform::Ps4));
        assert_eq!(Platform::of_stem("ps4x_ui_core_000a_0"), None);
        for p in Platform::ALL {
            assert_eq!(Platform::from_id(p.id()), Some(p));
        }
    }

    #[test]
    fn minimal_accepted_manifest_is_2a_00() {
        assert_eq!(encode(&[], "").unwrap(), vec![0x2a, 0x00]);
    }

    #[test]
    fn l11_offer_id_is_zero_and_a_nonzero_one_is_detected() {
        // The detector must be able to see a nonzero f1, or the L11 control is a tautology.
        let mut body = encode(&[], "").unwrap();
        let mut p = Vec::new();
        pb_varint(&mut p, 1, 0xB000_0001);
        pb_bytes(&mut p, 2, b"forged");
        let mut forged = Vec::new();
        pb_bytes(&mut forged, 3, &p);
        forged.append(&mut body);
        assert_eq!(nonzero_offer_ids(&forged).unwrap(), vec!["forged".to_string()]);
    }

    #[test]
    fn guid_check_follows_strncmp_over_key_101() {
        assert!(guid_matches_config("", ""));
        assert!(!guid_matches_config("abc", ""));
        assert!(guid_matches_config("abc", "abc"));
        // Beyond 0x25 bytes the client stops comparing.
        let a = "x".repeat(CONFIG_KEY_101_SIZE) + "AAA";
        let b = "x".repeat(CONFIG_KEY_101_SIZE) + "BBB";
        assert!(guid_matches_config(&a, &b));
        // But we refuse to emit one that long, because key 101 could not hold it.
        assert!(encode(&[], &a).is_err());
    }

    #[test]
    fn locale_table_round_trips() {
        assert_eq!(locale_code("en"), Some(1));
        assert_eq!(locale_code("jpn"), Some(6));
        assert_eq!(locale_code("zz"), None);
        assert_eq!(locale_tag(0), Some("neutral"));
        assert_eq!(locale_tag(7), Some("pt"));
    }

    // ── Below here the real 01.29 estate is required; these skip without it. ──

    #[test]
    fn policy_counts_match_the_pinned_estate() {
        let Some(est) = estate() else { return };
        assert_eq!(est.len(), PINNED_ESTATE_FILES);
        assert_eq!(select(&est, &sel(Policy::Newest, None)).len(), PINNED_NEWEST);
        assert_eq!(select(&est, &sel(Policy::All, None)).len(), PINNED_ALL);
        assert_eq!(select(&est, &sel(Policy::SymbolsNewest, None)).len(), PINNED_SYMBOLS_NEWEST);
        assert_eq!(
            select(&est, &sel(Policy::AllGenerations, Some(1))).len(),
            PINNED_ALL_GENERATIONS_EN
        );
    }

    #[test]
    fn legacy_policies_are_byte_identical_to_the_prototype_generator() {
        let Some(est) = estate() else { return };
        for (policy, lang, (len, sha)) in [
            (Policy::Newest, None, GOLDEN_NEWEST),
            (Policy::SymbolsNewest, None, GOLDEN_SYMBOLS_NEWEST),
            (Policy::All, None, GOLDEN_ALL),
        ] {
            let body = encode(&select(&est, &sel(policy, lang)), "").unwrap();
            assert_eq!(body.len(), len, "{policy:?} length");
            assert_eq!(sha256_hex(&body), sha, "{policy:?} sha256");
        }
    }

    #[test]
    fn w2_4_reproduces_the_served_full_en_manifest() {
        let Some(est) = estate() else { return };
        let body = encode(&select(&est, &sel(Policy::AllGenerations, Some(1))), "").unwrap();
        assert_eq!(body.len(), GOLDEN_FULL_EN.0);
        assert_eq!(sha256_hex(&body), GOLDEN_FULL_EN.1);
        // And against the artifact itself when it is on disk.
        if let Ok(served) = std::fs::read(SERVED) {
            assert!(body == served, "generated manifest differs from the served full_en.bin");
        }
    }

    #[test]
    fn engine_registered_ids_cover_both_spellings() {
        let Some(est) = estate() else { return };
        let ids = engine_ids(&est);
        assert_eq!(ids.len(), 3);
        let dropped: Vec<&str> =
            est.iter().filter(|r| ids.contains(&r.id)).map(|r| r.stem.as_str()).collect();
        assert_eq!(dropped.len(), 6, "three _unp3_ files and their _unpatchable_ twins: {dropped:?}");
        assert!(dropped.iter().filter(|s| s.contains("_unpatchable_")).count() == 3);
    }

    #[test]
    fn every_control_passes_on_the_pinned_estate() {
        let Some(_) = estate() else { return };
        for r in controls(DUMP, Some(Path::new(SERVED))).unwrap() {
            assert!(r.passed, "control failed: {} [{}]", r.name, r.detail);
        }
        // And with the served artifact hidden: the suite must still pass, on the digest row alone.
        let hidden = controls(DUMP, None).unwrap();
        assert!(hidden.iter().any(|r| r.skipped), "the byte-compare row should report SKIP");
        for r in &hidden {
            assert!(r.passed, "control failed without the served file: {} [{}]", r.name, r.detail);
        }
    }
}
