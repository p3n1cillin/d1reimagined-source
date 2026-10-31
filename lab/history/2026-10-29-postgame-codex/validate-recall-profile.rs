use std::{fs,path::Path};
use d1_oracle::object::ObjectOracle;
const P:&str="lab/history/2026-10-29-postgame-codex";
const S:&str="F:/d1/d1preservation-main/cfw-staging";
fn main()->Result<(),Box<dyn std::error::Error>> {
 let raw=fs::read(format!("{P}/sense54-before-recall-gate-profile.obj"))?;
 let template=fs::read("lab/history/2026-10-28-evening-claude/profile-complete164-xp900.obj")?;
 let mut oracle=ObjectOracle::new(0x80801743)?;
 let first=oracle.encode(&raw,0x10000)?; let decoded=oracle.decode(&first.payload)?;
 let canonical=decoded.object;
 fs::write(format!("{P}/sense54-canonical-profile.obj"),&canonical)?;
 let changed:Vec<_>=raw.iter().zip(canonical.iter()).enumerate().filter(|(_, (a,b))|a!=b).map(|(i,_)|i).collect();
 println!("Raw live normalization: {} bytes; canonical/template differences: {}",changed.len(),canonical.iter().zip(template.iter()).filter(|(a,b)|a!=b).count());
 // Every wire field in this captured profile must match the authoritative fixture.
 assert_eq!(canonical,template,"live encoded profile differs from template; review before making drafts");
 for value in [1u8,0] {
  let mut image=canonical.clone();assert_eq!(image[0x5661],2);image[0x5661]=value;
  assert_eq!(image.iter().zip(canonical.iter()).filter(|(a,b)|a!=b).count(),1);
  let(enc,dec)=oracle.round_trip(&image,0x10000)?;assert_eq!(image,dec.object);
  let stem=format!("sense54-completion673-{value}-recall");
  fs::write(format!("{P}/{stem}.obj"),&image)?;
  fs::write(Path::new(S).join(format!("{stem}.draft.bin")),&enc.payload)?;
  println!("PASS {stem}: only canonical byte+5661 changes2->{value}; {} bits, {} bytes; object IDENTICAL",enc.bits,enc.payload.len());
 }
 Ok(())
}