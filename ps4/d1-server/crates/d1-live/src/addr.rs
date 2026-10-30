// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The address / value expression language.
//!
//! Three address spaces meet in this project and confusing them is the cheapest way to produce a
//! confident wrong answer:
//!
//! | space | example | who uses it |
//! |---|---|---|
//! | **eboot vaddr** | `0xB471E0` | `d1-eboot`, every design doc, `guest-symbols.json` |
//! | **shadPS4 guest / host** | `0x800B471E0` | the emulator's crash lines, `/proc/<pid>/mem`, this snapshot |
//! | file offset | `0xB4B1E0` | nothing here - the bias is per-segment (ATLAS §4.1) |
//!
//! shadPS4 maps the guest 1:1 into the host, so a captured pointer is directly usable as a host
//! address; the only translation needed is `base` (measured, not assumed).
//!
//! The grammar, deliberately tiny:
//!
//! ```text
//! value := ('u8@'|'u16@'|'u32@'|'u64@'|'i32@') addr | addr
//! addr  := term (('+'|'-') term)*
//! term  := '[' addr ']'      deref u64 in the snapshot
//!        | 'v:' NUM          an eboot vaddr        -> base + NUM
//!        | 'a:' NUM          an absolute address   -> NUM
//!        | 'base' | 'scratch' | 'stop'
//!        | NAME              a guest-symbols.json symbol -> base + vaddr
//!        | NUM               bare: an eboot vaddr (the project's idiom)
//! ```
//!
//! ⚠ **Every resolution is printed.** `0x2815860` → `v:0x2815860 -> abs 0x802815860` appears in the
//! output of every command that resolves one. A resolution nobody saw is a resolution nobody
//! checked, and this project has twice published an address in the wrong space.

use anyhow::{anyhow, bail, Result};

use crate::symbols::Symbols;

/// Everything an expression can refer to.
pub struct Ctx<'a> {
    pub base: u64,
    pub syms: &'a Symbols,
    /// Deref source. `None` while only static resolution is possible (e.g. before a snapshot loads).
    pub mem: Option<&'a dyn Fn(u64) -> Result<u64>>,
    pub scratch: Option<u64>,
    pub stop: Option<u64>,
    /// Human-readable trace of how the last expression resolved.
    pub trace: std::cell::RefCell<Vec<String>>,
}

impl<'a> Ctx<'a> {
    pub fn new(base: u64, syms: &'a Symbols) -> Self {
        Self { base, syms, mem: None, scratch: None, stop: None, trace: Default::default() }
    }
    pub fn take_trace(&self) -> Vec<String> {
        std::mem::take(&mut self.trace.borrow_mut())
    }
}

struct P<'s> {
    s: &'s [u8],
    i: usize,
}

