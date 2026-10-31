from pathlib import Path
import ctypes,importlib.util,struct,json,datetime,sys
P=Path(r'lab/history\2026-10-28-static-combat-codex')
spec=importlib.util.spec_from_file_location('wg',r'F:\d1\public-work\d1reimagined-sense\tools\ps4_setup\d1-winguest.py');g=importlib.util.module_from_spec(spec);spec.loader.exec_module(g)
p,base=g.open_client();reg=p.u(p.u(base+g.REGISTRY,'Q'),'Q')
def resolve(tag):
 v=ctypes.c_int32(tag).value>>13;sl=(((v&0xffffffff)>>18)|0x3ff)&v&0xffffffff;r=reg+sl*64
 d,es,m=p.u(r+16,'Q'),p.u(r+48,'i'),p.u(r+52,'i');assert d and 0<es<0x100000
 e=d+(tag&0x1fff)*es;return e-ctypes.c_int64(m&p.u(e+8,'Q')).value
out={'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'base':hex(base),'controls':[]}
for tag,fn,off,n in [(0x80cb9120,'owned-dockroute-80cb9120.bin',0x1360,0x40),(0x80cb9120,'owned-dockroute-80cb9120.bin',0x2110,0x60),(0x80cb86aa,'owned-definition-80cb86aa.bin',0x4b0,0x580),(0x80cb86ba,'owned-definition-80cb86ba.bin',0x4f8,0x28),(0x80cb86bd,'owned-definition-80cb86bd.bin',0x4f8,0x28),(0x80cb86a5,'owned-dockroute-80cb86a5.bin',0x2c0,8)]:
 try:
  a=resolve(tag);expected=(P/fn).read_bytes()[off:off+n];actual=p.read(a+off,n);r={'tag':hex(tag),'off':hex(off),'bytes':n,'native_match':actual==expected,'address':hex(a+off)}
 except (OSError,AssertionError) as e:r={'tag':hex(tag),'error':str(e)}
 out['controls'].append(r)
us=importlib.util.spec_from_file_location('unit',r'F:\d1\public-work\d1reimagined-sense\tools\ps4_setup\d1_unit_tracking.py');u=importlib.util.module_from_spec(us);sys.modules['unit']=u;us.loader.exec_module(u)
sample=u.read_unit_position(p,base);out['player_position']=sample.position if sample else None
(P/'dockroute-native-checks.json').write_text(json.dumps(out,indent=2));print(json.dumps(out,indent=2))
