// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! `d1-rsat`: inspect the native codec's schema model, check coverage, encode an image and decode
//! a body.
//! Reads the eboot `$D1_EBOOT` names (else the selected build's).

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(about = "Native RSAT codec: schema model, coverage, encode and decode")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Every descriptor reachable from TAG, with every column the encoder reads.
    Schema {
        #[arg(value_parser = parse_u32)]
        tag: u32,
    },
    /// Whether the native encoder implements every field reachable from each TAG.
    Check {
        #[arg(value_parser = parse_u32, required = true)]
        tags: Vec<u32>,
    },
    /// Encode a hex struct image (zero-padded to the struct size) and print bits and payload.
    Encode {
        #[arg(value_parser = parse_u32)]
        tag: u32,
        /// The struct image, hex. Empty encodes the zero image.
        #[arg(default_value = "")]
        hex: String,
        /// A replicated full-state body (leading presence bit), as udt 11 and 21 are sent.
        #[arg(long)]
        replicated: bool,
    },
    /// Decode a wire body (a file, or hex with --hex) and print the bits consumed, whether the
    /// native re-encode reproduces the body, and the image's non-zero bytes.
    Decode {
        #[arg(value_parser = parse_u32)]
        tag: u32,
        /// A file holding the body, or the body in hex with --hex.
        body: String,
        #[arg(long)]
        hex: bool,
        /// A replicated body (leading presence bit), decoded over the zero image.
        #[arg(long)]
        replicated: bool,
    },
    /// Decode many bodies in one start: stdin lines `TAG HEX [AT [FILL]]`, the body decoded as TAG
    /// from bit AT (default 0) into an image of FILL bytes (hex, default 00; two fills tell which
    /// fields a sparse body carries). One JSON object per line on stdout: `{"tag", "at", "end", "len_bits",
    /// "object"}` (end = the reader's absolute bit position after the walk, object = the decoded
    /// struct image in hex) or `{"tag", "error"}`; a walk that fails part-way also gives "end" (where it
    /// stopped) and "partial" (the image as far as it got). For investment messages (args from bit 16) and
    /// for trying a body against candidate tags (`d1-srcp.py investment`).
    DecodeBatch,
}

fn hex_bytes(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok()).collect()
}

