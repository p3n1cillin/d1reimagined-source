from pathlib import Path
import ctypes,importlib.util,sys,struct,json,datetime
P=Path(r'lab/history\2026-10-28-static-combat-codex');sys.path.insert(0,str(P));import static56578 as st
sp=importlib.util.spec_from_file_location('wg',r'F:\d1\public-work\d1reimagined-sense\tools\ps4_setup\d1-winguest.py');g=importlib.util.module_from_spec(sp);sp.loader.exec_module(g);p,base=g.open_client();reg=p.u(p.u(base+g.REGISTRY,'Q'),'Q')
for va in (0x30dfa0,0x30e030,0x324870,0x324a90,0x5142c0,0x546fd0):assert p.read(base+va,24)==st.read(va,24)
def resolve(tag):
 v=ctypes.c_int32(tag).value>>13;sl=(((v&0xffffffff)>>18)|0x3ff)&v&0xffffffff;r=reg+sl*64;d,es,m=p.u(r+16,'Q'),p.u(r+48,'i'),p.u(r+52,'i');assert d and 0<es<0x100000;e=d+(tag&0x1fff)*es;return e-ctypes.c_int64(m&p.u(e+8,'Q')).value
player=p.u(p.u(base+g.PLAYER_POOL,'Q')+8,'Q');uh=p.u(player+0x644,'I');tab=base+0x26897f0;data=p.u(tab+8,'Q');stride=p.u(tab+16,'I');mask=p.u(tab+40,'i');assert stride==32;e=data+(uh&0x1fff)*stride;unit=e-ctypes.c_int64(mask&p.u(e+8,'Q')).value;assert p.u(unit+0x24,'I')==uh
component_unit=unit
entity_handle=p.u(unit+0x2c,'I');entity_table=base+0x2692220;entity_stride=p.u(entity_table+0x10,'I');assert 0<entity_stride<0x1000
entity=p.u(entity_table+8,'Q')+(entity_handle&0x1fff)*entity_stride
parent_handle=p.u(entity+0x54,'I');e=data+(parent_handle&0x1fff)*stride;unit=e-ctypes.c_int64(mask&p.u(e+8,'Q')).value
print('entity',hex(entity_handle),hex(entity),'parent',hex(parent_handle),hex(unit),'entity_stride',entity_stride,'pool_count',p.u(entity_table+0x14,'I'))

h=p.u(unit+8,'I');off=p.u(unit+0x10,'Q');definition=resolve(h)+off;container=definition+0x38;size=p.u(container+8,'I');assert 0<size<=4096 and size&(size-1)==0

slots=container+0x20+p.u(container+0x10,'q');records=container+0x30+p.u(container+0x20,'q')
typecell=p.u(base+0x19b3058,'Q');th=p.u(typecell,'I');typ=resolve(th);key=p.u(typ+8,'I');seed=p.u(container,'I')
a=((key<<4)+(seed^key))&0xffffffff;a^=a>>10;a=(a*0x81)&0xffffffff;a^=a>>13;slot=a&(size-1);found=None
for i in range(size):
 q=slots+slot*8;k,start,flag=struct.unpack('<Ihh',p.read(q,8))
 if k==key:found=(start,flag&0x7fff);break
 if flag>=0:break
 slot=(slot+1)&(size-1)
print('unit',hex(unit),'container',hex(container),'size',size,'typehandle',hex(th),'typekey',hex(key),'found',found)
rows=[]
if found:
 for i in range(found[1]):
  rec=records+(found[0]+i)*0x28;tag=p.u(rec+0x10,'I');off=p.u(rec+0x18,'Q')
  runtime_off=p.u(rec+4,'i');component=unit+runtime_off;state_handle=p.u(component+0x24,'I');state=resolve(state_handle);raw=p.read(state,0xc8)
  rows.append({'component':hex(component),'runtime_offset':hex(runtime_off),'definition_tag':hex(tag),'definition_offset':hex(off),'state_handle':hex(state_handle),'state':hex(state),'header':raw.hex(),'identity_84_hex':raw[0x84:0x94].hex(),'power_94':list(struct.unpack_from('<4f',raw,0x94)),'mode_b4':raw[0xb4]})

source=p.read(player+0xc58,0x10);config=p.read(player+0x1660,0x28)
assert p.u(player+0x644,'I')==uh and p.u(entity+0x54,'I')==parent_handle
out={'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'unit':hex(unit),'unit_component_handle':hex(uh),'entity_handle':hex(entity_handle),'parent_handle':hex(parent_handle),'source':source.hex(),'config':config.hex(),'rows':rows};(P/'sense40-power-unit-native.json').write_text(json.dumps(out,indent=2));print(json.dumps(out,indent=2))
