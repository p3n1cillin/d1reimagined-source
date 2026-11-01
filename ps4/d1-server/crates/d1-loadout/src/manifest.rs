// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! Reading the preserved Bungie D1 world-content manifest.
//!
//! # What the file actually is
//!
//! `manifest/data/world_en_world_sql_content_<hex>.content` is a **ZIP archive with exactly one
//! member**, DEFLATE-compressed, and that member is a **SQLite 3 database** (5.7 MB → 59 MB). It is
//! not a `.content` format of its own [C, measured on the preserved copy].
//!
//! # Three traps in the schema, all of which produce wrong-but-plausible results
//!
//! 1. **`id` is the item hash reinterpreted as a signed 32-bit integer.** `Gjallarhorn` has
//!    `itemHash` 3564229425 and `id` -730737871. Anything that treats `id` as the hash gets a
//!    negative number and finds nothing; anything that filters `id > 0` silently drops half the
//!    game. This module never reads `id` - it parses `itemHash` out of the JSON, which is stored
//!    unsigned. [C, measured: `MIN(id)` = -2146969240.]
//! 2. **The `json` column is *declared* `BLOB` and *stores* `TEXT`.** `CREATE TABLE … [json] BLOB`,
//!    but `typeof(json)` is `'text'` for all 8,609 item rows [C, measured]. A reader that binds
//!    `Vec<u8>` on the strength of the schema gets `Invalid column type Text at index 0` - which is
//!    how this was found. [`definitions`] accepts either, because the declared type is not evidence
//!    about the stored type in SQLite and betting on one of them is a coin flip.
//! 3. **The same item exists under several hashes.** D1 re-issued gear each year; `Gjallarhorn` is
//!    `1274330687` (Year 1) *and* `3564229425` (the Rise of Iron quest version). Looking an item up
//!    by name and taking the first row picks one of them at random. See [`crate::era`].
//!
//! # Provenance
//!
//! Everything here is `static-client-inference` at best - it is Bungie's public API data, four years
//! newer than our build, and it has never been checked against the 01.29 client.

use std::io::Read as _;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use rusqlite::Connection;

use crate::catalogue::{Catalogue, CatalogueEntry, CatalogueHeader};
use crate::era;
use crate::model::{Damage, Slot, Tier};

/// A world-content database, open for reading.
pub struct WorldManifest {
    conn: Connection,
    source: PathBuf,
    /// Decompressed copy on disk, removed on drop. `None` when the caller pointed us at a database
    /// that was already unpacked.
    scratch: Option<PathBuf>,
}

impl Drop for WorldManifest {
    fn drop(&mut self) {
        if let Some(p) = &self.scratch {
            let _ = std::fs::remove_file(p);
        }
    }
}

impl WorldManifest {
    /// Open a `.content` archive (or an already-unpacked SQLite file).
    ///
    /// `scratch_dir` is where the 59 MB database is unpacked; it is removed again on drop.
    pub fn open(path: &Path, scratch_dir: &Path) -> Result<WorldManifest> {
        let magic = {
            let mut f = std::fs::File::open(path)
                .with_context(|| format!("opening manifest {}", path.display()))?;
            let mut buf = [0u8; 16];
            let n = f.read(&mut buf)?;
            buf[..n].to_vec()
        };

        // "SQLite format 3\0" - an already-unpacked database. Open it in place, don't copy 59 MB.
        if magic.starts_with(b"SQLite format 3") {
            let conn = Connection::open_with_flags(
                path,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
            )?;
            return Ok(WorldManifest { conn, source: path.to_path_buf(), scratch: None });
        }
        if !magic.starts_with(b"PK") {
            bail!(
                "{} is neither a ZIP nor a SQLite database (first bytes {:02x?})",
                path.display(),
                &magic[..magic.len().min(8)]
            );
        }

        std::fs::create_dir_all(scratch_dir)
            .with_context(|| format!("creating scratch dir {}", scratch_dir.display()))?;
        let mut zip = zip::ZipArchive::new(std::fs::File::open(path)?)
            .with_context(|| format!("reading {} as a ZIP", path.display()))?;
        if zip.len() != 1 {
            // Not fatal in principle, but it means the file is not the shape we measured, and
            // guessing which member is the database is exactly the kind of silent wrong answer this
            // project bans.
            bail!("expected exactly one member in {}, found {}", path.display(), zip.len());
        }
        let mut member = zip.by_index(0)?;
        let name = member.name()?.to_string();
        let out = scratch_dir.join(format!("{}.sqlite", sanitise(&name)));
        {
            let mut w = std::fs::File::create(&out)
                .with_context(|| format!("creating {}", out.display()))?;
            std::io::copy(&mut member, &mut w)
                .with_context(|| format!("inflating {name}"))?;
        }
        let conn = Connection::open_with_flags(
            &out,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
        )?;
        Ok(WorldManifest { conn, source: path.to_path_buf(), scratch: Some(out) })
    }

