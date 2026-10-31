use std::{fs,collections::BTreeSet};
fn main()->Result<(),Box<dyn std::error::Error>> {
 let dir="F:/d1/d1preservation-main/dumps/later-56578-CUSA00219/CUSA00219/packages";
 let estate=d1_pkg::Estate::load(dir)?;
 let mut seen=BTreeSet::new();
 let mut rows=Vec::new();
 for p in &estate.packages {
  if p.locale()!=0 || !seen.insert(p.id()) {continue;}
  let p=estate.newest(p.id(),0).unwrap();
  for y in p.symbols().filter(|y|y.name.contains("tower") || y.name.contains("cosmo_m0") || y.name.contains("orbit")) {
   rows.push((y.tag,format!("{} {} type{:08x}",p.name,y.name,y.type_tag),0));
  }
 }
 let root="lab/history/2026-10-29-postgame-codex";
 let mut text=String::new();
 for (t,p,n) in rows {text+=&format!("{t:08x}\t{n}\t{p}\n");}
 fs::write(format!("{root}/world-symbol-catalog.tsv"),&text)?;
 print!("{text}");
 Ok(())
}
