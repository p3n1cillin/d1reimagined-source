"""One bounded read-only clock/trigger snapshot; no input or memory writes."""
import ctypes,importlib.util,struct,json,datetime
import static56578 as st
spec=importlib.util.spec_from_file_location('g',r'F:\d1\public-work\d1reimagined-sense\tools\ps4_setup\d1-winguest.py');g=importlib.util.module_from_spec(spec);spec.loader.exec_module(g)
p,base=g.open_client()
for va in (0x12a800,0x12adf9,0x108730,0x2a2ec0,0x2a2910):assert p.read(base+va,24)==st.read(va,24)
reg=p.u(p.u(base+g.REGISTRY,'Q'),'Q')
def resolve(tag):
 v=ctypes.c_int32(tag).value>>13;sl=(((v&0xffffffff)>>18)|0x3ff)&v&0xffffffff;r=reg+sl*64
 d,es,m=p.u(r+16,'Q'),p.u(r+48,'i'),p.u(r+52,'i');assert d and 0<es<0x100000
 e=d+(tag&0x1fff)*es;return e-ctypes.c_int64(m&p.u(e+8,'Q')).value
T=base+g.LT_TABLE;idx=p.u(base+g.LT_COUNT,'H');content=p.u(T+p.u(T,'q')+idx*p.u(T+16,'Q')+0x30,'Q')
world=p.u(base+g.WORLD_POOL,'Q');clock=p.u(world+0x10,'Q')
out={'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'world':hex(world),'clock':hex(clock),'tick_ms':p.u(clock+0x18,'f'),'received_stamp':p.u(clock+0x3c,'i'),'monotonic_seconds':p.u(clock+0x40,'f'),'receive_time':p.u(clock+0x50,'i'),'triggers':[]}
engine=p.u(base+0x26bc880,'Q')
out['engine_tick_info']={'address':hex(engine),'hex':p.read(engine,0x30).hex(),'ticks_per_second':p.u(engine+4,'i')}
start,n=struct.unpack('<hh',p.read(content+0x2bd1c+0x1d*4,4))
for j in range(n):
 dx=p.u(content+0x2a51c+2*(start+j),'H');r=content+0x510+dx*0x38
 try:obj=resolve(p.u(r+16,'I'))+p.u(r+24,'Q');definition=resolve(p.u(obj,'I'))+p.u(obj+8,'Q')
 except (AssertionError,OSError):continue
 name=struct.unpack('<IBxH',p.read(definition+0x40,8))
 if name[0]!=0x992bb471 or name[2] not in (39,40,41):continue
 out['triggers'].append({'name':list(name),'component':hex(obj),'last_report_tick':p.u(obj+0x1ac,'i'),'armed':p.u(obj+0x1b0,'B'),'arm_tick':p.u(obj+0x1b4,'i')})
(st.ROOT/'trigger-clock-native-latest.json').write_text(json.dumps(out,indent=2));print(json.dumps(out,indent=2))
