// SPDX-License-Identifier: GPL-2.0-or-later
//! The Destiny 1 PS4 `.pkg` container format.
//!
//! **This is the single description of that format in the project** (`design/format-description.md`
//! - no format may be described twice). There is deliberately no Kaitai `.ksy` for the container:
//! tooling here needs to *resolve* entries across the generation chain, which is an algorithm, not a
//! layout, so a declarative description could not carry it.
//!
//! # Layout (patch 01.29, CUSA00219, container version 7)
//!
//! ```text
//! 0x0000        0x140-byte header, RSA-2048/SHA-256 signed
//! 0x0800        0x100-byte signature over SHA-256(header[0:0x140])
//! hdr[0xf0]     symbol table   hdr[0xec] x 0x44   SHA-1 @ hdr[0xf4]
//! hdr[0xb8]     entry  table   hdr[0xb4] x 0x10   SHA-1 @ hdr[0xbc]
//! hdr[0xd4]     block  table   hdr[0xd0] x 0x20   SHA-1 @ hdr[0xd8]
//! ...           block payload bytes
//! every region is 0x800-aligned
//! ```
//!
//! Field provenance is the client's own code: `0xd65b0` bounds the entry count, `0xd7c3a` /
//! `0xd7dac` read the table offsets, and the two SHA-1 fields are guarded by the strings
//! `Failed to validate entry table hash` / `…block table hash`. See
//! `historicaldocs/design/package-container-pvp-content.md` for the disassembly.
//!
//! # The generation chain - the part that is an algorithm
//!
//! Every generation of a package ships a **complete** entry *and* block table, but a block record
//! names, in `gen`, the generation whose **file** physically holds the bytes. Older generations are
//! therefore retained as byte stores, which is why the whole chain must always be registered.
//! [`Estate`] performs that resolution.
//!
//! # Block size when decompressed
//!
//! A decompressed block is **at most** [`BLOCK_PLAIN_SIZE`] (`0x40000`) bytes, and usually exactly
//! that. Two independent facts pin the ceiling: the entry `block_off` field is 14 bits of 16-byte
//! units (max `0x3fff * 16` = 262128, i.e. it addresses inside a 256 KiB block and no further), and
//! uncompressed blocks (`flags` bit 0 clear) have a stored size that reaches exactly `0x40000` and
//! never exceeds it.
//!
//! It is a **ceiling, not a guarantee** - `ps4_globals_pvp_0169_3` block 561 consumes all 65479 of
//! its stored bytes at 114688 plain bytes. So a decompressor must terminate on input exhaustion,
//! not on reaching a target size. Entries may also run *past* the end of a block, continuing into
//! the next one.

pub mod manifest;

use sha1::{Digest, Sha1};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

pub const SYM_REC: usize = 0x44;
pub const ENT_REC: usize = 0x10;
pub const BLK_REC: usize = 0x20;

/// Size of a block once decompressed. See the module docs for why this is fixed.
pub const BLOCK_PLAIN_SIZE: usize = 0x4_0000;

/// `flags` bit 0: the stored bytes are Oodle-compressed (LZH/LZHLW).
///
/// Measured over the whole 01.29 corpus (701,164 blocks): 532,600 set (**76.0%**), 168,564 clear.
/// Blocks with the bit clear have a stored size that tops out at exactly [`BLOCK_PLAIN_SIZE`];
/// blocks with it set never reach it. Decompress the former with nothing, the latter with
/// `capture-rig/oodle/` (the client's own decoder, run under emulation).
pub const BLOCK_FLAG_COMPRESSED: u16 = 0x0001;

// ─────────────────────────────────────── tags ───────────────────────────────────────

/// A content tag: `bit31 | (package_id + 0x400) << 13 | entry_index`.
///
/// The 13-bit index field is exactly why `register_package` caps entry counts at `0x2000`. Note
/// what a tag does *not* carry: any locale field.
pub fn make_tag(pkg_id: u16, entry_index: u32) -> u32 {
    0x8000_0000 | (((pkg_id as u32 + 0x400) & 0x3FFF) << 13) | (entry_index & 0x1FFF)
}

/// The package id encoded in a tag.
pub fn tag_pkg(tag: u32) -> i32 {
    (((tag >> 13) & 0x3FFF) as i32) - 0x400
}

/// The entry index encoded in a tag.
pub fn tag_index(tag: u32) -> u32 {
    tag & 0x1FFF
}

