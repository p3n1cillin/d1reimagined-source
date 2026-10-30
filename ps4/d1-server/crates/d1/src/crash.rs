// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! `d1 crash` - harvest and SYMBOLIZE the guest's own crash report.
//!
//! Port of `capture-rig/harvest-crash.py`. Destiny ships a crash reporter: on a fatal halt it
//! writes, into `~/.local/share/shadPS4/temp/CUSA00219/temp/crash_folder/orbis/crash_folder_<stamp>/`:
//!
//! ```text
//! crash_info.txt        guest backtrace + the HALT REASON + `state:: bootflow:<gate>`
//! bug_screenshot.bmp    the exact frame at the moment of the halt
//! report_complete.txt   the reporter's own progress log
//! active_packages.txt   which content packages were mounted
//! ```
//!
//! This is the single richest signal in the project and it is **destroyed by the next run** - the
//! folder is recreated each boot. Harvest it immediately, into the run directory, or lose it.
//!
//! `crash_info.txt`'s stack is bare guest vaddrs with `<unknown>` for every frame. Symbolization
//! resolves each address to its enclosing function by **ANCHORING on the function prologue**
//! (`push rbp; mov rbp, rsp`), never a guessed `va-N` - a guessed anchor produces plausible wrong
//! frames, which is the worst failure this project has. The function is then named from the
//! diagnostic string literals it references via `lea reg, [rip+disp32]`, the only naming signal a
//! stripped PS4 eboot offers.
//!
//! ## Named divergences from the prototype, all deliberate
//!
//! * Default run selection is the CLI's newest *timestamp-named* run (same rule as `d1 gate`),
//!   not the prototype's newest-by-mtime pick, which a scratch directory could win.
//! * An explicit run that does not exist is refused loudly (the prototype died in `copytree`
//!   with a traceback).
//! * A missing or truncated eboot is refused loudly before symbolizing anything: a short image
//!   would render every frame as "no prologue", which reads exactly like a real answer.
//! * A prologue match below the image base (file offset < the 0x4000 bias, i.e. inside the SELF
//!   header) is treated as no-prologue instead of producing the prototype's negative `fn`. An
//!   anchor outside the text is a guessed anchor.
//! * The prototype's `--stack` accepted negative values and rendered them in Python's signed hex;
//!   negative guest vaddrs are refused here.
//!
//! Everything else - window sizes, extent bounds, string filters, output bytes - is verbatim; the
//! executed golden diff against the Python over real crash addresses is byte-identical (see the
//! port record).

use std::path::{Path, PathBuf};

use iced_x86::{Decoder, DecoderOptions, Instruction, Mnemonic, Register};

/// File offset = guest vaddr + BIAS for the eboot image dump.
pub const BIAS: usize = 0x4000;
/// `push rbp; mov rbp, rsp` - the plain-frame prologue the anchor rule keys on.
pub const PROLOGUE: [u8; 4] = [0x55, 0x48, 0x89, 0xe5];
/// The eboot text extent; addresses outside it are host frames.
pub const TEXT_LO: u64 = 0x1000;
pub const TEXT_HI: u64 = 0x1900000;
/// How far back from the queried address a prologue may be.
pub const MAX_BACK: usize = 0x6000;
/// How far forward from the prologue the string scan disassembles.
pub const SCAN_SPAN: usize = 0x900;

/// One symbolized frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Symbolized {
    /// Not resolvable to a function; `note` says why in the prototype's exact words.
    Note { va: u64, note: &'static str },
    /// Anchored on a real prologue at `fn_va`, `off` bytes in.
    Frame { va: u64, fn_va: u64, off: u64, strings: Vec<String> },
}

/// Ascend from the cwd and the executable for the repository root - the directory holding both
/// `capture-rig/` and `runs/`. Keyed on the two DIRECTORIES, deliberately never on any script
/// file, so archiving the Python tools cannot break discovery (same contract as
/// `d1-trace/src/controls.rs::find_repo_root`).
pub fn find_repo_root() -> Option<PathBuf> {
    let is_root = |p: &Path| p.join("capture-rig").is_dir() && p.join("runs").is_dir();
    if let Ok(repo) = std::env::var("D1_REPO") {
        let p = PathBuf::from(repo);
        if is_root(&p) {
            return Some(p);
        }
    }
    let mut starts: Vec<PathBuf> = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        starts.push(cwd);
    }
    if let Ok(exe) = std::env::current_exe() {
        starts.push(exe);
    }
    for start in starts {
        let mut cur: Option<&Path> = Some(&start);
        while let Some(p) = cur {
            if is_root(p) {
                return Some(p.to_path_buf());
            }
            cur = p.parent();
        }
    }
    None
}

