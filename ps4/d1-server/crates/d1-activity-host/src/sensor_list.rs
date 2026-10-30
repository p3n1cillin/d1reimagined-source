// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! Sensor lists used by world tables, mission steps and live control commands.
//!
//! A list may override the launch authority with `@authority MACHINE[,SEQ[,APPLIED]]`
//! and lifetime phase with `@phase N` (-1 through 14, the four-bit bias-1 wire enum).
//! All three callers must honour it: the local machine can change on a slice crossing.

use anyhow::{bail, Context, Result};
use d1_oracle::sensor::{SensorAuthority, SensorClientRef, SensorPhaseUpdate};

/// Clone the launch parameters, replace the clients and apply list-local settings.
/// The lifetime record is emitted separately by the sensor encoder, so exclude its
/// duplicate from the list. Team selection only changes player-bundle A values.
pub fn params_for_list(
    base: &SensorPhaseUpdate,
    group: u32,
    team: i32,
    text: &str,
) -> Result<SensorPhaseUpdate> {
    let mut params = base.clone();
    params.name_hash = group;
    params.clients.clear();
    // The lifetime record's sub differs between worlds (3 in the Highway and First Light, 2 in
    // the Tower). A list naming exactly one lifetime row for `group` names the record; with
    // none, or several, the launch sub stands. A frame whose lifetime name is absent from the
    // world is not applied (sense56: 1 of 100 Tower clients delivered under sub 3).
    let lifetime_subs: Vec<i16> = text
        .lines()
        .filter_map(|l| SensorClientRef::parse_line(l.split('#').next().unwrap_or("").trim()))
        .filter(|c| c.name_hash == group && c.name_i8 == base.name_i8)
        .map(|c| c.name_sub)
        .collect();
    if let [sub] = lifetime_subs.as_slice() {
        params.name_sub = *sub;
    }
    let mut authority_seen = false;
    let mut phase_seen = false;
    for (index, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.split_whitespace().next() == Some("@authority") {
            if authority_seen {
                bail!("duplicate @authority on line {}", index + 1);
            }
            authority_seen = true;
            let value = line.trim_start_matches("@authority").trim();
            params.authority = Some(
                parse_authority(value)
                    .with_context(|| format!("@authority on line {}", index + 1))?,
            );
        } else if line.split_whitespace().next() == Some("@phase") {
            if phase_seen {
                bail!("duplicate @phase on line {}", index + 1);
            }
            phase_seen = true;
            let value = line.trim_start_matches("@phase").trim();
            let phase: i8 = value
                .parse()
                .with_context(|| format!("@phase on line {}: want -1..14", index + 1))?;
            if !(-1..=14).contains(&phase) {
                bail!("@phase on line {}: want -1..14", index + 1);
            }
            params.phase = phase;
        } else if let Some(mut client) = SensorClientRef::parse_line(line) {
            if client.name_hash == group
                && client.name_i8 == params.name_i8
                && client.name_sub == params.name_sub
            {
                continue;
            }
            if let Some((0x8080_22A5, image)) = client.a_value.as_mut() {
                if image.len() >= 0x3C {
                    image[0x38..0x3C].copy_from_slice(&team.to_le_bytes());
                }
            }
            params.clients.push(client);
        }
    }
    Ok(params)
}

