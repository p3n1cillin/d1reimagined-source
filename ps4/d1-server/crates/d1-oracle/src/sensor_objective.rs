// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Build 56578 objective handovers, without copying another objective's presentation mode.
//! These are value images for the existing sensor codec, not wire payloads.

use anyhow::{bail, Context, Result};
use std::collections::BTreeSet;

pub const TAG: u32 = 0x8080_2A1B;
pub const SIZE: usize = 0x2F0;

/// Native run divide16: mode 3 presents the title without mode 1's NEW MISSION
/// banner; mode 5 resolves the waypoint but hides the title. Do not call 3 "complete".
pub fn visible(group: u32, marker: u16, machine: u8) -> Result<Vec<u8>> {
    if marker > i16::MAX as u16 || machine >= 0x80 {
        bail!("marker must fit a nonnegative i16; machine must be below 128");
    }
    let mut image = cleared();
    image[0] = 3;
    image[0x40] = 1;
    image[0x4C..0x50].copy_from_slice(&u32::from(machine).to_le_bytes());
    image[0x5C..0x60].copy_from_slice(&1f32.to_le_bytes());
    image[0x60..0x64].copy_from_slice(&group.to_le_bytes());
    image[0x64] = 0x2C;
    image[0x66..0x68].copy_from_slice(&marker.to_le_bytes());
    Ok(image)
}

pub fn cleared() -> Vec<u8> {
    let mut image = vec![0; SIZE];
    image[0x2C0..0x2C4].fill(0xFF);
    image
}

fn number(s: &str) -> Result<u32> {
    let n = if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u32::from_str_radix(h, 16)
    } else {
        s.parse()
    };
    n.with_context(|| format!("invalid unsigned integer {s:?}"))
}

/// Replace only requested A images. Keep list-local authority, B images and
/// non-target rows verbatim. Validate every target before returning any output.
pub fn handover(
    text: &str,
    group: u32,
    active: u16,
    clear: &[u16],
    marker: u16,
    machine: u8,
) -> Result<String> {
    if active > i16::MAX as u16 || clear.iter().any(|x| *x > i16::MAX as u16) {
        bail!("objective indices must fit a nonnegative i16");
    }
    let mut wanted: BTreeSet<u16> = clear.iter().copied().collect();
    if wanted.len() != clear.len() || !wanted.insert(active) {
        bail!("objective indices must be distinct; cannot clear the active objective");
    }
    let active_image = visible(group, marker, machine)?;
    let inactive_image = cleared();
    let mut seen = BTreeSet::new();
    let mut lines = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let (body, comment) = line
            .split_once('#')
            .map_or((line, None), |(b, c)| (b, Some(c)));
        let f: Vec<&str> = body.split_whitespace().collect();
        if f.len() < 3 || f[0].starts_with('@') || number(f[0])? != group || number(f[1])? != 0x20 {
            lines.push(line.to_owned());
            continue;
        }
        let sub = number(f[2])?;
        if sub > u16::MAX as u32 || !wanted.contains(&(sub as u16)) {
            lines.push(line.to_owned());
            continue;
        }
        let sub = sub as u16;
        if !seen.insert(sub) {
            bail!("duplicate objective {sub} on line {}", index + 1);
        }
        if f.len() < 5 || f[3] != "1" || !matches!(f[4], "0" | "1") {
            bail!("objective {sub} must have an A value on line {}", index + 1);
        }
        if f.len() != 5 && (f.len() < 7 || number(f[5])? != TAG) {
            bail!("wrong or incomplete objective A type on line {}", index + 1);
        }
        let image = if sub == active {
            &active_image
        } else {
            &inactive_image
        };
        let mut fields: Vec<String> = f[..5].iter().map(|s| (*s).to_owned()).collect();
        fields.push(format!("{TAG:#010x}"));
        fields.push(crate::control::hexs(image));
        fields.extend(f.iter().skip(7).map(|s| (*s).to_owned()));
        let mut output = fields.join(" ");
        if let Some(c) = comment {
            output.push_str(" #");
            output.push_str(c);
        }
        lines.push(output);
    }
    if seen != wanted {
        bail!(
            "objectives missing from input: {:?}",
            wanted.difference(&seen).collect::<Vec<_>>()
        );
    }
    Ok(lines.join("\n") + "\n")
}

/// Executable synthetic controls; native observations are recorded separately.
pub fn controls() -> Result<()> {
    let source = "# synthetic\n@authority 12,2,0\n42 32 14 1 0\n42 32 17 1 1 0x80802a1b 00 7 cafe # B\n99 1 9 1 0\n";
    let output = handover(source, 42, 17, &[14], 46, 12)?;
    let rows: Vec<_> = output
        .lines()
        .filter_map(crate::sensor::SensorClientRef::parse_line)
        .collect();
    let old = rows
        .iter()
        .find(|c| c.name_sub == 14)
        .context("old objective")?;
    let new = rows
        .iter()
        .find(|c| c.name_sub == 17)
        .context("new objective")?;
    let image = &new.a_value.as_ref().context("new A")?.1;
    if old.a_value.as_ref().context("old A")?.1 != cleared()
        || image != &visible(42, 46, 12)?
        || image[0] != 3
        || image[0x64] != 0x2C
        || image[0x66] != 46
        || image[0x4C] != 12
        || !output.contains("@authority 12,2,0\n")
        || !output.contains("7 cafe # B\n99 1 9 1 0\n")
    {
        bail!("objective handover/preservation control failed");
    }
    for bad in [
        source.replace("42 32 14 1 0\n", ""),
        source.replace("0x80802a1b", "0x9999"),
        format!("{source}42 32 17 1 0\n"),
    ] {
        if handover(&bad, 42, 17, &[14], 46, 12).is_ok() {
            bail!("negative objective control accepted invalid input");
        }
    }
    if handover(source, 42, 17, &[17], 46, 12).is_ok()
        || visible(42, 0xFFFF, 12).is_ok()
        || visible(42, 46, 128).is_ok()
    {
        bail!("range/overlap control failed");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn handover_and_negative_controls() {
        super::controls().unwrap();
    }
}