// ────────────────────────────────────── records ─────────────────────────────────────

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct Entry {
    pub index: u32,
    /// Tag of this entry's class/definition record. `0xffffffff` marks a raw blob with no class.
    pub class_tag: u32,
    /// `[U]` a bitmask with an embedded small enum - *not* simply an alignment field, despite the
    /// low 16 bits often looking like one. See `historicaldocs/design/package-container-pvp-content.md` §1.7.
    pub u04: u32,
    /// Packed location; use the accessors rather than reading this directly.
    pub loc: u64,
}

impl Entry {
    /// bits 0..13 - index into the block table.
    pub fn block(&self) -> u32 {
        (self.loc & 0x3FFF) as u32
    }
    /// bits 14..27, in 16-byte units - byte offset within the *decompressed* block.
    pub fn block_off(&self) -> u64 {
        ((self.loc >> 14) & 0x3FFF) * 16
    }
    /// bits 28..57 - payload size in bytes.
    pub fn size(&self) -> u64 {
        (self.loc >> 28) & 0x3FFF_FFFF
    }
    /// bits 58..63 - always 0 across 4,713,258 observed entries.
    pub fn high(&self) -> u64 {
        self.loc >> 58
    }
    /// The predicate the client itself tests at `0xd865f`: does this package supply the entry?
    pub fn has_data(&self) -> bool {
        self.loc & 0x03FF_FFFF_0000_0000 != 0
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct Block {
    pub index: u32,
    /// Byte offset **into the file of generation `gen`**, not necessarily this package's file.
    pub offset: u32,
    pub stored_size: u32,
    pub gen: u16,
    pub flags: u16,
    #[serde(serialize_with = "hex20")]
    pub sha1: [u8; 20],
}

impl Block {
    pub fn compressed(&self) -> bool {
        self.flags & BLOCK_FLAG_COMPRESSED != 0
    }
}

fn hex20<S: serde::Serializer>(v: &[u8; 20], s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&hex(v))
}

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Symbol {
    pub index: u32,
    pub tag: u32,
    pub type_tag: u32,
    pub name: String,
}

// ────────────────────────────────────── platform ────────────────────────────────────

/// The platform a package was built for: the header's `u16` at `+0x02` ([`Header::version`]).
///
/// The numbering is Tiger's own, as `v4nguard/tiger-pkg` reads it (`PackagePlatform`): 7 is PS4 and
/// 8 is Xbox One. Every file of the PS4 estates has 7 there, which is why this field was first taken
/// for a container version. The `u16` at `+0x00` ([`Header::magic`], `0x18`) is the package format
/// version, 24 for Destiny 1 from The Taken King on, on every platform.
///
/// The platform also names the files and the packages the engine registers by itself: `"%s_%s"`
/// with the [`Platform::prefix`] (`ps4_ui_core_000a_0.pkg`, `xboxone_ui_core_000a_0.pkg`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
pub enum Platform {
    Ps4,
    XboxOne,
}

impl Platform {
    pub const ALL: [Platform; 2] = [Platform::Ps4, Platform::XboxOne];

    /// The header's platform id.
    pub fn id(self) -> u16 {
        match self {
            Platform::Ps4 => 7,
            Platform::XboxOne => 8,
        }
    }

    /// The platform for a header's id; `None` for ids no Destiny 1 console estate here uses.
    pub fn from_id(id: u16) -> Option<Platform> {
        Platform::ALL.into_iter().find(|p| p.id() == id)
    }

    /// The prefix of the platform's package file names, without the underscore.
    pub fn prefix(self) -> &'static str {
        match self {
            Platform::Ps4 => "ps4",
            Platform::XboxOne => "xboxone",
        }
    }

    /// The platform whose prefix a package file stem starts with.
    pub fn of_stem(stem: &str) -> Option<Platform> {
        Platform::ALL
            .into_iter()
            .find(|p| stem.strip_prefix(p.prefix()).is_some_and(|rest| rest.starts_with('_')))
    }
}

// ────────────────────────────────────── package ─────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct Header {
    /// The package format version (`0x18`, 24); see [`Platform`].
    pub magic: u16,
    /// The platform id; see [`Platform`] and [`Header::platform`].
    pub version: u16,
    pub id: u16,
    pub flag06: u16,
    pub guid: u64,
    pub timestamp: u32,
    pub build: u32,
    pub revision: u32,
    pub gen: u16,
    pub locale: u16,
    pub build_string: String,
    pub n_ent: u32,
    pub off_ent: u32,
    pub n_blk: u32,
    pub off_blk: u32,
    pub n_sym: u32,
    pub off_sym: u32,
    pub declared_size: u32,
}