fn parse_authority(value: &str) -> Result<SensorAuthority> {
    let fields: Vec<&str> = value.split(',').map(str::trim).collect();
    if !(1..=3).contains(&fields.len()) || fields.iter().any(|f| f.is_empty()) {
        bail!("want MACHINE[,SEQ[,APPLIED]]");
    }
    let machine: u8 = fields[0].parse().context("machine must fit u8")?;
    let seq: u16 = fields
        .get(1)
        .unwrap_or(&"1")
        .parse()
        .context("sequence must fit u16")?;
    let applied = match fields.get(2).copied().unwrap_or("1") {
        "0" => false,
        "1" => true,
        _ => bail!("applied must be 0 or 1"),
    };
    Ok(SensorAuthority::all_to_with(machine, seq, applied))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Synthetic configuration, with no captured client data or executable needed.
    fn base() -> SensorPhaseUpdate {
        SensorPhaseUpdate {
            gen: 7,
            name_hash: 42,
            name_i8: 16,
            name_sub: 3,
            phase: 3,
            tail: Some((8, 9)),
            clients: vec![SensorClientRef::parse_line("42 1 0 1 0").unwrap()],
            authority: Some(SensorAuthority::all_to_with(3, 1, false)),
        }
    }

    #[test]
    fn list_authority_replaces_launch_authority_without_changing_lifetime() {
        let base = base();
        let p = params_for_list(
            &base,
            99,
            -1,
            "# Divide-like synthetic list\n @authority 12, 2, 0 # transfer pending\n99 1 4 1 0\n",
        )
        .unwrap();
        assert_eq!(
            p.authority,
            Some(SensorAuthority::all_to_with(12, 2, false))
        );
        assert_eq!((p.name_hash, p.name_i8, p.name_sub), (99, 16, 3));
        assert_eq!((p.gen, p.phase, p.tail), (base.gen, base.phase, base.tail));
        assert_eq!(p.clients.len(), 1);
        assert_eq!(p.clients[0].name_sub, 4);
        assert_eq!(
            base.authority,
            Some(SensorAuthority::all_to_with(3, 1, false))
        );
    }

    #[test]
    fn missing_override_keeps_launch_authority_and_excludes_lifetime_duplicate() {
        let base = base();
        let p = params_for_list(&base, 99, 0, "99 16 3 1 0\n99 16 4 1 0\n").unwrap();
        assert_eq!(p.authority, base.authority);
        assert_eq!(p.clients.len(), 1);
        assert_eq!(p.clients[0].name_sub, 4);
    }

    #[test]
    fn a_single_lifetime_row_sets_the_record_sub() {
        // The Tower's lifetime record is sub 2; the launch default is 3.
        let p = params_for_list(&base(), 99, 0, "99 16 2 1 0 # lifetime\n99 12 4 1 1\n").unwrap();
        assert_eq!((p.name_hash, p.name_i8, p.name_sub), (99, 16, 2));
        assert_eq!(p.clients.len(), 1);
        assert_eq!(p.clients[0].name_i8, 12);
        // Another group's kind-16 row is an ordinary client and does not move the record.
        let p = params_for_list(&base(), 99, 0, "7 16 2 1 0\n").unwrap();
        assert_eq!((p.name_sub, p.clients.len()), (3, 1));
        // Without a lifetime row the launch sub stands.
        assert_eq!(
            params_for_list(&base(), 99, 0, "99 1 4 1 0")
                .unwrap()
                .name_sub,
            3
        );
    }

    #[test]
    fn authority_optional_fields_have_the_documented_defaults() {
        for (line, seq, applied) in [
            ("@authority 12", 1, true),
            ("@authority\t12,2", 2, true),
            ("@authority 12,2,0", 2, false),
        ] {
            let p = params_for_list(&base(), 99, 0, line).unwrap();
            assert_eq!(
                p.authority,
                Some(SensorAuthority::all_to_with(12, seq, applied))
            );
        }
    }

    #[test]
    fn malformed_authority_never_silently_falls_back_or_truncates() {
        for value in [
            "", "256", "-1", "12,65536", "12,no", "12,2,no", "12,2,2", "12,", "12,,0", "12,2,0,1",
        ] {
            let error =
                params_for_list(&base(), 99, 0, &format!("@authority {value}")).unwrap_err();
            assert!(error.to_string().contains("line 1"), "{value}: {error}");
        }
        assert!(params_for_list(&base(), 99, 0, "@authority 12\n@authority 3").is_err());
    }

    #[test]
    fn both_team_frames_patch_only_the_bundle_team() {
        let image = vec![0xABu8; 0x40];
        let hex: String = image.iter().map(|b| format!("{b:02x}")).collect();
        let list = format!("99 12 0 1 0 0x808022a5 {hex}\n99 4 1 1 0 0x808007d9 {hex}");
        for team in [0i32, -1] {
            let p = params_for_list(&base(), 99, team, &list).unwrap();
            let bundle = &p.clients[0].a_value.as_ref().unwrap().1;
            assert_eq!(&bundle[..0x38], &image[..0x38]);
            assert_eq!(&bundle[0x38..0x3C], &team.to_le_bytes());
            assert_eq!(&bundle[0x3C..], &image[0x3C..]);
            assert_eq!(&p.clients[1].a_value.as_ref().unwrap().1, &image);
        }
    }

    #[test]
    fn phase_override_applies_to_the_generated_lifetime_entry() {
        let b = base();
        let p =
            params_for_list(&b, 99, -1, "@phase 7 # postgame\n99 16 3 1 0\n99 1 4 1 0\n").unwrap();
        assert_eq!(p.phase, 7);
        assert_eq!((p.gen, p.tail, &p.authority), (b.gen, b.tail, &b.authority));
        assert_eq!(p.clients.len(), 1);
        assert_eq!(p.clients[0].name_sub, 4);
        assert_eq!(b.phase, 3);
        for phase in [-1, 0, 14] {
            assert_eq!(
                params_for_list(&b, 99, -1, &format!("@phase {phase}"))
                    .unwrap()
                    .phase,
                phase
            );
        }
    }

    #[test]
    fn malformed_phase_is_rejected_instead_of_ignored_or_wrapped() {
        for value in ["", "15", "-2", "256", "no", "7 8"] {
            let error = params_for_list(&base(), 99, -1, &format!("@phase {value}")).unwrap_err();
            assert!(error.to_string().contains("line 1"), "{value}: {error}");
        }
        assert!(params_for_list(&base(), 99, -1, "@phase 7\n@phase 3").is_err());
    }
}
