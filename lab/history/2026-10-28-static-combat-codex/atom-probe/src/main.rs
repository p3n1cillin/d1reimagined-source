use std::{cell::RefCell,collections::VecDeque,rc::Rc};
use d1_oracle::object::ObjectOracle;
use unicorn_engine::RegisterX86;
fn main()->Result<(),Box<dyn std::error::Error>> {
 let image=std::fs::read(std::env::args().nth(1).expect("image path"))?;
 let mut o=ObjectOracle::new(0x80800868)?;
 let ring=Rc::new(RefCell::new(VecDeque::new()));let r=ring.clone();
 o.harness().uc().add_code_hook(1,0,move|u,va,size|{
  let mut r=r.borrow_mut();if r.len()==24{r.pop_front();}
  r.push_back((va,size,u.reg_read(RegisterX86::RSP).unwrap_or(0),u.reg_read(RegisterX86::RAX).unwrap_or(0),u.reg_read(RegisterX86::RDI).unwrap_or(0),u.reg_read(RegisterX86::RSI).unwrap_or(0)));
 })?;
 let result=o.round_trip(&image,0x10000);
 for row in ring.borrow().iter(){println!("{:x?}",row);}
 match result {Ok((e,d))=>{println!("PASS bits={} decoded={} identical={}",e.bits,d.bits,d.object==image);Ok(())},Err(e)=>{eprintln!("FAIL {e}");Err(e.into())}}
}
