import sys


def rep(s, old, new):
    assert s.count(old) == 1, (old[:70], s.count(old))
    return s.replace(old, new)


ST = [
("""    OnSlice { slice: u32, timeout: Duration },
}
""",
 """    OnSlice { slice: u32, timeout: Duration },
    /// `#G/K/I|N`: when the client reports player trigger `G/K/I` (incident 3403 naming it, see
    /// [`trigger_report`]), or N seconds after the previous step. Each report is used once, so a
    /// trigger that fires before its step is due (two armed at once) still counts.
    OnTrigger { group: u32, kind: u8, index: u16, timeout: Duration },
}

/// The player trigger an incident-3403 struct image names: its body (a bit buffer at `+0xD8`,
/// read MSB first) holds the trigger's name at bit 303: u32 group hash, the kind in 7 bits biased
/// by one, then the i16 index in offset binary (+0x8000) (run agr-breach13: `0x992BB471/0x1D/34`).
pub fn trigger_report(image: &[u8]) -> Option<(u32, u8, u16)> {
    let body = image.get(0xD8..)?;
    let bits = |at: usize, n: usize| -> Option<u64> {
        let mut v = 0u64;
        for k in at..at + n {
            v = (v << 1) | u64::from((body.get(k / 8)? >> (7 - k % 8)) & 1);
        }
        Some(v)
    };
    let hash = bits(303, 32)? as u32;
    let kind = (bits(335, 7)? as u8).checked_sub(1)?;
    let index = u16::try_from(bits(342, 16)?.checked_sub(0x8000)?).ok()?;
    (!matches!(hash, 0 | 0x811C_9DC5)).then_some((hash, kind, index))
}
"""),
("""        if let Some(rest) = s.strip_prefix('%') {""",
 """        if let Some(rest) = s.strip_prefix('#') {
            let (name, n) = rest.split_once('|')?;
            let mut f = name.split('/');
            let num = |t: &str| {
                let t = t.trim();
                t.strip_prefix("0x").map_or_else(|| t.parse::<u32>().ok(), |h| u32::from_str_radix(h, 16).ok())
            };
            let (group, kind, index) = (num(f.next()?)?, num(f.next()?)?, num(f.next()?)?);
            return Some(Self::OnTrigger { group, kind: kind.try_into().ok()?, index: index.try_into().ok()?, timeout: secs(n)? });
        }
        if let Some(rest) = s.strip_prefix('%') {"""),
("    current_slice: Option<u32>,\n",
 "    current_slice: Option<u32>,\n"
 "    /// Player-trigger reports (incident 3403) by name, and whether a step has used each.\n"
 "    trigger_reports: Vec<((u32, u8, u16), bool)>,\n"),
("            current_slice: None,\n",
 "            current_slice: None,\n            trigger_reports: Vec::new(),\n"),
("""                        tracing::info!("incident {t} reported by the client");
                        self.incidents.push((t, now));
""",
 """                        tracing::info!("incident {t} reported by the client");
                        self.incidents.push((t, now));
                        if t == 3403 {
                            if let Some(name) = trigger_report(struct_image) {
                                if !self.trigger_reports.iter().any(|(n, used)| *n == name && !used) {
                                    tracing::info!("⭐ player trigger {:#010x}/{:#x}/{} reported", name.0, name.1, name.2);
                                    self.trigger_reports.push((name, false));
                                }
                            }
                        }
"""),
("""            StepWhen::OnSlice { slice, timeout } => {""",
 """            StepWhen::OnTrigger { group, kind, index, timeout } => {
                let hit = self.trigger_reports.iter_mut().find(|(n, used)| *n == (group, kind, index) && !used);
                let reported = hit.is_some();
                if let Some((_, used)) = hit {
                    *used = true;
                    tracing::info!("⭐ mission step {i}: player trigger {group:#010x}/{kind:#x}/{index}");
                }
                reported || now.saturating_duration_since(prev) >= timeout
            }
            StepWhen::OnSlice { slice, timeout } => {"""),
("""    /// ⭐ A slice step fires when the client reports that slice set as current.
""",
 """    /// ⭐ The trigger name in a 3403 report, from the run agr-breach13 incident (trigger 34).
    #[test]
    fn a_trigger_report_names_its_trigger() {
        let mut img = vec![0u8; 0x2CC];
        let body = hex_bytes("1009a7015a200000000000403a2000000000004021e8d0000000000000a16d3f4906599a3d1b325768e27a008a04727714");
        img[0xD8..0xD8 + body.len()].copy_from_slice(&body);
        // Bytes +0xD8.. of the decoded struct; the name sits at body bit 303.
        assert_eq!(trigger_report(&img), Some((0x992B_B471, 0x1D, 34)));
        assert_eq!(
            StepWhen::parse("#0x992bb471/0x1d/34|900"),
            Some(StepWhen::OnTrigger { group: 0x992B_B471, kind: 0x1D, index: 34, timeout: Duration::from_secs(900) })
        );
    }

    fn hex_bytes(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|k| u8::from_str_radix(&s[k..k + 2], 16).unwrap()).collect()
    }

    /// ⭐ A slice step fires when the client reports that slice set as current.
"""),
]

for root in sys.argv[1:]:
    p = root + '/crates/d1-activity-host/src/state.rs'
    s = open(p, encoding='utf-8').read()
    for o, n in ST:
        s = rep(s, o, n)
    open(p, 'w', encoding='utf-8', newline='').write(s)
    print('ok', root)