impl<'s> P<'s> {
    fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn eat(&mut self, c: u8) -> bool {
        self.ws();
        if self.i < self.s.len() && self.s[self.i] == c {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn peek(&mut self) -> Option<u8> {
        self.ws();
        self.s.get(self.i).copied()
    }
    fn ident(&mut self) -> String {
        self.ws();
        let start = self.i;
        while self.i < self.s.len()
            && (self.s[self.i].is_ascii_alphanumeric() || self.s[self.i] == b'_')
        {
            self.i += 1;
        }
        String::from_utf8_lossy(&self.s[start..self.i]).to_string()
    }
}

fn parse_num(t: &str) -> Option<u64> {
    let t = t.trim();
    if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        u64::from_str_radix(h, 16).ok()
    } else {
        t.parse().ok()
    }
}

fn term(p: &mut P, ctx: &Ctx) -> Result<u64> {
    match p.peek() {
        Some(b'[') => {
            p.eat(b'[');
            let inner = expr(p, ctx)?;
            if !p.eat(b']') {
                bail!("expected `]`");
            }
            let mem = ctx
                .mem
                .ok_or_else(|| anyhow!("`[…]` needs a snapshot to dereference {inner:#x} through"))?;
            let v = mem(inner)?;
            ctx.trace.borrow_mut().push(format!("  [{inner:#x}] = {v:#x}"));
            Ok(v)
        }
        Some(c) if c.is_ascii_digit() => {
            let start = p.i;
            while p.i < p.s.len()
                && (p.s[p.i].is_ascii_alphanumeric() || p.s[p.i] == b'x' || p.s[p.i] == b'X')
            {
                p.i += 1;
            }
            let t = String::from_utf8_lossy(&p.s[start..p.i]).to_string();
            let n = parse_num(&t).ok_or_else(|| anyhow!("not a number: {t:?}"))?;
            // Bare numbers are EBOOT VADDRS - the idiom of every design doc in this project.
            let abs = ctx.base + n;
            ctx.trace.borrow_mut().push(format!("  v:{n:#x} -> abs {abs:#x}  (bare = eboot vaddr)"));
            Ok(abs)
        }
        Some(_) => {
            let id = p.ident();
            if id.is_empty() {
                bail!("expected a term at byte {}", p.i);
            }
            if p.eat(b':') {
                let start = p.i;
                while p.i < p.s.len() && (p.s[p.i].is_ascii_alphanumeric()) {
                    p.i += 1;
                }
                let t = String::from_utf8_lossy(&p.s[start..p.i]).to_string();
                let n = parse_num(&t).ok_or_else(|| anyhow!("not a number: {t:?}"))?;
                return match id.as_str() {
                    "v" => {
                        let abs = ctx.base + n;
                        ctx.trace.borrow_mut().push(format!("  v:{n:#x} -> abs {abs:#x}"));
                        Ok(abs)
                    }
                    "a" => {
                        ctx.trace.borrow_mut().push(format!("  a:{n:#x} -> abs {n:#x}"));
                        Ok(n)
                    }
                    other => bail!("unknown address-space prefix {other:?} (use `v:` or `a:`)"),
                };
            }
            match id.as_str() {
                "base" => Ok(ctx.base),
                "scratch" => ctx
                    .scratch
                    .ok_or_else(|| anyhow!("`scratch` is only available inside a replay")),
                "stop" => ctx.stop.ok_or_else(|| anyhow!("`stop` is only available inside a replay")),
                name => {
                    let va = ctx.syms.lookup(name).ok_or_else(|| {
                        anyhow!(
                            "unknown symbol {name:?} - it is not in {} (add it there, with a \
                             `source` and a `confidence`; do not hardcode it here)",
                            ctx.syms.path.display()
                        )
                    })?;
                    let abs = ctx.base + va;
                    ctx.trace.borrow_mut().push(format!("  {name} = v:{va:#x} -> abs {abs:#x}"));
                    Ok(abs)
                }
            }
        }
        None => bail!("empty expression"),
    }
}

fn expr(p: &mut P, ctx: &Ctx) -> Result<u64> {
    let mut acc = term(p, ctx)?;
    loop {
        match p.peek() {
            Some(b'+') => {
                p.eat(b'+');
                acc = acc.wrapping_add(term_offset(p, ctx)?);
            }
            Some(b'-') => {
                p.eat(b'-');
                acc = acc.wrapping_sub(term_offset(p, ctx)?);
            }
            _ => return Ok(acc),
        }
    }
}

/// After `+`/`-` a bare number is an **offset**, not a vaddr - `G+0xf8` must not add `base` twice.
fn term_offset(p: &mut P, ctx: &Ctx) -> Result<u64> {
    if let Some(c) = p.peek() {
        if c.is_ascii_digit() {
            let start = p.i;
            while p.i < p.s.len()
                && (p.s[p.i].is_ascii_alphanumeric() || p.s[p.i] == b'x' || p.s[p.i] == b'X')
            {
                p.i += 1;
            }
            let t = String::from_utf8_lossy(&p.s[start..p.i]).to_string();
            return parse_num(&t).ok_or_else(|| anyhow!("not a number: {t:?}"));
        }
    }
    term(p, ctx)
}

/// Resolve an address expression to an absolute address.
pub fn addr(text: &str, ctx: &Ctx) -> Result<u64> {
    let mut p = P { s: text.as_bytes(), i: 0 };
    let v = expr(&mut p, ctx)?;
    p.ws();
    if p.i != p.s.len() {
        bail!("trailing junk in address expression {text:?} at byte {}", p.i);
    }
    Ok(v)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Width {
    U8,
    U16,
    U32,
    U64,
    I32,
}

impl Width {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "u8" => Width::U8,
            "u16" => Width::U16,
            "u32" => Width::U32,
            "u64" => Width::U64,
            "i32" => Width::I32,
            _ => return None,
        })
    }
    pub fn bytes(self) -> usize {
        match self {
            Width::U8 => 1,
            Width::U16 => 2,
            Width::U32 | Width::I32 => 4,
            Width::U64 => 8,
        }
    }
    pub fn decode(self, b: &[u8]) -> u64 {
        match self {
            Width::U8 => b[0] as u64,
            Width::U16 => u16::from_le_bytes(b[..2].try_into().unwrap()) as u64,
            Width::U32 => u32::from_le_bytes(b[..4].try_into().unwrap()) as u64,
            Width::I32 => i32::from_le_bytes(b[..4].try_into().unwrap()) as i64 as u64,
            Width::U64 => u64::from_le_bytes(b[..8].try_into().unwrap()),
        }
    }
}

