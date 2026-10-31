import sys

root = sys.argv[1]


def patch(p, pairs):
    s = open(p, encoding='utf-8').read()
    for o, n in pairs:
        assert s.count(o) == 1, (p, o[:70], s.count(o))
        s = s.replace(o, n)
    open(p, 'w', encoding='utf-8', newline='').write(s)


patch(root + '/crates/d1-activity-host/src/state.rs', [
("""/// One activity instance, authoritative over its own Activity State.
#[derive(Debug)]
pub struct ActivityInstance {""", """/// Encodes a sensor-client list file into a frame for the given spawn team (`clients GROUP PATH`
/// control lines, see [`ActivityInstance::set_control_builder`]).
#[derive(Clone)]
pub struct ControlBuilder(pub std::sync::Arc<dyn Fn(u32, &str, i32) -> Result<Vec<u8>, String> + Send + Sync>);

impl std::fmt::Debug for ControlBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ControlBuilder")
    }
}

/// One activity instance, authoritative over its own Activity State.
#[derive(Debug)]
pub struct ActivityInstance {"""),
("""    control_file: Option<std::path::PathBuf>,
""", """    control_file: Option<std::path::PathBuf>,
    /// Builds the frame for a `clients GROUP PATH` control line.
    control_builder: Option<ControlBuilder>,
"""),
("""            control_file: None,
""", """            control_file: None,
            control_builder: None,
"""),
("""    fn poll_control(&mut self) {
        let Some(path) = self.control_file.as_ref() else { return };
        let Ok(text) = std::fs::read_to_string(path) else { return };
        let _ = std::fs::remove_file(path);
        for line in text.lines() {
            let f: Vec<&str> = line.split_whitespace().collect();
            let parsed = match f.as_slice() {""", """    /// Let `clients GROUP PATH` control lines publish a sensor-client list at once (the same frame a
    /// mission step builds), for trying states without a new run.
    pub fn set_control_builder(&mut self, b: ControlBuilder) {
        self.control_builder = Some(b);
    }

    fn poll_control(&mut self) {
        let Some(path) = self.control_file.as_ref() else { return };
        let Ok(text) = std::fs::read_to_string(path) else { return };
        let _ = std::fs::remove_file(path);
        for line in text.lines() {
            let f: Vec<&str> = line.split_whitespace().collect();
            if let ["clients", g, file] = f.as_slice() {
                let group = u32::from_str_radix(g.trim_start_matches("0x"), 16).ok();
                match (group, self.control_builder.clone()) {
                    (Some(group), Some(b)) => {
                        let team0 = matches!(&self.sensor_team_frames, Some((_, _, set)) if set.contains(&self.activity_index));
                        match (b.0)(group, file, if team0 { 0 } else { -1 }) {
                            Ok(bytes) => {
                                tracing::info!("⭐ control: sensor clients {file} ({} bytes)", bytes.len());
                                self.sensor_payload = Some(bytes);
                            }
                            Err(e) => tracing::warn!("control: clients {file}: {e}"),
                        }
                    }
                    _ => tracing::warn!("control: ignored line {line:?} (bad group, or no builder)"),
                }
                continue;
            }
            let parsed = match f.as_slice() {"""),
("""                None => tracing::warn!("control: ignored line {line:?} (want `incident TYPE BODYHEX`)"),""",
 """                None => tracing::warn!("control: ignored line {line:?} (want `incident TYPE BODYHEX` or `clients GROUP PATH`)"),"""),
])

patch(root + '/crates/d1-activity-host/src/main.rs', [
("""                    match d1_activity_host::codec::SchemaCodec::build_sensor(&params) {
                        Ok(bytes) => {
                            tracing::info!(
                                "⭐⭐ D1_AH_SENSOR_PHASE={phase} - armed""", """                    // `clients GROUP PATH` control lines (D1_AH_CONTROL_FILE) build a frame the way a
                    // mission step does.
                    {
                        let params = params.clone();
                        control_builder = Some(d1_activity_host::state::ControlBuilder(std::sync::Arc::new(
                            move |group: u32, path: &str, team: i32| -> Result<Vec<u8>, String> {
                                let clients: Vec<d1_oracle::sensor::SensorClientRef> = std::fs::read_to_string(path)
                                    .map_err(|e| e.to_string())?
                                    .lines()
                                    .filter_map(d1_oracle::sensor::SensorClientRef::parse_line)
                                    .filter(|c| !(c.name_hash == group && c.name_i8 == name_i8 && c.name_sub == name_sub))
                                    .collect();
                                let mut p = params.clone();
                                p.name_hash = group;
                                p.clients = clients;
                                for c in p.clients.iter_mut() {
                                    if let Some((0x8080_22A5, img)) = c.a_value.as_mut() {
                                        if img.len() >= 0x3C {
                                            img[0x38..0x3C].copy_from_slice(&team.to_le_bytes());
                                        }
                                    }
                                }
                                d1_activity_host::codec::SchemaCodec::build_sensor(&p).map_err(|e| e.to_string())
                            },
                        )));
                    }
                    match d1_activity_host::codec::SchemaCodec::build_sensor(&params) {
                        Ok(bytes) => {
                            tracing::info!(
                                "⭐⭐ D1_AH_SENSOR_PHASE={phase} - armed"""),
("""            let mut mission_steps: std::collections::HashMap<u16, Vec<d1_activity_host::state::MissionStep>> = std::collections::HashMap::new();
""", """            let mut mission_steps: std::collections::HashMap<u16, Vec<d1_activity_host::state::MissionStep>> = std::collections::HashMap::new();
            let mut control_builder: Option<d1_activity_host::state::ControlBuilder> = None;
"""),
("""                            i.set_control_file(path.into());
""", """                            i.set_control_file(path.into());
                            if let Some(b) = control_builder.clone() {
                                i.set_control_builder(b);
                            }
"""),
])
print('ok')
