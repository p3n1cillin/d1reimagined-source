use d1_oracle::ObjectOracle;
use std::{collections::HashMap,io::Write};
fn part(b:&[u8],s:usize)->Vec<u8>{let mut o=vec![0;(b.len()*8-s+7)/8];for i in s..b.len()*8{o[(i-s)/8]|=((b[i/8]>>(7-i%8))&1)<<(7-(i-s)%8);}o}
fn bits(b:&[u8],s:usize,n:usize)->anyhow::Result<u64>{anyhow::ensure!(s+n<=b.len()*8,"short field {s}+{n}");Ok((s..s+n).fold(0,|a,i|(a<<1)|((b[i/8]>>(7-i%8))&1)as u64))}
fn main()->anyhow::Result<()>{
 let args=std::env::args().collect::<Vec<_>>();let text=std::fs::read_to_string(&args[1])?;
 let mut out=std::io::BufWriter::new(std::fs::File::create(&args[2])?);
 let mut names=ObjectOracle::new(0x808004b1)?;let mut oracles=HashMap::new();let mut states:HashMap<(u8,u64),Vec<u8>>=HashMap::new();
 let tags:HashMap<u8,u32>=[(1,0x80800357),(2,0x808005d9),(3,0x80800830),(4,0x80800387),(12,0x80802c30),(24,0x8080048a),(28,0x808005c3),(39,0x80802b18)].into();
 for(line_no,line)in text.lines().enumerate(){let Some(h)=line.split("\tudt6\t").nth(1)else{continue};
  let p=h.trim().as_bytes().chunks_exact(2).map(|c|u8::from_str_radix(std::str::from_utf8(c).unwrap(),16).unwrap()).collect::<Vec<_>>();
  let result=(||->anyhow::Result<serde_json::Value>{let gen=bits(&p,1,8)? as u8;anyhow::ensure!(bits(&p,9,1)?==0,"reserved bit");let mut pos=10;let mut entries=Vec::new();let mut pending=states.clone();
   while bits(&p,pos,1)?!=0{pos+=1;let n=names.decode(&part(&p,pos))?;anyhow::ensure!(n.bits==55,"name bits");pos+=55;let key=u64::from_le_bytes(n.object[..8].try_into()?);let kind=n.object[4];let tag=*tags.get(&kind).ok_or_else(||anyhow::anyhow!("unknown kind {kind}"))?;
    if !oracles.contains_key(&tag){oracles.insert(tag,ObjectOracle::with_types(tag,d1_oracle::sensor::TYPED_VALUE_TYPES)?);}let oracle=oracles.get_mut(&tag).unwrap();let seed=pending.get(&(gen,key)).cloned().unwrap_or_else(||vec![0;oracle.struct_size()]);let body=oracle.decode_replicated(&part(&p,pos),&seed)?;let start=pos;pos+=body.bits as usize;let rev=bits(&p,pos,32)?;pos+=32;
    entries.push(serde_json::json!({"group":format!("{:08x}",key as u32),"kind":kind,"sub":(key>>48)as u16,"tag":format!("{tag:08x}"),"bits":body.bits,"start":start,"revision":rev,"nonzero":body.nonzero_qwords(),"changed":seed.iter().zip(&body.object).enumerate().filter_map(|(i,(a,b))|if a!=b{Some((i,*a,*b))}else{None}).collect::<Vec<_>>() }));pending.insert((gen,key),body.object);
   }pos+=1;anyhow::ensure!(p.len()*8-pos<8,"extra trailing bits {}",p.len()*8-pos);anyhow::ensure!(bits(&p,pos,p.len()*8-pos)?==0,"nonzero padding");states=pending;
   Ok(serde_json::json!({"line":line_no+1,"time":line.split('\t').next(),"gen":gen,"first":bits(&p,0,1)?,"bytes":p.len(),"entries":entries}))})();
  match result{Ok(v)=>writeln!(out,"{v}")?,Err(e)=>writeln!(out,"{}",serde_json::json!({"line":line_no+1,"error":e.to_string(),"hex":h}))?};out.flush()?;
 }Ok(())
}
