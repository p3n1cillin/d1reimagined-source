use d1_activity_host::{worlds,sensor_list,codec::SchemaCodec};
use d1_oracle::sensor::SensorPhaseUpdate;
fn main()->anyhow::Result<()> {
 let tower=worlds::lookup("city_tower_default1").expect("Tower KnownWorld missing from linked host library");
 assert_eq!(tower.scenario_tag,0x80c7a005);assert_eq!(tower.checksum,0x24340499);assert_eq!(tower.bubble_count,5);assert_eq!(tower.chosen,&[0,1,2,3,4]);
 let text=std::fs::read_to_string("F:/d1/d1preservation-main/cfw-staging/tower-common-clients.draft.txt")?;
 let base=SensorPhaseUpdate{gen:0,name_hash:0x4786c0e0,name_i8:16,name_sub:3,phase:3,tail:None,clients:vec![],authority:None};
 for team in [0,-1] {
  let p=sensor_list::params_for_list(&base,0x4786c0e0,team,&text)?;
  let frame=SchemaCodec::build_sensor(&p)?;
  println!("PASS: real KnownWorld Tower lookup and list encoder; team{team}, phase{}, {} non-lifetime clients, {} bytes",p.phase,p.clients.len(),frame.len());
 }
 Ok(())
}