/// Read an eboot image, refusing a short one loudly. A truncated image would make every frame
/// symbolize as "no prologue", which is indistinguishable from a real answer - the exact shape of
/// silent failure this project keeps paying for.
pub fn load_eboot_from(path: &Path) -> Result<Vec<u8>, String> {
    let data = std::fs::read(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    let need = TEXT_HI as usize + BIAS;
    if data.len() < need {
        return Err(format!(
            "{} is {} bytes, smaller than the text extent + bias ({need}); a truncated eboot \
             would symbolize every frame as 'no prologue', which reads exactly like a real \
             answer - refusing",
            path.display(),
            data.len()
        ));
    }
    Ok(data)
}

/// Locate and load the selected build's eboot (`capture-rig/builds.json`, `$D1_BUILD`).
pub fn load_eboot() -> Result<Vec<u8>, String> {
    let root = find_repo_root().ok_or(
        "could not locate the repository root (a directory holding both capture-rig/ and runs/); \
         set D1_REPO",
    )?;
    load_eboot_from(&root.join(&d1_build::selected().eboot))
}

/// Load the eboot only if some address actually needs it (the prototype loaded lazily, so a
/// backtrace of pure host frames symbolizes without an eboot on disk). Returns an empty buffer
/// when nothing in `addrs` is inside the text extent; `symbolize` never consults the data for
/// out-of-extent addresses.
pub fn load_eboot_if_needed(addrs: &[u64]) -> Result<Vec<u8>, String> {
    if addrs.iter().any(|a| (TEXT_LO..TEXT_HI).contains(a)) {
        load_eboot()
    } else {
        Ok(Vec::new())
    }
}

/// The nearest plain-frame prologue at or before `va`, within [`MAX_BACK`] - the anchor rule.
/// Returns the function's guest vaddr. A match below the image base (inside the SELF header)
/// returns None: that would be an anchor outside the text, i.e. a guess.
pub fn find_prologue(data: &[u8], va: u64) -> Option<u64> {
    let hi = (va as usize).checked_add(BIAS)?;
    let lo = hi.saturating_sub(MAX_BACK);
    let end = std::cmp::min(data.len(), hi + 4);
    if lo + 4 > end {
        return None;
    }
    let window = &data[lo..end];
    let mut found: Option<usize> = None;
    for i in 0..=(window.len() - 4) {
        if window[i..i + 4] == PROLOGUE {
            found = Some(lo + i);
        }
    }
    match found {
        Some(idx) if idx >= BIAS => Some((idx - BIAS) as u64),
        _ => None,
    }
}

/// Printable C string at a guest vaddr, or None. Verbatim from the prototype: NUL within 160
/// bytes, at least 4 chars, all printable ASCII.
pub fn cstring_at(data: &[u8], va: u64) -> Option<String> {
    let o = (va as usize).checked_add(BIAS)?;
    if o >= data.len() {
        return None;
    }
    let limit = std::cmp::min(data.len(), o + 160);
    let end = data[o..limit].iter().position(|&b| b == 0)? + o;
    let s = &data[o..end];
    if s.len() < 4 || !s.iter().all(|&c| (32..127).contains(&c)) {
        return None;
    }
    Some(s.iter().map(|&b| b as char).collect())
}

/// Diagnostic string literals a function reaches via `lea reg, [rip+disp32]` - the only naming
/// signal a stripped eboot offers. Disassembles FORWARD from the prologue, so the instruction
/// stream is aligned by construction, and stops where the decoder stops (an undecodable byte),
/// exactly as the prototype's capstone loop did.
///
/// Port fidelity note: the prototype contains `if ins.mnemonic == "ret": break` AFTER
/// `if ins.mnemonic != "lea": continue`, so the break is dead code and the scan always runs the
/// whole span. Preserved exactly - this is a port, not a redesign.
pub fn strings_referenced(data: &[u8], fn_va: u64) -> Vec<String> {
    let start = match (fn_va as usize).checked_add(BIAS) {
        Some(s) if s < data.len() => s,
        _ => return Vec::new(),
    };
    let end = std::cmp::min(data.len(), start + SCAN_SPAN);
    let mut decoder = Decoder::with_ip(64, &data[start..end], fn_va, DecoderOptions::NONE);
    let mut ins = Instruction::default();
    let mut out: Vec<String> = Vec::new();
    while decoder.can_decode() {
        decoder.decode_out(&mut ins);
        if ins.is_invalid() {
            break; // capstone stops at the first undecodable byte; so do we
        }
        if ins.mnemonic() != Mnemonic::Lea {
            continue;
        }
        if ins.memory_base() == Register::RIP {
            // iced resolves rip-relative displacement to the absolute target for us; the
            // prototype computed ins.address + ins.size + disp by hand. Same value.
            if let Some(s) = cstring_at(data, ins.memory_displacement64()) {
                if !out.contains(&s) {
                    out.push(s);
                }
            }
        }
    }
    out
}

/// Resolve one guest vaddr: extent check, prologue anchor, string naming. First six strings only,
/// as the prototype capped it.
pub fn symbolize(data: &[u8], va: u64) -> Symbolized {
    if !(TEXT_LO..TEXT_HI).contains(&va) {
        return Symbolized::Note { va, note: "outside the eboot text extent (host frame?)" };
    }
    match find_prologue(data, va) {
        None => Symbolized::Note {
            va,
            note: "no prologue within 0x6000 -- not a plain-frame function",
        },
        Some(fn_va) => {
            let mut strings = strings_referenced(data, fn_va);
            strings.truncate(6);
            Symbolized::Frame { va, fn_va, off: va - fn_va, strings }
        }
    }
}

/// Python's `repr()` for the printable-ASCII strings `cstring_at` admits: single quotes unless the
/// string contains `'` and no `"`; backslash and the quote character escaped.
pub fn py_repr(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut out = String::with_capacity(s.len() + 2);
    out.push(quote);
    for c in s.chars() {
        if c == '\\' {
            out.push_str("\\\\");
        } else if c == quote {
            out.push('\\');
            out.push(c);
        } else {
            out.push(c);
        }
    }
    out.push(quote);
    out
}

/// Render one frame exactly as the prototype printed it. Byte parity is the port's verification
/// instrument; do not restyle.
pub fn render(sym: &Symbolized) -> String {
    match sym {
        Symbolized::Note { va, note } => format!("  0x{va:07X}  {note}"),
        Symbolized::Frame { va, fn_va, off, strings } => {
            let head = format!("  0x{va:07X}  fn 0x{fn_va:07X} + 0x{off:x}");
            if strings.is_empty() {
                format!("{head}   (no string literals -- unnamed)")
            } else {
                let body: Vec<String> =
                    strings.iter().map(|s| format!("             | {}", py_repr(s))).collect();
                format!("{head}\n{}", body.join("\n"))
            }
        }
    }
}

/// Parse the `--stack` list: comma-separated, Python `int(x, 0)` semantics (0x/0o/0b prefixes or
/// decimal, underscores allowed). Negative values are refused - see the divergence list.
pub fn parse_stack_list(list: &str) -> Result<Vec<u64>, String> {
    let mut out = Vec::new();
    for raw in list.split(',') {
        let t: String = raw.trim().chars().filter(|c| *c != '_').collect();
        let t = t.strip_prefix('+').unwrap_or(&t);
        if t.starts_with('-') {
            return Err(format!("'{}': negative values are not guest vaddrs", raw.trim()));
        }
        let (digits, radix) = if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X"))
        {
            (h, 16)
        } else if let Some(o) = t.strip_prefix("0o").or_else(|| t.strip_prefix("0O")) {
            (o, 8)
        } else if let Some(b) = t.strip_prefix("0b").or_else(|| t.strip_prefix("0B")) {
            (b, 2)
        } else {
            (t, 10)
        };
        let v = u64::from_str_radix(digits, radix)
            .map_err(|e| format!("'{}': {e}", raw.trim()))?;
        out.push(v);
    }
    Ok(out)
}

/// The shadPS4 guest crash-folder root for this title.
pub fn crash_root() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
    Ok(PathBuf::from(home).join(".local/share/shadPS4/temp/CUSA00219/temp/crash_folder/orbis"))
}

