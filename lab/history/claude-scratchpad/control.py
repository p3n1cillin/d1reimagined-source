import sys


def rep(s, old, new, n=1):
    assert s.count(old) == n, (old[:70], s.count(old))
    return s.replace(old, new)


for root in sys.argv[1:]:
    p = root + '/crates/d1-activity-host/src/state.rs'
    s = open(p, encoding='utf-8').read()
    s = rep(s, "    pending_incidents: Vec<(u32, Vec<u8>)>,\n",
            "    pending_incidents: Vec<(u32, Vec<u8>)>,\n"
            "    /// ⭐ `D1_AH_CONTROL_FILE`: lines `incident TYPE BODYHEX` queue host incidents on demand\n"
            "    /// (the file is consumed: read, then deleted). A diagnostic lever, like the 01.24 control file.\n"
            "    control_file: Option<std::path::PathBuf>,\n")
    s = rep(s, "            pending_incidents: Vec::new(),\n",
            "            pending_incidents: Vec::new(),\n            control_file: None,\n")
    s = rep(s, """    pub fn incident_due(&mut self) -> Option<AhPush> {
        if !matches!(self.state, InstanceState::Live) || self.pending_incidents.is_empty() {""",
            """    pub fn incident_due(&mut self) -> Option<AhPush> {
        self.poll_control();
        if !matches!(self.state, InstanceState::Live) || self.pending_incidents.is_empty() {""")
    s = rep(s, """    /// Arm the timed mission steps (see [`MissionStep`]).
""", """    /// Use `path` as the control file (see [`ActivityInstance::incident_due`]).
    pub fn set_control_file(&mut self, path: std::path::PathBuf) {
        self.control_file = Some(path);
    }

    fn poll_control(&mut self) {
        let Some(path) = self.control_file.as_ref() else { return };
        let Ok(text) = std::fs::read_to_string(path) else { return };
        let _ = std::fs::remove_file(path);
        for line in text.lines() {
            let f: Vec<&str> = line.split_whitespace().collect();
            let parsed = match f.as_slice() {
                ["incident", t, hex] => t.parse::<u32>().ok().zip(
                    (0..hex.len()).step_by(2)
                        .map(|k| hex.get(k..k + 2).and_then(|b| u8::from_str_radix(b, 16).ok()))
                        .collect::<Option<Vec<u8>>>(),
                ),
                _ => None,
            };
            match parsed {
                Some((t, body)) => {
                    tracing::info!("⭐ control: host incident {t} ({} body bytes)", body.len());
                    self.pending_incidents.push((t, body));
                }
                None if line.trim().is_empty() => {}
                None => tracing::warn!("control: ignored line {line:?} (want `incident TYPE BODYHEX`)"),
            }
        }
    }

    /// Arm the timed mission steps (see [`MissionStep`]).
""")
    open(p, 'w', encoding='utf-8', newline='').write(s)
    p = root + '/crates/d1-activity-host/src/main.rs'
    s = open(p, encoding='utf-8').read()
    s = rep(s, """                        i.set_mission_steps(mission_steps.clone());
""", """                        i.set_mission_steps(mission_steps.clone());
                        if let Ok(path) = std::env::var("D1_AH_CONTROL_FILE") {
                            i.set_control_file(path.into());
                        }
""")
    open(p, 'w', encoding='utf-8', newline='').write(s)
    print('ok', root)
