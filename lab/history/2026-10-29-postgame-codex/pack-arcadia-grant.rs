use std::{env,fs,path::Path};
fn main()->Result<(),Box<dyn std::error::Error>> {
 let args:Vec<String>=env::args().collect();
 if args.len()!=4 {return Err("usage: pack-arcadia-grant CURRENT_FAMILY4_VERSION PROFILE_DRAFT_BIN OUTPUT.draft.bin; OUTPUT is an UNSERVED QueueZ body".into());}
 let current:u32=args[1].parse()?;
 let next=current.checked_add(1).ok_or("family version overflow")?;
 let profile_path=Path::new(&args[2]);let output=Path::new(&args[3]);
 if !profile_path.to_string_lossy().ends_with(".draft.bin") || !output.to_string_lossy().ends_with(".draft.bin") {return Err("require explicitly named draft input/output files".into());}
 if output.exists(){return Err("output exists; refusing overwrite".into());}
 let item=fs::read("F:/d1/d1preservation-main/cfw-staging/arcadia-3517-inventory.draft.bin")?;
 let profile=fs::read(profile_path)?;
 let body=d1_bap::loot::item_grant_delta(0xd100000000000001,next,0xd1000000000003f1,&item,Some((0xd100000000000201,&profile)));
 fs::write(output,&body)?;
 println!("UNSERVED family4 DELTA body: accepted version {current} -> {next}, 2 objects, {} bytes. Not a BAP/encrypted frame. No network or control writes.",body.len());
 Ok(())
}