impl Header {
    /// Decode the `0x140`-byte header. **The only place these offsets are written down.**
    pub fn parse(raw: &[u8; 0x140]) -> Header {
        Header {
            magic: u16le(raw, 0),
            version: u16le(raw, 2),
            id: u16le(raw, 4),
            flag06: u16le(raw, 6),
            guid: u64le(raw, 8),
            timestamp: u32le(raw, 0x10),
            build: u32le(raw, 0x18),
            revision: u32le(raw, 0x1C),
            gen: u16le(raw, 0x20),
            locale: u16le(raw, 0x22),
            build_string: raw[0x24..0xA4]
                .split(|&c| c == 0)
                .next()
                .map(|s| String::from_utf8_lossy(s).into_owned())
                .unwrap_or_default(),
            n_ent: u32le(raw, 0xB4),
            off_ent: u32le(raw, 0xB8),
            n_blk: u32le(raw, 0xD0),
            off_blk: u32le(raw, 0xD4),
            n_sym: u32le(raw, 0xEC),
            off_sym: u32le(raw, 0xF0),
            declared_size: u32le(raw, 0x13C),
        }
    }

    /// Read **only** the header off disk.
    ///
    /// [`Package::open`] additionally pulls the entry, block and symbol tables, which for the whole
    /// 01.29 estate is 1,647 files worth of seeks that a header-only question does not need. The
    /// manifest generator is exactly such a question, so it uses this.
    ///
    /// ⚠ Requires a file of at least `0x140` bytes. The smallest package in the pinned estate is
    /// 8,192 bytes, so this never diverges from a shorter-prefix reader there.
    pub fn read(path: impl AsRef<Path>) -> anyhow::Result<Header> {
        let mut raw = [0u8; 0x140];
        File::open(path.as_ref())?.read_exact(&mut raw)?;
        Ok(Header::parse(&raw))
    }

    /// The platform the package was built for, if it is one of [`Platform::ALL`].
    pub fn platform(&self) -> Option<Platform> {
        Platform::from_id(self.version)
    }

    /// `gen` and `locale` as the single 32-bit word they occupy at `+0x20`.
    ///
    /// A reader that takes `+0x20` as a `u32` gets this without meaning to. It is also the
    /// supersession key every manifest in the project is ordered by; see `manifest::Row::sort_key`
    /// for why that is kept, and for the controls that measure where it does and does not matter.
    pub fn gen_word(&self) -> u32 {
        self.gen as u32 | (self.locale as u32) << 16
    }
}

/// Decode one `0x44`-byte symbol record. Shared by [`Package::symbols`] and
/// [`read_symbol_names`] so the record layout is described once.
pub fn parse_symbol(rec: &[u8], index: u32) -> Symbol {
    Symbol {
        index,
        tag: u32le(rec, 0),
        type_tag: u32le(rec, 4),
        name: rec[8..8 + 0x3C]
            .split(|&c| c == 0)
            .next()
            .map(|s| String::from_utf8_lossy(s).into_owned())
            .unwrap_or_default(),
    }
}

/// The logical resource names a package declares, read without loading its entry or block tables.
///
/// This is the set name resolution actually scans: the manifest registers packages, and the engine
/// then finds `client_bootstrap_patchable` and friends by walking these tables, newest build
/// timestamp winning. Empty names are dropped, so the result is empty for a package that declares
/// nothing resolvable.
pub fn read_symbol_names(path: impl AsRef<Path>, h: &Header) -> anyhow::Result<Vec<String>> {
    // The same sanity bound the prototype generator used: a symbol count this large is a corrupt
    // or non-package file, not a package with a huge table.
    if h.n_sym == 0 || h.n_sym > 100_000 {
        return Ok(Vec::new());
    }
    let mut f = File::open(path.as_ref())?;
    f.seek(SeekFrom::Start(h.off_sym as u64))?;
    let mut blob = vec![0u8; h.n_sym as usize * SYM_REC];
    // A short read is not fatal: decode whatever whole records arrived.
    let mut got = 0usize;
    loop {
        match f.read(&mut blob[got..]) {
            Ok(0) => break,
            Ok(n) => got += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.into()),
        }
        if got == blob.len() {
            break;
        }
    }
    let mut out = Vec::new();
    for i in 0..got / SYM_REC {
        let name = parse_symbol(&blob[i * SYM_REC..(i + 1) * SYM_REC], i as u32).name;
        if !name.is_empty() {
            out.push(name);
        }
    }
    Ok(out)
}