/// Resolve a **value** expression: a literal, a load (`u32@EXPR`), or an address.
///
/// ⚠ **A bare number here is a LITERAL, not a vaddr** - the one place the two rules differ, and
/// deliberately so. `--arg 0` must mean the integer zero; making it mean `base + 0` produced
/// `rsi = 0x800000000` in the first live use of this tool, which happened to pass a 32-bit compare
/// and would have passed a 64-bit one silently wrong. Addresses in a value position must say so:
/// `v:0x…`, `a:0x…`, a symbol name, or `[…]`.
pub fn value(text: &str, ctx: &Ctx, load: &dyn Fn(u64, Width) -> Result<u64>) -> Result<u64> {
    let t = text.trim();
    if let Some(n) = parse_num(t) {
        ctx.trace.borrow_mut().push(format!("  {t} = literal {n:#x} ({n})"));
        return Ok(n);
    }
    if let Some(neg) = t.strip_prefix('-') {
        if let Some(n) = parse_num(neg) {
            let v = (n as i64).wrapping_neg() as u64;
            ctx.trace.borrow_mut().push(format!("  {t} = literal {v:#x}"));
            return Ok(v);
        }
    }
    if let Some((w, rest)) = text.split_once('@') {
        let width = Width::parse(w.trim())
            .ok_or_else(|| anyhow!("unknown width {w:?} - use u8/u16/u32/i32/u64"))?;
        let a = addr(rest, ctx)?;
        let v = load(a, width)?;
        ctx.trace.borrow_mut().push(format!("  {w}@{a:#x} = {v:#x}"));
        return Ok(v);
    }
    addr(text, ctx)
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    /// The project address book is a derived input that isn't in the repository
    /// (`ps4/capture-rig/guest-symbols.json`, or `$D1_GUEST_SYMBOLS`). These tests skip without it, as
    /// on CI; a file that is there but doesn't load still fails them.
    fn symbols() -> Option<Symbols> {
        let path = crate::symbols::default_path();
        if !path.exists() {
            eprintln!("skipped: needs the guest symbol file {} (or set D1_GUEST_SYMBOLS)", path.display());
            return None;
        }
        Some(Symbols::open_default().unwrap())
    }

    fn ctx(syms: &Symbols) -> Ctx<'_> {
        let mut c = Ctx::new(0x8_0000_0000, syms);
        // A fake memory: `[0x802815860] = 0x204eda280`, everything else 0.
        let f: &dyn Fn(u64) -> Result<u64> =
            Box::leak(Box::new(|a: u64| Ok(if a == 0x8_0281_5860 { 0x2_04ed_a280 } else { 0 })));
        c.mem = Some(f);
        c.scratch = Some(0x7000_0000_0000);
        c
    }

    #[test]
    fn bare_number_in_an_ADDRESS_position_is_an_eboot_vaddr() {
        let Some(s) = symbols() else { return };
        let c = ctx(&s);
        assert_eq!(addr("0xB471E0", &c).unwrap(), 0x8_00B4_71E0);
        assert_eq!(addr("v:0xB471E0", &c).unwrap(), 0x8_00B4_71E0);
        assert_eq!(addr("a:0xB471E0", &c).unwrap(), 0x00B4_71E0);
    }

    /// The footgun that bit on first live use: `--arg 0` must be the integer zero, never `base+0`.
    #[test]
    fn bare_number_in_a_VALUE_position_is_a_literal() {
        let Some(s) = symbols() else { return };
        let c = ctx(&s);
        let load = |_a: u64, _w: Width| -> Result<u64> { Ok(0) };
        assert_eq!(value("0", &c, &load).unwrap(), 0);
        assert_eq!(value("0x1234", &c, &load).unwrap(), 0x1234);
        // …but an address in a value position still resolves as one.
        assert_eq!(value("v:0x10", &c, &load).unwrap(), 0x8_0000_0010);
    }

    #[test]
    fn deref_then_offset_does_not_add_base_twice() {
        let Some(s) = symbols() else { return };
        let c = ctx(&s);
        // `[v:0x2815860]` is a heap pointer; `+0xf8` must be a plain offset on it.
        assert_eq!(addr("[v:0x2815860]+0xf8", &c).unwrap(), 0x2_04ed_a280 + 0xf8);
    }

    #[test]
    fn symbols_resolve_through_the_project_address_book() {
        let Some(s) = symbols() else { return };
        let c = ctx(&s);
        assert_eq!(addr("object_registry_ptr", &c).unwrap(), 0x8_026A_2040);
        assert!(addr("no_such_symbol", &c).is_err());
    }

    /// The `--abs` axis, stated as an assertion rather than as prose: the SAME number must resolve
    /// to two DIFFERENT places depending on which space it was declared in. If these two ever agree
    /// the base is zero and every "heap pointer" reading in the tree is unfalsifiable.
    #[test]
    fn abs_and_vaddr_are_not_interchangeable() {
        let Some(s) = symbols() else { return };
        let c = ctx(&s);
        let n = "0x204eda280"; // a heap pointer, of the shape `d1guest --abs` exists for
        assert_ne!(addr(n, &c).unwrap(), addr(&format!("a:{n}"), &c).unwrap());
        assert_eq!(addr(&format!("a:{n}"), &c).unwrap(), 0x2_04ed_a280);
        assert_eq!(addr(n, &c).unwrap(), 0x8_0000_0000 + 0x2_04ed_a280);
    }
}