    pub fn source(&self) -> &Path {
        &self.source
    }

    /// Table names, sorted. Used by the CLI's `manifest tables` and by the controls.
    pub fn tables(&self) -> Result<Vec<String>> {
        let mut st = self
            .conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")?;
        let rows = st.query_map([], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn row_count(&self, table: &str) -> Result<u64> {
        // Table names cannot be bound as parameters; check it against the real table list rather
        // than interpolating whatever the caller passed.
        if !self.tables()?.iter().any(|t| t == table) {
            bail!("no such table: {table}");
        }
        let n: i64 = self
            .conn
            .query_row(&format!("SELECT COUNT(*) FROM [{table}]"), [], |r| r.get(0))?;
        Ok(n as u64)
    }

    /// Every JSON definition row in `table`, in `id` order.
    ///
    /// Accepts the `json` column as either `TEXT` or `BLOB` - see the module docs, trap 2.
    pub fn definitions(&self, table: &str) -> Result<Vec<serde_json::Value>> {
        if !self.tables()?.iter().any(|t| t == table) {
            bail!("no such table: {table}");
        }
        let mut st = self.conn.prepare(&format!("SELECT json FROM [{table}] ORDER BY id"))?;
        let rows = st.query_map([], |r| {
            use rusqlite::types::ValueRef;
            match r.get_ref(0)? {
                ValueRef::Text(b) | ValueRef::Blob(b) => Ok(b.to_vec()),
                other => Err(rusqlite::Error::InvalidColumnType(
                    0,
                    "json".to_string(),
                    other.data_type(),
                )),
            }
        })?;
        let mut out = Vec::new();
        for (i, r) in rows.enumerate() {
            let bytes = r?;
            let v: serde_json::Value = serde_json::from_slice(&bytes)
                .with_context(|| format!("parsing {table} row {i}"))?;
            out.push(v);
        }
        Ok(out)
    }

    /// Build the catalogue: every item that can be equipped into one of the 17 equip slots.
    ///
    /// The filter is `equippable == true` **and** the item's `bucketTypeHash` resolving to a
    /// [`Slot`]. Both halves are needed: `equippable` alone admits items bound for vault buckets,
    /// and the bucket alone admits non-equippable placeholders.
    pub fn extract_catalogue(&self) -> Result<Catalogue> {
        let defs = self.definitions("DestinyInventoryItemDefinition")?;
        let scanned = defs.len();
        let mut entries = Vec::with_capacity(4096);

        for d in &defs {
            let equippable = d.get("equippable").and_then(|v| v.as_bool()).unwrap_or(false);
            if !equippable {
                continue;
            }
            let bucket_hash = match d.get("bucketTypeHash").and_then(|v| v.as_u64()) {
                Some(h) => h as u32,
                None => continue,
            };
            let slot = match Slot::from_bucket_hash(bucket_hash) {
                Some(s) => s,
                None => continue,
            };
            let hash = match d.get("itemHash").and_then(|v| v.as_u64()) {
                Some(h) => h as u32,
                None => continue,
            };

            let source_hashes: Vec<u32> = d
                .get("sourceHashes")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_u64()).map(|x| x as u32).collect())
                .unwrap_or_default();
            let verdict = era::classify(&source_hashes);

            let damage = d
                .get("damageTypes")
                .and_then(|v| v.as_array())
                .and_then(|a| a.first())
                .and_then(|x| x.as_u64())
                .map(|x| Damage::from_enum_value(x as u32))
                .unwrap_or(Damage::None);

            // `qualityLevel > 0` marks the Year-1 copy of an item. See `era`'s module docs for the
            // measurement and for the two-sided control that keeps this honest.
            let quality_level = d.get("qualityLevel").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

            entries.push(CatalogueEntry {
                hash,
                name: string_of(d, "itemName"),
                description: string_of(d, "itemDescription"),
                tier: Tier::from_tier_type(
                    d.get("tierType").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                ),
                slot,
                bucket_hash,
                item_type_name: string_of(d, "itemTypeName"),
                icon: string_of(d, "icon"),
                class_type: d.get("classType").and_then(|v| v.as_u64()).unwrap_or(3) as u32,
                damage,
                quality_level,
                year_one: quality_level > 0,
                era: verdict.era,
                era_note: verdict.note,
            });
        }

        entries.sort_by_key(|e| (e.slot, std::cmp::Reverse(e.tier), e.name.clone(), e.hash));

        Ok(Catalogue {
            header: CatalogueHeader {
                schema_version: crate::model::SCHEMA_VERSION,
                manifest_version: era::MANIFEST_VERSION.to_string(),
                manifest_file: self
                    .source
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                target_build: era::TARGET_BUILD.to_string(),
                items_scanned: scanned as u64,
                items_kept: entries.len() as u64,
                provenance_warning:
                    "2020 manifest; the target build is 2016. Item hashes are largely stable across \
                     D1's life but this is NOT an era-matched source, and the manifest has no \
                     date-added field. Era verdicts are inference, never observation."
                        .to_string(),
            },
            entries,
        })
    }
}