pub struct Package {
    pub path: PathBuf,
    pub name: String,
    pub file_size: u64,
    pub header: Header,
    raw: [u8; 0x140],
    ent: Vec<u8>,
    blk: Vec<u8>,
    sym: Vec<u8>,
}

fn u16le(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn u32le(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}
fn u64le(b: &[u8], o: usize) -> u64 {
    let mut a = [0u8; 8];
    a.copy_from_slice(&b[o..o + 8]);
    u64::from_le_bytes(a)
}

impl Package {
    pub fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let mut f = File::open(&path)?;
        let file_size = f.metadata()?.len();
        let mut raw = [0u8; 0x140];
        f.read_exact(&mut raw)?;
        let header = Header::parse(&raw);

        let read_tbl = |f: &mut File, off: u32, n: u32, rec: usize| -> anyhow::Result<Vec<u8>> {
            if n == 0 {
                return Ok(Vec::new());
            }
            let mut v = vec![0u8; n as usize * rec];
            f.seek(SeekFrom::Start(off as u64))?;
            f.read_exact(&mut v)?;
            Ok(v)
        };
        let ent = read_tbl(&mut f, header.off_ent, header.n_ent, ENT_REC)?;
        let blk = read_tbl(&mut f, header.off_blk, header.n_blk, BLK_REC)?;
        let sym = read_tbl(&mut f, header.off_sym, header.n_sym, SYM_REC)?;

        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(Self { path, name, file_size, header, raw, ent, blk, sym })
    }

    pub fn id(&self) -> u16 {
        self.header.id
    }
    pub fn gen(&self) -> u16 {
        self.header.gen
    }
    pub fn locale(&self) -> u16 {
        self.header.locale
    }

    pub fn entry(&self, i: u32) -> Option<Entry> {
        let o = i as usize * ENT_REC;
        if o + ENT_REC > self.ent.len() {
            return None;
        }
        Some(Entry {
            index: i,
            class_tag: u32le(&self.ent, o),
            u04: u32le(&self.ent, o + 4),
            loc: u64le(&self.ent, o + 8),
        })
    }

    pub fn entries(&self) -> impl Iterator<Item = Entry> + '_ {
        (0..self.header.n_ent).filter_map(move |i| self.entry(i))
    }

    pub fn block(&self, i: u32) -> Option<Block> {
        let o = i as usize * BLK_REC;
        if o + BLK_REC > self.blk.len() {
            return None;
        }
        let mut sha1 = [0u8; 20];
        sha1.copy_from_slice(&self.blk[o + 12..o + 32]);
        Some(Block {
            index: i,
            offset: u32le(&self.blk, o),
            stored_size: u32le(&self.blk, o + 4),
            gen: u16le(&self.blk, o + 8),
            flags: u16le(&self.blk, o + 10),
            sha1,
        })
    }

    pub fn blocks(&self) -> impl Iterator<Item = Block> + '_ {
        (0..self.header.n_blk).filter_map(move |i| self.block(i))
    }

    pub fn symbols(&self) -> impl Iterator<Item = Symbol> + '_ {
        (0..self.header.n_sym).filter_map(move |i| {
            let o = i as usize * SYM_REC;
            if o + SYM_REC > self.sym.len() {
                return None;
            }
            Some(parse_symbol(&self.sym[o..o + SYM_REC], i))
        })
    }

    /// Verify each table against the SHA-1 the header stores for it - the same check the client
    /// performs before it will register the package.
    pub fn verify_tables(&self) -> TableCheck {
        let cmp = |raw: &[u8], off: usize| -> Option<bool> {
            if raw.is_empty() {
                None
            } else {
                Some(Sha1::digest(raw).as_slice() == &self.raw[off..off + 20])
            }
        };
        TableCheck {
            entry: cmp(&self.ent, 0xBC),
            block: cmp(&self.blk, 0xD8),
            symbol: cmp(&self.sym, 0xF4),
            size: self.header.declared_size as u64 == self.file_size,
        }
    }

    /// The `MANIFEST.sha256`-independent layout check: tables sit where the arithmetic says.
    pub fn layout_ok(&self) -> bool {
        let ru = |x: u32| x.div_ceil(0x800) * 0x800;
        let off_ent = ru(0x1000 + self.header.n_sym * SYM_REC as u32);
        let off_blk = ru(off_ent + self.header.n_ent * ENT_REC as u32);
        (self.header.n_sym == 0 || self.header.off_ent == off_ent)
            && (self.header.n_ent == 0 || self.header.off_blk == off_blk)
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct TableCheck {
    /// `None` = the table is absent, not that it failed.
    pub entry: Option<bool>,
    pub block: Option<bool>,
    pub symbol: Option<bool>,
    pub size: bool,
}

impl TableCheck {
    pub fn all_ok(&self) -> bool {
        self.entry.unwrap_or(true) && self.block.unwrap_or(true) && self.symbol.unwrap_or(true) && self.size
    }
}

// ─────────────────────────────────────── estate ─────────────────────────────────────

/// Every generation of every package, keyed `(id, locale) -> gen -> Package`.
///
/// This exists because a block record's `gen` names the file that physically holds its bytes, so
/// reading one package's content can require opening a different, older file.
pub struct Estate {
    pub packages: Vec<Package>,
    by_id: BTreeMap<(u16, u16), BTreeMap<u16, usize>>,
}

impl Estate {
    pub fn load(dir: impl AsRef<Path>) -> anyhow::Result<Self> {
        let mut paths: Vec<PathBuf> = std::fs::read_dir(dir.as_ref())?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "pkg"))
            .collect();
        paths.sort();
        let mut packages = Vec::new();
        for p in paths {
            packages.push(Package::open(&p)?);
        }
        let mut by_id: BTreeMap<(u16, u16), BTreeMap<u16, usize>> = BTreeMap::new();
        for (i, p) in packages.iter().enumerate() {
            by_id.entry((p.id(), p.locale())).or_default().insert(p.gen(), i);
        }
        Ok(Self { packages, by_id })
    }

    pub fn find(&self, name: &str) -> Option<&Package> {
        self.packages.iter().find(|p| p.name == name)
    }

    pub fn newest(&self, id: u16, locale: u16) -> Option<&Package> {
        let chain = self.by_id.get(&(id, locale))?;
        chain.iter().next_back().map(|(_, &i)| &self.packages[i])
    }

    /// The file that physically holds `block`'s bytes, following the generation chain.
    /// Local blocks belong to this exact file: the engine's unpatchable twins share
    /// (id, locale, gen), so the chain's last-insert winner is not their identity.
    pub fn owner<'a>(&'a self, pkg: &'a Package, block: &Block) -> Option<&'a Package> {
        if block.gen == pkg.gen() {
            return Some(pkg);
        }
        let chain = self.by_id.get(&(pkg.id(), pkg.locale()))?;
        chain.get(&block.gen).map(|&i| &self.packages[i])
    }

    /// Read a block's **stored** bytes - still Oodle-compressed if `flags` bit 0 is set.
    ///
    /// Verifies the block SHA-1, which covers the stored bytes (compressed ones included). There is
    /// no stored digest of the *decompressed* output; see `verify-plain` in the CLI for the check
    /// that does exist.
    pub fn read_block_stored(&self, pkg: &Package, block: &Block) -> anyhow::Result<Vec<u8>> {
        let owner = self
            .owner(pkg, block)
            .ok_or_else(|| anyhow::anyhow!("generation {} missing for {}", block.gen, pkg.name))?;
        let mut f = File::open(&owner.path)?;
        f.seek(SeekFrom::Start(block.offset as u64))?;
        let mut v = vec![0u8; block.stored_size as usize];
        f.read_exact(&mut v)?;
        let got = Sha1::digest(&v);
        if got.as_slice() != block.sha1 {
            anyhow::bail!(
                "block {} sha1 mismatch: stored bytes hash {}, record says {}",
                block.index,
                hex(&got),
                hex(&block.sha1)
            );
        }
        Ok(v)
    }
}