/// Newest crash folder by mtime, like the prototype. None when the client did not halt (or a
/// later run already wiped it).
pub fn newest_crash_folder(root: &Path) -> Option<PathBuf> {
    let mut cands: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(root)
        .ok()?
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let m = e.metadata().ok()?.modified().ok()?;
            Some((m, e.path()))
        })
        .collect();
    cands.sort_by_key(|(m, _)| *m);
    cands.pop().map(|(_, p)| p)
}

/// Recursive copy, the port of `shutil.copytree` for a crash folder (plain files and
/// directories; the reporter writes nothing else).
pub fn copy_tree(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_tree(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// Guest stack addresses out of `crash_info.txt`: lines of 8..=16 uppercase hex digits followed
/// by ` <unknown>` (the prototype's `^([0-9A-F]{8,16}) <unknown>` multiline regex).
pub fn stack_addrs(info: &str) -> Vec<u64> {
    let mut out = Vec::new();
    for line in info.lines() {
        let n = line
            .bytes()
            .take_while(|c| matches!(c, b'0'..=b'9' | b'A'..=b'F'))
            .count();
        if (8..=16).contains(&n) && line[n..].starts_with(" <unknown>") {
            if let Ok(v) = u64::from_str_radix(&line[..n], 16) {
                out.push(v);
            }
        }
    }
    out
}

/// The four `crash_info.txt` lines that matter most, in the prototype's key order (all matches
/// for the first key, then all for the second, and so on).
const KEY_LINES: [&str; 4] =
    ["halt information:", "state::", "file_server_status:", "environment_string:"];

/// Harvest `crash_folder` into `<run>/guest-crash-report`, symbolize the backtrace, convert the
/// screenshot. Returns the full report text (printed by the caller), byte-compatible with the
/// prototype's stdout.
pub fn harvest(run: &Path, crash_folder: &Path) -> Result<String, String> {
    let dest = run.join("guest-crash-report");
    if dest.exists() {
        std::fs::remove_dir_all(&dest)
            .map_err(|e| format!("clearing {}: {e}", dest.display()))?;
    }
    copy_tree(crash_folder, &dest)
        .map_err(|e| format!("copying {} -> {}: {e}", crash_folder.display(), dest.display()))?;

    let mut out = String::new();
    out.push_str(&format!(
        "harvested {} -> {}\n",
        crash_folder.file_name().unwrap_or_default().to_string_lossy(),
        dest.display()
    ));

    let info_path = dest.join("crash_info.txt");
    let info_bytes = std::fs::read(&info_path)
        .map_err(|e| format!("reading {}: {e} - the crash folder is incomplete", info_path.display()))?;
    let info = String::from_utf8_lossy(&info_bytes);

    // The two lines that matter most, first.
    for key in KEY_LINES {
        for line in info.lines() {
            if line.contains(key) {
                out.push_str(&format!("  {}\n", line.trim()));
            }
        }
    }

    out.push_str("\nSYMBOLIZED GUEST BACKTRACE (innermost first)\n");
    let addrs = stack_addrs(&info);
    let eboot = load_eboot_if_needed(&addrs)?;
    for a in addrs {
        out.push_str(&format!("{}\n", render(&symbolize(&eboot, a))));
    }

    let bmp = dest.join("bug_screenshot.bmp");
    if bmp.exists() {
        let png = dest.join("bug_screenshot.png");
        let converted = std::process::Command::new("convert")
            .arg(&bmp)
            .arg(&png)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if converted {
            out.push_str(&format!("\nframe at halt: {}\n", png.display()));
        } else {
            out.push_str(&format!(
                "\nframe at halt: {}  (install ImageMagick for a PNG)\n",
                bmp.display()
            ));
        }
    }
    Ok(out)
}

/// The prototype's exact no-crash message.
pub const NO_CRASH_MSG: &str =
    "no guest crash folder -- the client did not halt (or a later run already wiped it)";

/// Controls: each returns Err with a reason on failure. Run as `#[test]`s and inside
/// `d1 controls` (exit 2 on failure).
pub mod controls {
    use super::*;

    /// A synthetic image: a plain-frame function at va 0x1000 (file offset 0x5000) whose one
    /// `lea rax, [rip+disp32]` names a control string at va 0x1100.
    fn synthetic_image() -> Vec<u8> {
        let mut img = vec![0u8; 0x9000];
        img[0x5000..0x5004].copy_from_slice(&PROLOGUE); // push rbp; mov rbp, rsp
        // lea rax, [rip+0xf5] at va 0x1004: next ip 0x100b, target 0x1100.
        img[0x5004..0x5008].copy_from_slice(&[0x48, 0x8d, 0x05, 0xf5]);
        img[0x5008..0x500b].copy_from_slice(&[0x00, 0x00, 0x00]);
        img[0x500b] = 0xc3; // ret
        let s = b"resourcerer: control string\0";
        img[0x5100..0x5100 + s.len()].copy_from_slice(s);
        img
    }

    /// C-CRASH-1: the anchor rule end to end on a synthetic image. The queried address resolves
    /// to the REAL prologue (bytes checked at the anchor - never a guessed `va-N`), and the
    /// function is named by the string its `lea` references. Output pinned byte-for-byte.
    pub fn c_crash_synthetic_anchor() -> Result<(), String> {
        let img = synthetic_image();
        let sym = symbolize(&img, 0x1006);
        let Symbolized::Frame { fn_va, off, .. } = &sym else {
            return Err(format!("expected a frame, got {sym:?}"));
        };
        if *fn_va != 0x1000 || *off != 0x6 {
            return Err(format!("anchor wrong: fn {fn_va:#x} off {off:#x}"));
        }
        // The rule itself: the prologue bytes must really be at the anchor.
        let at = *fn_va as usize + BIAS;
        if img[at..at + 4] != PROLOGUE {
            return Err("anchor does not sit on real prologue bytes".into());
        }
        let want = "  0x0001006  fn 0x0001000 + 0x6\n             | 'resourcerer: control string'";
        let got = render(&sym);
        if got != want {
            return Err(format!("render mismatch:\n got: {got:?}\nwant: {want:?}"));
        }
        Ok(())
    }

    /// C-CRASH-2 (inverted): perturb the input, prove the verdict changes. A non-printable byte
    /// in the string demotes the frame to unnamed; destroyed prologue bytes demote it to the
    /// no-prologue note.
    pub fn c_crash_perturbation_changes_output() -> Result<(), String> {
        let img = synthetic_image();
        let clean = render(&symbolize(&img, 0x1006));

        let mut no_string = img.clone();
        no_string[0x5100] = 0x01; // non-printable first byte: cstring_at must refuse
        let got = render(&symbolize(&no_string, 0x1006));
        let want = "  0x0001006  fn 0x0001000 + 0x6   (no string literals -- unnamed)";
        if got != want {
            return Err(format!("unnamed render mismatch:\n got: {got:?}\nwant: {want:?}"));
        }
        if got == clean {
            return Err("perturbing the string did not change the output".into());
        }

        let mut no_prologue = img.clone();
        no_prologue[0x5000] = 0x90; // break the anchor
        let got = render(&symbolize(&no_prologue, 0x1006));
        let want = "  0x0001006  no prologue within 0x6000 -- not a plain-frame function";
        if got != want {
            return Err(format!("no-prologue render mismatch:\n got: {got:?}\nwant: {want:?}"));
        }
        Ok(())
    }

    /// C-CRASH-3: extent boundaries. Host-shaped addresses and out-of-extent guest addresses get
    /// the host-frame note without ever consulting the image; an in-extent address over a
    /// zero-filled image gets the no-prologue note (the empty/truncated-artifact shape, refused
    /// with words rather than a plausible frame).
    pub fn c_crash_extent_notes() -> Result<(), String> {
        let empty: &[u8] = &[];
        for va in [0u64, 0xFFF, 0x1900000, 0x559A6FA472D7] {
            let got = render(&symbolize(empty, va));
            if !got.ends_with("outside the eboot text extent (host frame?)") {
                return Err(format!("va {va:#x}: expected host-frame note, got {got:?}"));
            }
        }
        // Exact prototype rendering for a real host frame, wide address unpadded.
        let got = render(&symbolize(empty, 0x559A6FA472D7));
        if got != "  0x559A6FA472D7  outside the eboot text extent (host frame?)" {
            return Err(format!("host-frame render mismatch: {got:?}"));
        }
        let zeros = vec![0u8; 0x20000];
        let got = render(&symbolize(&zeros, 0x2000));
        if got != "  0x0002000  no prologue within 0x6000 -- not a plain-frame function" {
            return Err(format!("zero-image render mismatch: {got:?}"));
        }
        Ok(())
    }

    /// C-CRASH-4: pinned real-eboot symbolization. The expected blocks were produced by the
    /// Python prototype over addresses from real `runs/*/uploads/ticket_drop-*.bin` crash
    /// reports (executed golden, 2026-08-11) and must keep matching byte-for-byte. Includes an
    /// inverted half: shifting the query by one byte must move the offset, not the anchor.
    pub fn c_crash_pinned_real() -> Result<(), String> {
        let eboot = load_eboot()?;
        let pins: [(u64, &str); 3] = [
            (
                0x94702F,
                "  0x094702F  fn 0x0946D30 + 0x2ff\n             | 'Unknown release_vhalt'",
            ),
            (
                0x346A57,
                "  0x0346A57  fn 0x0346930 + 0x127\n             | 'Go0#b#a'\n             | \"world tag loader only allows failure during load, but resourcerer failed during unload with status {{%s}} ({{'%s'}})\"",
            ),
            (0x750964, "  0x0750964  fn 0x0750940 + 0x24   (no string literals -- unnamed)"),
        ];
        for (va, want) in pins {
            let got = render(&symbolize(&eboot, va));
            if got != want {
                return Err(format!(
                    "pinned symbolization drifted for {va:#x}:\n got: {got:?}\nwant: {want:?}"
                ));
            }
        }
        // Inverted: one byte over moves the offset within the same anchor.
        match symbolize(&eboot, 0x94702F + 1) {
            Symbolized::Frame { fn_va: 0x946D30, off: 0x300, .. } => Ok(()),
            other => Err(format!("va+1 did not track: {other:?}")),
        }
    }

    /// C-CRASH-5: break attempts on the image itself. A truncated eboot, an empty file and a
    /// missing file are all refused LOUDLY by the loader; none reaches symbolization.
    pub fn c_crash_eboot_guard() -> Result<(), String> {
        let dir = std::env::temp_dir().join(format!("d1-crash-ctl-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir: {e}"))?;
        let short = dir.join("short-eboot.bin");
        std::fs::write(&short, [0u8; 100]).map_err(|e| format!("write: {e}"))?;
        let empty = dir.join("empty-eboot.bin");
        std::fs::write(&empty, []).map_err(|e| format!("write: {e}"))?;
        let missing = dir.join("does-not-exist.bin");

        let fail = |name: &str, r: Result<Vec<u8>, String>, needle: &str| match r {
            Ok(_) => Err(format!("{name}: a bad image was accepted")),
            Err(e) if e.contains(needle) => Ok(()),
            Err(e) => Err(format!("{name}: refused for the wrong reason: {e}")),
        };
        let a = fail("truncated", load_eboot_from(&short), "refusing");
        let b = fail("empty", load_eboot_from(&empty), "refusing");
        let c = fail("missing", load_eboot_from(&missing), "reading");
        let _ = std::fs::remove_dir_all(&dir);
        a?;
        b?;
        c
    }

    /// C-CRASH-6: format parity micro-pins. `py_repr` against Python `repr()` ground truth
    /// (captured 2026-08-11), the stack-line matcher against the prototype regex's edges, and
    /// `int(x, 0)` parsing.
    pub fn c_crash_format_parity() -> Result<(), String> {
        let repr_pins = [
            ("plain", "'plain'"),
            ("can't", "\"can't\""),
            ("say \"hi\"", "'say \"hi\"'"),
            ("both ' and \"", "'both \\' and \"'"),
            ("back\\slash", "'back\\\\slash'"),
            ("tick'and\\slash", "\"tick'and\\\\slash\""),
        ];
        for (input, want) in repr_pins {
            let got = py_repr(input);
            if got != want {
                return Err(format!("py_repr({input:?}) = {got:?}, python says {want:?}"));
            }
        }
        let info = "stack:\n000000000094702F <unknown>\n0000559A6FA472D7 <unknown>\nABCDEF1 <unknown>\n00000000000000000 <unknown>\n000000000094702f <unknown>\n";
        let got = stack_addrs(info);
        // 7 hex digits: too short. 17: too long. Lowercase: not the reporter's format.
        if got != vec![0x94702F, 0x559A6FA472D7] {
            return Err(format!("stack matcher drifted: {got:x?}"));
        }
        let got = parse_stack_list("0x94702F, 32 ,0b101,0o17")
            .map_err(|e| format!("parse refused good input: {e}"))?;
        if got != vec![0x94702F, 32, 5, 15] {
            return Err(format!("parse drifted: {got:?}"));
        }
        if parse_stack_list("xyz").is_ok() || parse_stack_list("-5").is_ok() {
            return Err("parse accepted garbage".into());
        }
        Ok(())
    }

    /// Every control, in one list, for `d1 controls` and the test harness.
    pub fn all() -> Vec<(&'static str, fn() -> Result<(), String>)> {
        vec![
            ("C-CRASH-1 prologue anchor + string naming, pinned render", c_crash_synthetic_anchor),
            ("C-CRASH-2 perturbed input changes the verdict", c_crash_perturbation_changes_output),
            ("C-CRASH-3 extent notes; zero image refused in words", c_crash_extent_notes),
            ("C-CRASH-4 pinned real-eboot symbolization (python golden)", c_crash_pinned_real),
            ("C-CRASH-5 truncated/empty/missing eboot refused loudly", c_crash_eboot_guard),
            ("C-CRASH-6 repr/regex/int parity with the prototype", c_crash_format_parity),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c_crash_synthetic_anchor() {
        controls::c_crash_synthetic_anchor().unwrap();
    }
    #[test]
    fn c_crash_perturbation_changes_output() {
        controls::c_crash_perturbation_changes_output().unwrap();
    }
    #[test]
    fn c_crash_extent_notes() {
        controls::c_crash_extent_notes().unwrap();
    }
    /// Needs the real eboot, found through the repository layout (`capture-rig/` and `runs/`, or
    /// `$D1_REPO`). Skipped without that layout, as on CI; with it, a missing or short eboot still fails.
    #[test]
    fn c_crash_pinned_real() {
        if find_repo_root().is_none() {
            eprintln!("skipped: c_crash_pinned_real needs the game executable (repository layout or D1_REPO)");
            return;
        }
        controls::c_crash_pinned_real().unwrap();
    }
    #[test]
    fn c_crash_eboot_guard() {
        controls::c_crash_eboot_guard().unwrap();
    }
    #[test]
    fn c_crash_format_parity() {
        controls::c_crash_format_parity().unwrap();
    }

    /// The harvest driver over a synthetic crash folder: exact output shape, including key-line
    /// ordering (all matches for a key before the next key) and the no-bmp case.
    #[test]
    fn harvest_output_shape() {
        let base = std::env::temp_dir().join(format!("d1-crash-harv-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let cf = base.join("crash_folder_test");
        std::fs::create_dir_all(&cf).unwrap();
        std::fs::write(
            cf.join("crash_info.txt"),
            "stack:\n0000559A6FA472D7 <unknown>\nhalt information:\n  why it halted\n    state:: bootflow:orbit\n    environment_string: live\n",
        )
        .unwrap();
        let run = base.join("run");
        std::fs::create_dir_all(&run).unwrap();
        let got = harvest(&run, &cf).unwrap();
        let want = format!(
            "harvested crash_folder_test -> {}\n  halt information:\n  state:: bootflow:orbit\n  environment_string: live\n\nSYMBOLIZED GUEST BACKTRACE (innermost first)\n  0x559A6FA472D7  outside the eboot text extent (host frame?)\n",
            run.join("guest-crash-report").display()
        );
        assert_eq!(got, want);
        // Break attempt: a crash folder without crash_info.txt fails loudly.
        let bad = base.join("crash_folder_bad");
        std::fs::create_dir_all(&bad).unwrap();
        let err = harvest(&run, &bad).unwrap_err();
        assert!(err.contains("incomplete"), "{err}");
        let _ = std::fs::remove_dir_all(&base);
    }
}
