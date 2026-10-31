use d1_oracle::harness::{Harness, Stub};
use std::path::Path;
fn main() -> anyhow::Result<()> {
    let root = std::env::args().nth(1).expect("private input directory");
    let root = Path::new(&root);
    let (image, mut h) = Harness::open_default()?;
    anyhow::ensure!(image.sha256() == d1_oracle::weapon_stats::MEASURED_EBOOT_SHA256, "wrong image");
    if std::env::var_os("FIX_SCALAR").is_some() {
        use unicorn_engine::RegisterX86 as R;
        h.uc().add_code_hook(1,0,|uc,a,_| {
            let Ok(b)=uc.mem_read_as_vec(a,4) else {return};
            if b[0]!=0xc5 || b[1]&7!=2 || ![0x58,0x59,0x5c,0x5e].contains(&b[2]) || b[3]&0xc0!=0xc0 {return}
            let dest=((b[3]>>3)&7) | (((!b[1]>>7)&1)<<3);
            let src1=(!b[1]>>3)&15;let src2=b[3]&7;
            let rb=R::XMM0 as i32;
            let x=uc.reg_read_long(rb+src1 as i32).unwrap();let y=uc.reg_read_long(rb+src2 as i32).unwrap();
            let xf=f32::from_le_bytes(x[..4].try_into().unwrap());let yf=f32::from_le_bytes(y[..4].try_into().unwrap());
            let value=match b[2] {0x58=>xf+yf,0x59=>xf*yf,0x5c=>xf-yf,0x5e=>xf/yf,_=>unreachable!()};
            let mut out=vec![0u8;32];out[..16].copy_from_slice(&x);out[..4].copy_from_slice(&value.to_le_bytes());
            uc.reg_write_long(R::YMM0 as i32+dest as i32,&out).unwrap();
            uc.reg_write(R::RIP,a+4).unwrap();
        })?;
    }
    if std::env::var_os("TRACE_ARITH").is_some() {
        use unicorn_engine::RegisterX86 as R;
        for (name,code) in [
            ("vaddss alias",vec![0xc5,0xf2,0x58,0xc0]),
            ("vaddss separate",vec![0xc5,0xf2,0x58,0xd8]),
            ("vaddss same source1",vec![0xc5,0xfa,0x58,0xc1]),
            ("addss legacy",vec![0xf3,0x0f,0x58,0xc1]),
        ] {
            let p=h.alloc(32)?;let mut b=code;b.extend([0x66,0x0f,0x7e,0xc0,0xc3]);h.write(p,&b)?;
            for (r,f) in [(R::XMM0,2f32),(R::XMM1,10f32),(R::XMM2,0.25f32),(R::XMM3,0f32)] {
                let mut v=vec![0u8;16];v[..4].copy_from_slice(&f.to_le_bytes());h.uc().reg_write_long(r,&v)?;
            }
            let result=h.call(p,&[])?;
            let v=h.uc().reg_read_long(R::XMM3)?;
            println!("ARITH {name}: xmm0={} xmm3={}",f32::from_bits(result as u32),f32::from_le_bytes(v[..4].try_into()?));
        }
        return Ok(());
    }
    let stat = std::fs::read(root.join("owned-stat-definitions.bin"))?;
    anyhow::ensure!(stat.len() == 2624 && u64::from_le_bytes(stat[8..16].try_into()?) == 36);
    let table = h.alloc(stat.len() as u64)?; h.write(table, &stat)?;
    let interface = h.alloc(8)?; let vtable = h.alloc(0x600)?;
    h.write_u64(interface, vtable)?; h.write_u64(vtable + 0x4c8, 0x25b6e0)?;
    h.stub(0x25a550, Stub::Return(interface));
    if std::env::var_os("TRACE_CURVE").is_some() {
        use unicorn_engine::RegisterX86 as R;
        h.uc().add_code_hook(1,0,|uc,a,_| {
            if [0x281ab5,0x281ac1,0x281ac9,0x281ad4,0x1017661,0x10176b3,0x10176ed,0x1017705,0x1017730,0x1017742,0x915e0,0x915e4,0x915e8,0x915ec].contains(&a) {
                let mut regs=Vec::new();
                for r in [R::XMM0,R::XMM1,R::XMM2,R::XMM4] {
                    let b=uc.reg_read_long(r).unwrap();regs.push(f32::from_le_bytes(b[..4].try_into().unwrap()));
                }
                println!("TRACE {a:x} code={:x?} xmm={regs:?} rax={:x} rdx={:x} flags={:x}",uc.mem_read_as_vec(a,4).unwrap(),uc.reg_read(R::RAX).unwrap(),uc.reg_read(R::RDX).unwrap(),uc.reg_read(R::EFLAGS).unwrap());
            }
        })?;
        let def=std::fs::read(root.join("owned-definition-ci3598.bin"))?;
        let mut item=std::fs::read(root.join("instance-ci3598.decoded.bin"))?;
        item[0x18..0x1c].copy_from_slice(&2i32.to_le_bytes());
        let d=h.alloc(def.len() as u64)?;h.write(d,&def)?;
        let i=h.alloc(item.len() as u64)?;h.write(i,&item)?;
        h.stub(0x25b6e0,Stub::Return(table+0x30+16*11));
        println!("TRACE_RESULT {}",h.call(0x26f7e0,&[d,i,0,0,11])?);
        return Ok(());
    }
    for ci in [6514,5942,3886,5946,5879,710,4531,4286,3598] {
        let def = std::fs::read(root.join(format!("owned-definition-ci{ci}.bin")))?;
        let item = std::fs::read(root.join(format!("instance-ci{ci}.decoded.bin")))?;
        let d = h.alloc(def.len() as u64)?; h.write(d, &def)?;
        let i = h.alloc(item.len() as u64)?; h.write(i, &item)?;
        for key in [0,1,2,3,4,5,6,7,11] {
            h.stub(0x25b6e0, Stub::Return(table + 0x30 + 16*key));
            let value = h.call(0x26f7e0, &[d,i,0,0,key])? as u32;
            println!("ci={ci} key={key} value={value}");
        }
    }
    let mut content = d1_oracle::oodle::Content::open_default()?;
    let mut native = d1_oracle::native_item_stat::NativeItemStat::open(&mut content)?;
    for (ci,key) in [(3598,11),(5942,1)] {
        let def = std::fs::read(root.join(format!("owned-definition-ci{ci}.bin")))?;
        let mut item = std::fs::read(root.join(format!("instance-ci{ci}.decoded.bin")))?;
        for (level,expected) in [(0,3),(1,5),(2,10),(20,100),(40,200)] {
            item[0x18..0x1c].copy_from_slice(&i32::to_le_bytes(level));
            let result=native.base_value(&def,&item,key)?;
            anyhow::ensure!(result==Some(expected), "curve control {ci}/{level}: {result:?}");
            println!("CONTROL curve ci={ci} key={key} level={level} expected_anchor={expected} returned={result:?}");
        }
        anyhow::ensure!(native.base_value(&def,&item,63).is_err());
        anyhow::ensure!(native.base_value(&def,&item,2)?.is_none());
        item[0x58..0x5a].copy_from_slice(&0u16.to_le_bytes());
        anyhow::ensure!(native.base_value(&def,&item,key).is_err());
        println!("PASS absent-key, out-of-range and active-talent refusals ci={ci}");
    }
    Ok(())
}
