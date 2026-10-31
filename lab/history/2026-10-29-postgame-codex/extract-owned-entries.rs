use std::fs;
fn main()->Result<(),Box<dyn std::error::Error>> {
 let mut content=d1_oracle::oodle::Content::open_default()?;
 let root="lab/history/2026-10-29-postgame-codex";
 for arg in std::env::args().skip(1) {
  let tag=u32::from_str_radix(arg.trim_start_matches("0x"),16)?;
  match content.entry_bytes_for_tag(tag) {
   Ok((pkg,index,class,raw))=> {
    fs::write(format!("{root}/owned-{tag:08x}.bin"),&raw)?;
    println!("{tag:08x}: {pkg}/{index} class{class:08x}, {} bytes",raw.len());
   }
   Err(e)=>println!("{tag:08x}: ERROR {e}"),
  }
 }
 Ok(())
}