/// Data-independent positive and negative controls for stored-block ownership.
/// Synthetic byte files exercise the existing resolver and SHA-1 verifier; they
/// do not implement or generate another copy of the package format.
pub fn block_owner_controls() -> anyhow::Result<Vec<manifest::ControlRow>> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "d1-pkg-owner-controls-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&root)?;
    // Remove only this invocation's named fixture files, even on an error.
    struct FixtureDir(PathBuf);
    impl Drop for FixtureDir {
        fn drop(&mut self) {
            for name in ["original", "twin", "locale", "older", "newer"] {
                let _ = std::fs::remove_file(self.0.join(name));
            }
            let _ = std::fs::remove_dir(&self.0);
        }
    }
    let _cleanup = FixtureDir(root.clone());
    let fixture = |name: &str, id: u16, locale: u16, gen: u16, bytes: &[u8]| -> anyhow::Result<Package> {
        let path = root.join(name);
        std::fs::write(&path, bytes)?;
        let mut header = Header::parse(&[0; 0x140]);
        header.id = id;
        header.locale = locale;
        header.gen = gen;
        Ok(Package {
            path, name: name.into(), file_size: bytes.len() as u64, header,
            raw: [0; 0x140], ent: Vec::new(), blk: Vec::new(), sym: Vec::new(),
        })
    };
    let original = b"first";
    let twin = b"other";
    let locale = b"local";
    let older = b"prior";
    let packages = vec![
        fixture("original", 0x179, 0, 0, original)?,
        fixture("twin", 0x179, 0, 0, twin)?,
        fixture("locale", 0x179, 1, 0, locale)?,
        fixture("older", 0x172, 0, 0, older)?,
        fixture("newer", 0x172, 0, 1, b"newer")?,
    ];
    // The duplicate key deliberately points at the twin, reproducing the old
    // last-insert-wins condition without duplicating any on-disk parser.
    let by_id = BTreeMap::from([
        ((0x179, 0), BTreeMap::from([(0, 1)])),
        ((0x179, 1), BTreeMap::from([(0, 2)])),
        ((0x172, 0), BTreeMap::from([(0, 3), (1, 4)])),
    ]);
    let estate = Estate { packages, by_id };
    let block = |bytes: &[u8], gen| Block {
        index: 0, offset: 0, stored_size: bytes.len() as u32,
        gen, flags: 0, sha1: Sha1::digest(bytes).into(),
    };
    let reads = |pkg: usize, bytes: &[u8], gen| {
        estate.read_block_stored(&estate.packages[pkg], &block(bytes, gen))
            .is_ok_and(|v| v == bytes)
    };
    let mut rows = Vec::new();
    let mut row = |name: &str, passed| rows.push(manifest::ControlRow {
        name: name.into(), passed, skipped: false,
        detail: "synthetic stored-byte ownership fixture".into(),
    });
    row("local block keeps exact package identity despite duplicate key", reads(0, original, 0));
    row("duplicate twin reads its own stored bytes", reads(1, twin, 0));
    row("locale stays separate from duplicate neutral package", reads(2, locale, 0));
    row("inherited block resolves older generation", reads(4, older, 0));
    row("missing generation rejected", estate.read_block_stored(&estate.packages[4], &block(older, 2)).is_err());
    row("different locale cannot supply missing generation", estate.read_block_stored(&estate.packages[2], &block(locale, 1)).is_err());
    let mut inverted = block(original, 0);
    inverted.sha1[0] ^= 1;
    row("inverted stored digest rejected", estate.read_block_stored(&estate.packages[0], &inverted).is_err());
    std::fs::write(&estate.packages[0].path, b"fault")?;
    row("changed stored payload rejected", estate.read_block_stored(&estate.packages[0], &block(original, 0)).is_err());
    Ok(rows)
}

