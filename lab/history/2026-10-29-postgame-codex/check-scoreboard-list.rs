#[path="F:/d1/public-work/d1reimagined-sense/ps4/d1-server/crates/d1-activity-host/src/sensor_list.rs"]
mod sensor_list;
use d1_oracle::sensor::SensorPhaseUpdate;
fn main() -> anyhow::Result<()> {
 let base=SensorPhaseUpdate{gen:0,name_hash:0x4786c0e0,name_i8:16,name_sub:3,phase:3,tail:None,clients:vec![],authority:None};
 let text=std::fs::read_to_string("F:/d1/d1preservation-main/cfw-staging/agr-postgame-scoreboard.draft.txt")?;
 let params=sensor_list::params_for_list(&base,0x4786c0e0,0,&text)?;
 assert_eq!(params.phase,3);
 assert_eq!(params.clients.len(),21); // lifetime row is separately generated
 let row=params.clients.iter().find(|r|r.name_i8==15&&r.name_sub==0).unwrap();
 assert!(row.has_a&&!row.has_b);
 assert_eq!(row.a_value.as_ref().unwrap().0,0x808008d0);
 let payload=params.encode()?;
 std::fs::write("lab/history/2026-10-29-postgame-codex/scoreboard-full-udt5.bin",&payload)?;
 println!("PASS: actual list parser and frame encoder, phase {}, {} non-lifetime clients, {} bytes",params.phase,params.clients.len(),payload.len());
 Ok(())
}
