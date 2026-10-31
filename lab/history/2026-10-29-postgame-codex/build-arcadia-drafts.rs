use std::{fs, path::Path};
use d1_oracle::{inventory::{ItemSpec,ItemSpec as Spec},item_index::IndexTable,object::ObjectOracle};
const P: &str="lab/history/2026-10-29-postgame-codex";
const S: &str="F:/d1/d1preservation-main/cfw-staging";
const I: u64=0xD100_0000_0000_03F1;
fn main()->Result<(),Box<dyn std::error::Error>> {
 let table=IndexTable::load(Path::new("F:/d1/d1preservation-main/capture-rig/builds/later-56578/item-content-index.tsv"))?;
 let mut spec:Spec=ItemSpec::resolved(I,0x8e7bf3ba,"Ships",&table)?;
 assert_eq!(spec.content_index,3517); spec.level=1;
 let obj=spec.to_object()?;
 fs::write(format!("{P}/arcadia-inventory.obj"),&obj)?;
 let mut codec=ObjectOracle::new(0x80801732)?;
 let item=d1_oracle::inventory::build(&mut codec,&spec)?;
 fs::write(format!("{S}/arcadia-3517-inventory.draft.bin"),&item.payload)?;
 println!("INVENTORY: {} bits, {} bytes, verified by inventory::build; instance {I:#018x}",item.bits,item.payload.len());
 for (suffix,base) in [("xp0","profile-complete164-prog.obj"),("xp900","profile-complete164-xp900.obj")] {
  let original=fs::read(format!("lab/history/2026-10-28-evening-claude/{base}"))?;
  assert_eq!(original.len(),29560);
  let slot=0x38+123*32;
  assert_eq!(&original[slot..slot+2],&[255,255]);
  assert!(!original.windows(8).any(|b|b==I.to_le_bytes()));
  let mut profile=original.clone();
  profile[slot..slot+2].copy_from_slice(&3517u16.to_le_bytes());
  profile[slot+8..slot+16].copy_from_slice(&I.to_le_bytes());
  for equipped in [false,true] {
   let mut image=profile.clone();
   if equipped {
    // Owned Arcadia client definition: equipped-array ordinal9, NOT backpack bucket123.
    let at=0x2b80+9*8;
    assert_eq!(&original[at..at+8],&[0;8]);
    image[at..at+8].copy_from_slice(&I.to_le_bytes());
   }
   let name=format!("arcadia-profile-{suffix}-{}",if equipped{"equipped"}else{"owned"});
   let mut oracle=ObjectOracle::new(0x80801743)?;
   let (enc,dec)=oracle.round_trip(&image,0x10000)?;
   assert_eq!(image,dec.object);
   fs::write(format!("{P}/{name}.obj"),&image)?;
   fs::write(format!("{S}/{name}.draft.bin"),&enc.payload)?;
   let changed=image.iter().zip(original.iter()).filter(|(a,b)|a!=b).count();
   println!("PROFILE {name}: {} bits, {} bytes, round trip identical, {changed} changed bytes",enc.bits,enc.payload.len());
  }
 }
 let mut scoreboard=fs::read(format!("{P}/scoreboard-one-player-A.bin"))?;
 // Player row0 starts+10. Its rewards count+110 and four 0xe0 typed-value slots+120.
 scoreboard[0x120..0x124].copy_from_slice(&1u32.to_le_bytes());
 scoreboard[0x130..0x134].copy_from_slice(&0x80801732u32.to_le_bytes());
 scoreboard[0x140..0x140+184].copy_from_slice(&obj);
 fs::write(format!("{P}/scoreboard-one-player-arcadia-A.obj"),&scoreboard)?;
 let mut oracle=ObjectOracle::with_types(0x808008d0,&[0x80801732])?;
 let (enc,dec)=oracle.round_trip(&scoreboard,0x10000)?;
 assert_eq!(scoreboard,dec.object);
 fs::write(format!("{P}/scoreboard-one-player-arcadia-A.wire.bin"),&enc.payload)?;
 println!("SCOREBOARD REWARD: {} bits, {} bytes, typed inventory item round trip identical",enc.bits,enc.payload.len());
 Ok(())
}