// ───────────────────────────── decompressed-block verification ──────────────────────

/// `class_tag` of an entry that is a raw blob - image/mesh bytes with no record header.
pub const CLASS_RAW_BLOB: u32 = 0xffff_ffff;

/// Smallest entry that could carry a `u64` length plus any payload at all.
pub const MIN_HEADERED_SIZE: u64 = 16;

/// Why an entry is or is not scored by [`check_self_lengths`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    /// Opens with a `u64` length. **These are the entries that are scored.**
    Headered,
    /// A headerless blob: either `class_tag == 0xffffffff`, or an entry that a same-package
    /// `class_tag` points at (i.e. something in this package describes it).
    RawBlob,
    /// A content-defined record: its `class_tag` names an entry **in this same package** rather than
    /// an external class definition. These open with structure, not a length.
    SelfClassed,
    /// Too small to carry a `u64` length followed by any payload.
    TooSmall,
}

/// One entry's self-length agreement, the check that validates a *decompressed* block.
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct SelfLenCheck {
    pub entry: u32,
    pub class_tag: u32,
    pub offset: u64,
    pub declared: u64,
    pub found: u64,
    pub kind: EntryKind,
    /// Only meaningful for [`EntryKind::Headered`].
    pub agrees: bool,
    /// The entry runs past the end of this block and continues into the next one.
    pub spills: bool,
}

