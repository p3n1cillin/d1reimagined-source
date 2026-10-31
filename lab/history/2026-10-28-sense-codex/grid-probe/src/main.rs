pub use d1_oracle::error;
#[path="F:/d1/public-work/d1reimagined-stats/ps4/d1-server/crates/d1-oracle/src/scalar_float.rs"] mod scalar_float;
use d1_oracle::harness::{Harness,Stub};
fn main()->anyhow::Result<()>{
 let root=std::path::Path::new(r"lab/history\2026-10-28-character-stats");
 let (image,mut h)=Harness::open_default()?;anyhow::ensure!(image.sha256()==d1_oracle::weapon_stats::MEASURED_EBOOT_SHA256,"wrong image");
 let scalar=scalar_float::ScalarFloat::install(h.uc(),&[(0x286ce0,0x286dff),(0x281a20,0x281b99),(0x1017530,0x101774d),(0x915e0,0x915ed)])?;
 let stat=std::fs::read(root.join("owned-stat-definitions.bin"))?;let table=h.alloc(stat.len()as u64)?;h.write(table,&stat)?;
 let def=std::fs::read(root.join("owned-definition-ci710.bin"))?;let d=h.alloc(def.len()as u64)?;h.write(d,&def)?;
 let grid=std::fs::read(root.join("owned-grid-132.bin"))?;let g=h.alloc(grid.len()as u64)?;h.write(g,&grid)?;
 let interface=h.alloc(8)?;let vt=h.alloc(0x600)?;h.write_u64(interface,vt)?;h.write_u64(vt+0x4c8,0x25b6e0)?;h.write_u64(vt+0x4e0,0x25b960)?;
 h.stub(0x25a550,Stub::Return(interface));h.stub(0x25b960,Stub::Return(g));
 let original=std::fs::read(root.join("instance-ci710.decoded.bin"))?;
 for pattern in ["all-ff","all-zero","node24-zero"]{
  let mut item=original.clone();item[0x58..0x5a].copy_from_slice(&132u16.to_le_bytes());item[0x5a..0x7e].fill(if pattern=="all-zero"{0}else{255});if pattern=="node24-zero"{item[0x5a+24]=0;}
  let i=h.alloc(item.len()as u64)?;h.write(i,&item)?;
  for key in [2,3,4]{h.stub(0x25b6e0,Stub::Return(table+0x30+16*key));let result=h.call(0x286ce0,&[g,d,i,0,key]);scalar.check()?;println!("{}",serde_json::json!({"synthetic_selection":pattern,"stat":key,"result":format!("{result:?}")}));}
 }Ok(())
}