/// One `decode-batch` line's JSON answer.
fn decode_batch_line(dec: &d1_rsat::Decoder<'_>, line: &str) -> String {
    let mut it = line.split_whitespace();
    let tag = it.next().map(parse_u32);
    let body = it.next().map(hex_bytes);
    let at = it.next().map(|s| s.parse::<u64>());
    let fill = it.next().map(|s| u8::from_str_radix(s.trim_start_matches("0x"), 16));
    let (tag, wire, at, fill) = match (tag, body, at, fill) {
        (Some(Ok(t)), Some(Some(b)), None, None) => (t, b, 0, 0),
        (Some(Ok(t)), Some(Some(b)), Some(Ok(a)), None) => (t, b, a, 0),
        (Some(Ok(t)), Some(Some(b)), Some(Ok(a)), Some(Ok(f))) => (t, b, a, f),
        _ => return r#"{"error":"bad line: want TAG HEX [AT [FILL]]"}"#.to_string(),
    };
    let hexs = |o: &[u8]| o.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let quote = |e: String| e.replace('\\', "\\\\").replace('"', "'");
    match dec.decode_at_partial(tag, &wire, at, fill) {
        Ok((d, None)) => format!(
            r#"{{"tag":"{:#010x}","at":{},"end":{},"len_bits":{},"object":"{}"}}"#,
            tag,
            at,
            d.bits,
            wire.len() * 8,
            hexs(&d.object)
        ),
        // The walk failed part-way: say why, and keep the image as far as it got ("partial").
        Ok((d, Some(e))) => format!(
            r#"{{"tag":"{:#010x}","error":"{}","at":{},"end":{},"len_bits":{},"partial":"{}"}}"#,
            tag,
            quote(e.to_string()),
            at,
            d.bits,
            wire.len() * 8,
            hexs(&d.object)
        ),
        Err(e) => format!(r#"{{"tag":"{:#010x}","error":"{}"}}"#, tag, quote(e.to_string())),
    }
}

fn parse_u32(s: &str) -> std::result::Result<u32, String> {
    let s = s.trim();
    match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        Some(h) => u32::from_str_radix(h, 16),
        None => s.parse(),
    }
    .map_err(|e| e.to_string())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let schemas = d1_rsat::schemas().map_err(|e| anyhow!("{e}"))?;
    match cli.cmd {
        Cmd::Schema { tag } => {
            print!("{}", d1_rsat::schema::render(schemas, tag).map_err(|e| anyhow!("{e}"))?);
        }
        Cmd::Check { tags } => {
            let mut bad = 0;
            for t in tags {
                match schemas.check(t) {
                    Ok(()) => println!("{t:#010x}  native"),
                    Err(e) => {
                        bad += 1;
                        println!("{t:#010x}  falls back: {e}");
                    }
                }
            }
            if bad > 0 {
                std::process::exit(2);
            }
        }
        Cmd::DecodeBatch => {
            use std::io::{BufRead, Write};
            let dec = d1_rsat::Decoder::new(schemas);
            let out = std::io::stdout();
            let mut out = out.lock();
            for line in std::io::stdin().lock().lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }
                writeln!(out, "{}", decode_batch_line(&dec, &line))?;
            }
        }
        Cmd::Encode { tag, hex, replicated } => {
            let image = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(hex.get(i..i + 2).unwrap_or("x"), 16))
                .collect::<std::result::Result<Vec<u8>, _>>()
                .context("the image is not hex")?;
            let enc = d1_rsat::Encoder::new(schemas);
            let e = if replicated { enc.encode_replicated(tag, &image) } else { enc.encode(tag, &image) }
                .map_err(|e| anyhow!("{e}"))?;
            println!("tag {:#010x} bits {}", e.tag, e.bits);
            println!("{}", e.payload.iter().map(|b| format!("{b:02x}")).collect::<String>());
        }
        Cmd::Decode { tag, body, hex, replicated } => {
            let wire = if hex {
                (0..body.len())
                    .step_by(2)
                    .map(|i| u8::from_str_radix(body.get(i..i + 2).unwrap_or("x"), 16))
                    .collect::<std::result::Result<Vec<u8>, _>>()
                    .context("the body is not hex")?
            } else {
                std::fs::read(&body).with_context(|| format!("reading {body}"))?
            };
            let dec = d1_rsat::Decoder::new(schemas);
            let d = if replicated { dec.decode_replicated(tag, &wire, &[]) } else { dec.decode(tag, &wire) }
                .map_err(|e| anyhow!("{e}"))?;
            let enc = d1_rsat::Encoder::new(schemas);
            let back = if replicated { enc.encode_replicated(tag, &d.object) } else { enc.encode(tag, &d.object) }
                .map_err(|e| anyhow!("{e}"))?;
            println!(
                "tag {:#010x} body {} bytes, {} bits consumed; struct {:#x} bytes; re-encode {}",
                tag,
                wire.len(),
                d.bits,
                d.object.len(),
                if back.payload == wire { "reproduces the body".to_string() } else {
                    format!("differs ({} bits, {} bytes)", back.bits, back.payload.len())
                }
            );
            let nz: Vec<String> = d
                .object
                .chunks(8)
                .enumerate()
                .filter(|(_, c)| c.iter().any(|&b| b != 0))
                .map(|(i, c)| format!("+{:#06x} {}", i * 8, c.iter().map(|b| format!("{b:02x}")).collect::<String>()))
                .collect();
            println!("{} non-zero qwords", nz.len());
            for l in nz.iter().take(64) {
                println!("  {l}");
            }
        }
    }
    Ok(())
}
