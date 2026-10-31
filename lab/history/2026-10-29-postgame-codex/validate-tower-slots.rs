#[path="F:/d1/public-work/d1rs-claude/ps4/d1-server/crates/d1-activity-host/src/sensor_list.rs"] mod sensor_list;
use d1_oracle::sensor::SensorPhaseUpdate;
fn main()->anyhow::Result<()> {
 let text=std::fs::read_to_string("F:/d1/d1preservation-main/cfw-staging/tower-north-authored-slots.diagnostic.draft.txt")?;
 let base=SensorPhaseUpdate{gen:0,name_hash:0x40aebf90,name_i8:16,name_sub:3,phase:3,tail:None,clients:vec![],authority:None};
 for team in [0,-1] {
  let p=sensor_list::params_for_list(&base,0x40aebf90,team,&text)?;
  assert_eq!(p.name_sub,2);assert_eq!(p.clients.len(),99);
  let squads:Vec<_>=p.clients.iter().filter(|r|r.name_hash==0x8d3f1166 && r.name_i8==1).collect();assert_eq!(squads.len(),15);
  let frame=d1_activity_host::codec::SchemaCodec::build_sensor(&p)?;
  println!("PASS: current list parser lifetime sub{}, {} clients, 15 typed squads; schema codec team{team}, {} bytes",p.name_sub,p.clients.len(),frame.len());
 }
 Ok(())
}
