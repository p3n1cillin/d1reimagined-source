import sys


def rep(s, old, new, n=1):
    assert s.count(old) == n, (old[:70], s.count(old))
    return s.replace(old, new)


ST = [
("""    pub team_minus1: Vec<u8>,
    pub region: Option<u32>,
}
""",
 """    pub team_minus1: Vec<u8>,
    pub region: Option<u32>,
    /// ⭐ An incident the host sends when the step fires (`incident=TYPE:BODYHEX` on the steps
    /// line): udt 18 is also AH → client. E.g. 1504, the world-loot drop that announces the
    /// starter rifle in The Breach (body = a `0x80800CB6` object).
    pub incident: Option<(u32, Vec<u8>)>,
}
"""),
("    trigger_reports: Vec<((u32, u8, u16), bool)>,\n",
 "    trigger_reports: Vec<((u32, u8, u16), bool)>,\n"
 "    /// Host incidents waiting to be sent (see [`MissionStep::incident`]).\n"
 "    pending_incidents: Vec<(u32, Vec<u8>)>,\n"),
("            trigger_reports: Vec::new(),\n",
 "            trigger_reports: Vec::new(),\n            pending_incidents: Vec::new(),\n"),
("""        let team0 = matches!(&self.sensor_team_frames, Some((_, _, set)) if set.contains(&self.activity_index));
        self.sensor_payload = Some(if team0 { step.team0 } else { step.team_minus1 });
""",
 """        let team0 = matches!(&self.sensor_team_frames, Some((_, _, set)) if set.contains(&self.activity_index));
        if let Some((t, body)) = step.incident.clone() {
            tracing::info!("⭐ mission step {i}: host incident {t} ({} body bytes)", body.len());
            self.pending_incidents.push((t, body));
        }
        self.sensor_payload = Some(if team0 { step.team0 } else { step.team_minus1 });
"""),
("""    /// Arm the timed mission steps (see [`MissionStep`]).
""",
 """    /// The next host incident to send, as udt 18 (`0x8080097D`, 716 bytes): `+0` the count (5 bits),
    /// `+4` the 12-bit types, `+0x68` an optional list (left empty), `+0xD4` the body length in bytes
    /// (9 bits) and `+0xD8` the body (up to 500 bytes).
    pub fn incident_due(&mut self) -> Option<AhPush> {
        if !matches!(self.state, InstanceState::Live) || self.pending_incidents.is_empty() {
            return None;
        }
        let (t, body) = self.pending_incidents.remove(0);
        Some(AhPush::new(self.ah_id, AhMessage::Opaque { user_defined_type: 18, bytes: incident_image(t, &body) }))
    }

    /// Arm the timed mission steps (see [`MissionStep`]).
"""),
("""/// The player trigger an incident-3403 struct image names:""",
 """/// A udt-18 struct image carrying one incident (see [`ActivityInstance::incident_due`]).
pub fn incident_image(incident: u32, body: &[u8]) -> Vec<u8> {
    let mut img = vec![0u8; 0x2CC];
    img[0..4].copy_from_slice(&1u32.to_le_bytes());
    img[4..8].copy_from_slice(&incident.to_le_bytes());
    let n = body.len().min(500);
    img[0xD4..0xD8].copy_from_slice(&(n as u32).to_le_bytes());
    img[0xD8..0xD8 + n].copy_from_slice(&body[..n]);
    img
}

/// The player trigger an incident-3403 struct image names:"""),
("""    /// ⭐ The trigger name in a 3403 report""",
 """    /// ⭐ A host incident goes out as udt 18 with its type and body in place.
    #[test]
    fn a_host_incident_is_a_udt_18_image() {
        let img = incident_image(1504, &[0xAA, 0xBB]);
        assert_eq!(img.len(), 0x2CC);
        assert_eq!(&img[0..8], &[1, 0, 0, 0, 0xE0, 0x05, 0, 0]);
        assert_eq!(&img[0xD4..0xDA], &[2, 0, 0, 0, 0xAA, 0xBB]);
    }

    /// ⭐ The trigger name in a 3403 report"""),
]

for root in sys.argv[1:]:
    p = root + '/crates/d1-activity-host/src/state.rs'
    s = open(p, encoding='utf-8').read()
    for o, n in ST:
        s = rep(s, o, n)
    s = s.replace("team_minus1: vec![b], region: None }", "team_minus1: vec![b], region: None, incident: None }")
    s = s.replace("team_minus1: vec![1], region: Some(112) }", "team_minus1: vec![1], region: Some(112), incident: None }")
    s = rep(s, "            team_minus1: vec![b],\n            region,\n        };", "            team_minus1: vec![b],\n            region,\n            incident: None,\n        };")
    open(p, 'w', encoding='utf-8', newline='').write(s)
    p = root + '/crates/d1-activity-host/src/listener.rs'
    s = open(p, encoding='utf-8').read()
    s = rep(s, """        if let Some(push) = self.instance.membership_due(now) {
            events.push(AhEvent::WantsToPush {
                user_defined_type: push.message.user_defined_type(),
            });
            events.extend(self.try_push(push));
        }
""", """        if let Some(push) = self.instance.membership_due(now) {
            events.push(AhEvent::WantsToPush {
                user_defined_type: push.message.user_defined_type(),
            });
            events.extend(self.try_push(push));
        }
        while let Some(push) = self.instance.incident_due() {
            events.push(AhEvent::WantsToPush {
                user_defined_type: push.message.user_defined_type(),
            });
            events.extend(self.try_push(push));
        }
""")
    open(p, 'w', encoding='utf-8', newline='').write(s)
    p = root + '/crates/d1-activity-host/src/main.rs'
    s = open(p, encoding='utf-8').read()
    s = rep(s, """                            if f.len() != 4 && f.len() != 5 {
                                anyhow::bail!("D1_AH_MISSION_STEPS line {line:?}: want `index seconds sensor_group sensor_file [region]`");
                            }""", """                            if !(4..=6).contains(&f.len()) {
                                anyhow::bail!("D1_AH_MISSION_STEPS line {line:?}: want `index seconds sensor_group sensor_file [region] [incident=TYPE:BODYHEX]`");
                            }
                            // ⭐ incident=TYPE:BODYHEX: a host incident sent when the step fires.
                            let mut incident = None;
                            let mut rest: Vec<&str> = Vec::new();
                            for t in &f[4..] {
                                if let Some(v) = t.strip_prefix("incident=") {
                                    let (ty, hex) = v.split_once(':')
                                        .ok_or_else(|| anyhow::anyhow!("D1_AH_MISSION_STEPS: bad incident {v:?} (want TYPE:BODYHEX)"))?;
                                    let body = (0..hex.len()).step_by(2)
                                        .map(|k| hex.get(k..k + 2).and_then(|b| u8::from_str_radix(b, 16).ok()))
                                        .collect::<Option<Vec<u8>>>()
                                        .ok_or_else(|| anyhow::anyhow!("D1_AH_MISSION_STEPS: bad incident body {hex:?}"))?;
                                    incident = Some((ty.parse::<u32>()?, body));
                                } else {
                                    rest.push(t);
                                }
                            }""")
    s = rep(s, """                            let region = match f.get(4) {""", """                            let region = match rest.first() {""")
    s = rep(s, """                                team_minus1,
                                region,
                            });""", """                                team_minus1,
                                region,
                                incident,
                            });""")
    open(p, 'w', encoding='utf-8', newline='').write(s)
    print('ok', root)