fn string_of(v: &serde_json::Value, key: &str) -> String {
    v.get(key).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

fn sanitise(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

/// Find `manifest/data/world_<lang>_*.content` by walking up from `start`.
///
/// The crates live in `d1-server/crates/<crate>`, the manifest lives at the repository root, and the
/// working directory depends on whether you ran `cargo test` or the binary. Walking up is the only
/// thing that works from all three.
pub fn find_world_manifest(start: &Path, lang: &str) -> Option<PathBuf> {
    let prefix = format!("world_{lang}_");
    let mut dir = Some(start);
    while let Some(d) = dir {
        let data = d.join("manifest").join("data");
        if data.is_dir() {
            if let Ok(rd) = std::fs::read_dir(&data) {
                let mut hits: Vec<PathBuf> = rd
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| {
                        p.file_name()
                            .and_then(|n| n.to_str())
                            .is_some_and(|n| n.starts_with(&prefix) && n.ends_with(".content"))
                    })
                    .collect();
                hits.sort();
                if let Some(p) = hits.into_iter().next() {
                    return Some(p);
                }
            }
        }
        dir = d.parent();
    }
    None
}

/// The scratch directory used for unpacking, honouring `$CLAUDE_JOB_DIR` / `$XDG_RUNTIME_DIR` before
/// falling back to the system temp dir.
pub fn default_scratch_dir() -> PathBuf {
    for var in ["CLAUDE_JOB_DIR", "XDG_RUNTIME_DIR"] {
        if let Ok(v) = std::env::var(var) {
            if !v.is_empty() {
                return PathBuf::from(v).join("d1-loadout");
            }
        }
    }
    std::env::temp_dir().join("d1-loadout")
}