/// Cross-check a decompressed block against the entry table.
///
/// **Why this is a real test of a decompressor.** The block record's SHA-1 covers the *stored*
/// bytes, so it proves nothing about decompressed output. But most payload classes begin with a
/// `u64` length, and the entry table declares that same length in a completely separate structure
/// that no decompressor touches. Agreement is therefore an independent check, and a chance match on
/// any single entry has probability ~2^-64.
///
/// Does this entry's `class_tag` name an entry in its *own* package?
///
/// This is the discriminator between the two populations. A normal typed payload's `class_tag`
/// points at an external class definition, and such payloads open with a `u64` length. A record
/// whose class lives in the same package is content-defined structure - a descriptor, a reference
/// record - and opens with something else.
pub fn is_self_classed(pkg: &Package, e: &Entry) -> bool {
    tag_pkg(e.class_tag) == pkg.id() as i32
        && pkg
            .entry(tag_index(e.class_tag))
            .is_some_and(|t| t.size() != 0 && t.index != e.index)
}

/// Entry indices that some same-package record points at - i.e. described, headerless blobs.
///
/// Derived **purely from the entry table**. This is the property that keeps the scoring honest: the
/// decompressed bytes never influence which entries are excluded, so a broken decoder cannot
/// classify away its own failures.
pub fn blob_entries(pkg: &Package) -> std::collections::BTreeSet<u32> {
    pkg.entries()
        .filter(|e| e.size() != 0 && is_self_classed(pkg, e))
        .map(|e| tag_index(e.class_tag))
        .collect()
}

