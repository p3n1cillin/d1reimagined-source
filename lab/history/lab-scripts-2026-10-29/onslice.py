import sys


def rep(s, old, new):
    assert s.count(old) == 1, (old[:70], s.count(old))
    return s.replace(old, new)


ST = [
("""    OnIncident { incidents: [u32; 4], timeout: Duration },
}
""",
 """    OnIncident { incidents: [u32; 4], timeout: Duration },
    /// `%S|N`: when the client reports slice set S as CURRENT (udt 21), or N seconds after the
    /// previous step. A walk into another bubble (a z-leg) switches on the client's own; this is
    /// the 01.24 mission's `{"on": {"type": 21, "slice_set": 24}}` (The Breach).
    OnSlice { slice: u32, timeout: Duration },
}
"""),
("""    /// Parse `N`, `+N` or `@T|N` (seconds may be fractional).
    pub fn parse(s: &str) -> Option<Self> {
        let secs = |t: &str| t.parse::<f32>().ok().filter(|v| *v >= 0.0).map(Duration::from_secs_f32);
""",
 """    /// Parse `N`, `+N`, `@T|N` or `%S|N` (seconds may be fractional).
    pub fn parse(s: &str) -> Option<Self> {
        let secs = |t: &str| t.parse::<f32>().ok().filter(|v| *v >= 0.0).map(Duration::from_secs_f32);
        if let Some(rest) = s.strip_prefix('%') {
            let (slice, n) = rest.split_once('|')?;
            return Some(Self::OnSlice { slice: slice.trim().parse().ok()?, timeout: secs(n)? });
        }
"""),
("    incidents: Vec<(u32, Instant)>,\n",
 "    incidents: Vec<(u32, Instant)>,\n"
 "    /// The slice set the client last reported as current (udt 21), for [`StepWhen::OnSlice`].\n"
 "    current_slice: Option<u32>,\n"),
("            incidents: Vec::new(),\n",
 "            incidents: Vec::new(),\n            current_slice: None,\n"),
("""        if let ClientMessage::ClientAuthoritativeDataUpdate { struct_image } = msg {
            self.follow_slice_target(struct_image);
        }
""",
 """        if let ClientMessage::ClientAuthoritativeDataUpdate { struct_image } = msg {
            self.track_current_slice(struct_image);
            self.follow_slice_target(struct_image);
        }
"""),
("""            StepWhen::OnIncident { incidents, timeout } => {""",
 """            StepWhen::OnSlice { slice, timeout } => {
                let here = self.current_slice == Some(slice);
                if here {
                    tracing::info!("⭐ mission step {i}: the client is in slice set {slice}");
                }
                here || now.saturating_duration_since(prev) >= timeout
            }
            StepWhen::OnIncident { incidents, timeout } => {"""),
("""    /// Move the region record to the slice set the client targets (see [`Self::on_client_message`]).
""",
 """    /// Record the client's current slice set from udt 21 (`+0xB8` = {10-bit index, name hash}).
    /// The update is a delta: the record is present only when its hash is (not 0 / FNV basis).
    fn track_current_slice(&mut self, image: &[u8]) {
        let u = |o: usize| image.get(o..o + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
        let (Some(index), Some(hash)) = (u(0xB8), u(0xBC)) else { return };
        let index = index & 0x3FF;
        if matches!(hash, 0 | 0x811C_9DC5) || index == 0x3FF || self.current_slice == Some(index) {
            return;
        }
        tracing::info!("⭐ client is now in slice set {index} (hash {hash:#010x}, was {:?})", self.current_slice);
        self.current_slice = Some(index);
    }

    /// Move the region record to the slice set the client targets (see [`Self::on_client_message`]).
"""),
("""    /// ⭐ An incident step fires on the client's report (not on its own echo), else on the timeout.
""",
 """    /// ⭐ A slice step fires when the client reports that slice set as current.
    #[test]
    fn a_slice_step_fires_when_the_client_is_in_that_slice_set() {
        assert_eq!(StepWhen::parse("%24|900"), Some(StepWhen::OnSlice { slice: 24, timeout: Duration::from_secs(900) }));
        let (mut i, now) = instance();
        let mut img = vec![0u8; 0x220];
        img[0xB8..0xBC].copy_from_slice(&24u32.to_le_bytes());
        i.on_client_message(&ClientMessage::ClientAuthoritativeDataUpdate { struct_image: img.clone() }, now);
        assert_eq!(i.current_slice, None, "a current record without its hash is a delta artefact");
        img[0xBC..0xC0].copy_from_slice(&0x4415_15C2u32.to_le_bytes());
        i.on_client_message(&ClientMessage::ClientAuthoritativeDataUpdate { struct_image: img }, now);
        assert_eq!(i.current_slice, Some(24));
    }

    /// ⭐ An incident step fires on the client's report (not on its own echo), else on the timeout.
"""),
]

for root in sys.argv[1:]:
    p = root + '/crates/d1-activity-host/src/state.rs'
    s = open(p, encoding='utf-8').read()
    for o, n in ST:
        s = rep(s, o, n)
    open(p, 'w', encoding='utf-8', newline='').write(s)
    print('ok', root)
