"""One bounded read-only snapshot; no pad, debugger, memory writes, or process stops."""
from pathlib import Path
import importlib.util,struct,json,datetime,math
import static56578 as st
SRC=Path(r'F:\d1\public-work\d1reimagined-sense\tools\ps4_setup\d1-winguest.py')
spec=importlib.util.spec_from_file_location('g',SRC);g=importlib.util.module_from_spec(spec);spec.loader.exec_module(g)
p,base=g.open_client()
controls=[]
for va in (0x912791,0x546fd0,0xa5b12e,0xa4ffa3,0x391330,0x38b5e0):
    assert p.read(base+va,24)==st.read(va,24),hex(va)
    controls.append({'va':hex(va),'bytes':24,'matches_owned':True})
players=p.u(p.u(base+g.PLAYER_POOL,'Q')+8,'Q');player=players
rows={}
for label,off,n in [('source_power',0xc58,0x30),('config',0x1660,0x28),('general',0x1460,0x80),('level_source_tail',0x19bc,0x30),('player_id',0x600,0x80)]:
    raw=p.read(player+off,n);rows[label]={'address':hex(player+off),'hex':raw.hex(),'u32':list(struct.unpack('<'+'I'*(n//4),raw)),'f32':[x if math.isfinite(x) else str(x) for x in struct.unpack('<'+'f'*(n//4),raw)]}
cache=p.read(base+0x277af10+0x288,0x30)
out={'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'base':hex(base),'player':hex(player),'controls':controls,'rows':rows,'death_cache':{'record':hex(base+0x277af10),'tail':cache.hex(),'source_raw_power':struct.unpack_from('<f',cache,0x24)[0],'category':struct.unpack_from('<I',cache,0x18)[0]}}
dest=st.ROOT/'native-combat-snapshot-v2.json'
with dest.open('x',encoding='utf8') as f:json.dump(out,f,indent=2,allow_nan=False)
print(json.dumps({'utc':out['utc'],'player':out['player'],'source_power':rows['source_power'],'config':rows['config'],'death_cache':out['death_cache']}))
