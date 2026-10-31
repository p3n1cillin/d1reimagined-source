use std::fs;
fn main()->Result<(),Box<dyn std::error::Error>> {
 let root="lab/history/2026-10-29-postgame-codex";
 let estate=d1_pkg::Estate::load("F:/d1/d1preservation-main/dumps/later-56578-CUSA00219/CUSA00219/packages")?;
 let mut content=d1_oracle::oodle::Content::open_default()?;
 let mut text=String::new();
 for id in [0x25cu16] {
  let p=estate.newest(id,0).unwrap();
  for y in p.symbols(){ text+=&format!("SYMBOL {:08x} {} {:08x}\n",y.tag,y.name,y.type_tag); }
  let out=format!("{root}/hangar-owned");fs::create_dir_all(&out)?;
  for e in p.entries().filter(|e| e.class_tag==0x8080076e && e.size()>=8 && e.size()<=4000000) {
   let tag=d1_pkg::make_tag(id,e.index);let Ok((_,_,_,raw))=content.entry_bytes_for_tag(tag) else {continue;};
   fs::write(format!("{out}/{tag:08x}.bin"),&raw)?;
   text+=&format!("ENTRY {tag:08x} {:08x} {}\n",e.class_tag,raw.len());
   for chunk in raw.split(|b| *b<32 || *b>=127).filter(|s|s.len()>=7) {text+=&format!("  {}\n",String::from_utf8_lossy(chunk));}
  }
 }
 fs::write(format!("{root}/hangar-owned-catalog.txt"),&text)?;
 println!("catalog written, {} bytes",text.len());Ok(())
}