/// Classes carrying no length header are excluded **from the entry table alone** - never because
/// they happened to disagree, which would be circular, and never from the decompressed bytes, which
/// would let a broken decoder classify away its own failures. See [`blob_entries`].
/// Every entry is returned regardless, so the caller always sees the whole population.
pub fn check_self_lengths(pkg: &Package, block_index: u32, plain: &[u8]) -> Vec<SelfLenCheck> {
    let blobs = blob_entries(pkg);
    let mut out = Vec::new();
    for e in pkg.entries() {
        if e.size() == 0 || e.block() != block_index {
            continue;
        }
        let o = e.block_off() as usize;
        let spills = e.block_off() + e.size() > plain.len() as u64;
        if o + 8 > plain.len() {
            continue;
        }
        let found = u64le(plain, o);
        let kind = if e.class_tag == CLASS_RAW_BLOB || blobs.contains(&e.index) {
            EntryKind::RawBlob
        } else if is_self_classed(pkg, &e) {
            EntryKind::SelfClassed
        } else if e.size() < MIN_HEADERED_SIZE {
            EntryKind::TooSmall
        } else {
            EntryKind::Headered
        };
        out.push(SelfLenCheck {
            entry: e.index,
            class_tag: e.class_tag,
            offset: e.block_off(),
            declared: e.size(),
            found,
            kind,
            agrees: kind == EntryKind::Headered && found == e.size(),
            spills,
        });
    }
    out.sort_by_key(|c| c.offset);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_ownership_positive_and_negative_controls_pass() {
        let rows = block_owner_controls().unwrap();
        assert_eq!(rows.len(), 8);
        for row in rows {
            assert!(row.passed && !row.skipped, "{}", row.name);
        }
    }

    #[test]
    fn tag_round_trips_through_package_id_and_index() {
        // The real tag of entry 106 of package 300 (ps4_investment_globals_012c).
        let t = make_tag(300, 106);
        assert_eq!(t, 0x80a5806a);
        assert_eq!(tag_pkg(t), 300);
        assert_eq!(tag_index(t), 106);
    }

    #[test]
    fn tag_index_field_caps_entry_counts_at_0x2000() {
        // The 13-bit index is exactly why register_package rejects counts above 0x2000.
        assert_eq!(tag_index(make_tag(1, 0x2000)), 0, "0x2000 wraps to 0 in 13 bits");
        assert_eq!(tag_index(make_tag(1, 0x1fff)), 0x1fff);
    }

    /// The real entry 106 of block 234, taken from the 01.29 dump.
    #[test]
    fn entry_loc_unpacks_into_block_offset_and_size() {
        let e = Entry { index: 106, class_tag: 0xffff_ffff, u04: 1, loc: 0 };
        assert!(!e.has_data());

        // block 234, offset 2080 (=130*16), size 12544
        let loc = 234u64 | (130u64 << 14) | (12544u64 << 28);
        let e = Entry { index: 106, class_tag: 0xffff_ffff, u04: 1, loc };
        assert_eq!(e.block(), 234);
        assert_eq!(e.block_off(), 2080);
        assert_eq!(e.size(), 12544);
        assert_eq!(e.high(), 0);
        assert!(e.has_data());
    }

    #[test]
    fn block_offset_field_can_address_exactly_one_plain_block() {
        // 14 bits of 16-byte units reaches 262128, just inside BLOCK_PLAIN_SIZE. This is one of the
        // two facts pinning the decompressed block size.
        let loc = (0x3fffu64 << 14) | (1u64 << 28);
        let e = Entry { index: 0, class_tag: 0, u04: 0, loc };
        assert_eq!(e.block_off(), 262128);
        assert!((e.block_off() as usize) < BLOCK_PLAIN_SIZE);
        assert_eq!(BLOCK_PLAIN_SIZE - e.block_off() as usize, 16);
    }

    // ── Tests below read the real 01.29 dump and skip when it is absent, so the crate still
    //    builds and tests on a machine without the packages.
    const DUMP: &str =
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../../dumps/patch-01.29-CUSA00219/packages");

    fn estate() -> Option<Estate> {
        Path::new(DUMP).is_dir().then(|| Estate::load(DUMP).unwrap())
    }

    #[test]
    fn block_sha1_covers_the_stored_compressed_bytes_not_the_decompressed_ones() {
        // This was got backwards once, and it sent a verification attempt down a dead end: the
        // record's digest is over the bytes as stored, compressed ones included. Nothing in the
        // container stores a digest of decompressed output - hence `check_self_lengths`.
        let Some(est) = estate() else { return };
        let p = est.find("ps4_investment_globals_012c_1.pkg").unwrap();
        let mut compressed = 0;
        for b in p.blocks() {
            est.read_block_stored(p, &b).expect("stored-bytes SHA-1 must match");
            compressed += b.compressed() as usize;
        }
        assert_eq!(p.header.n_blk, 340);
        assert_eq!(compressed, 334, "334 of 340 blocks are Oodle-compressed");
    }

    #[test]
    fn descriptor_entries_name_the_blob_they_describe() {
        // The pairing that makes blob classification decoder-independent.
        let Some(est) = estate() else { return };
        let p = est.find("ps4_investment_globals_012c_1.pkg").unwrap();
        let e111 = p.entry(111).unwrap();
        assert!(is_self_classed(p, &e111), "entry 111 is a descriptor");
        assert_eq!(tag_index(e111.class_tag), 114, "and it names entry 114");
        assert_eq!(e111.size(), 60);
        assert_eq!(p.entry(114).unwrap().size(), 12544, "which is the blob");
        assert!(blob_entries(p).contains(&114));

        // Same pattern at a different size, which is why the rule is not keyed on 60 bytes.
        let q = est.find("ps4_globals_pvp_0169_3.pkg").unwrap();
        let e5 = q.entry(5).unwrap();
        assert!(is_self_classed(q, &e5));
        assert_eq!(tag_index(e5.class_tag), 6);
        assert_eq!(q.entry(6).unwrap().size(), 701440);
        assert!(blob_entries(q).contains(&6));
    }

    #[test]
    fn every_package_in_the_estate_passes_its_own_table_hashes() {
        let Some(est) = estate() else { return };
        assert!(!est.packages.is_empty());
        for p in &est.packages {
            assert!(p.verify_tables().all_ok(), "{} failed its table hashes", p.name);
            assert!(p.layout_ok(), "{} failed layout arithmetic", p.name);
        }
    }

    #[test]
    fn compressed_flag_is_bit_zero() {
        let mk = |flags| Block {
            index: 0, offset: 0, stored_size: 0, gen: 0, flags, sha1: [0; 20],
        };
        assert!(mk(1).compressed());
        assert!(!mk(0).compressed());
    }
}
