use std::fs;
fn u32at(b:&[u8],o:usize)->u32{u32::from_le_bytes(b[o..o+4].try_into().unwrap())}
fn i64at(b:&[u8],o:usize)->i64{i64::from_le_bytes(b[o..o+8].try_into().unwrap())}
fn main()->Result<(),Box<dyn std::error::Error>> {
 let root="lab/history/2026-10-29-postgame-codex";
 let dir="F:/d1/d1preservation-main/dumps/later-56578-CUSA00219/CUSA00219/packages";
 let estate=d1_pkg::Estate::load(dir)?;
 let mut content=d1_oracle::oodle::Content::open_default()?;
 let mut checked=0;
 for id in [0x1a9,0x23d,0x244,0x24c,0x25c,0x265,0x268,0x26d] {
  let p=estate.newest(id,0).unwrap();
  for e in p.entries().filter(|e|e.class_tag&0xffff0000==0x81060000 && e.size()>=0x60 && e.size()<=0x20000) {
   checked+=1;
   let tag=d1_pkg::make_tag(id,e.index);
   let (_,_,_,raw)=content.entry_bytes_for_tag(tag)?;
   if raw.len()<0x60 {continue;}
   let a=0x30i64+i64at(&raw,0x30);
   if a<0 || a as usize>=raw.len() {continue;}
   let name=&raw[a as usize..];let n=name.iter().position(|&b|b==0).unwrap_or(name.len());
   if n==0 || n>100 || !name[..n].iter().all(|&b|b>=32&&b<127) {continue;}
   let mut hash=0x811c9dc5u32;for &b in &name[..n]{hash=hash.wrapping_mul(0x1000193)^b as u32;}
   if i64at(&raw,0) as usize != raw.len(){continue;}
   let bubbles=i64at(&raw,0x48);if !(0..=64).contains(&bubbles){continue;}
   let name=String::from_utf8_lossy(&name[..n]);
   println!("{tag:08x} class{:08x} {name} worldhash{:08x} private{:08x} public{:08x} bubbles{bubbles} {} bytes",e.class_tag,u32at(&raw,8),u32at(&raw,0xc),u32at(&raw,0x10),raw.len());
   fs::write(format!("{root}/owned-{tag:08x}.bin"),&raw)?;
  }
 }
 println!("checked {checked} generated-class candidates in seven owned packages");
 Ok(())
}
