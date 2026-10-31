"""Known bounded content records, READ + QUERY process handle, no input or writes."""
import runpy,struct,json,time
from pathlib import Path
P=Path(__file__).parent
v=runpy.run_path(str(P/'probe-launch-readonly.py'))
u,rd,base=v['u'],v['rd'],v['base']
def resolve(tag):
    signed=struct.unpack('<i',struct.pack('<I',tag))[0]>>13
    slot=(((signed&0xffffffff)>>18)|0x3ff)&signed&0xffffffff
    rec=u(u(base+0x1a238a0))+slot*64
    data=u(rec+0x10);stride=u(rec+0x30,'I');mask=u(rec+0x34,'i')&0xffffffffffffffff
    assert data and 0<stride<0x10000
    cell=data+(tag&0x1fff)*stride
    return (cell-(mask&u(cell+8)))&0xffffffffffffffff
manager=u(base+0x26ca088)
tag=u(manager+0x278,'I')
out={'time':time.time(),'base':hex(base),'scenario_tag':hex(tag)}
if tag!=0xffffffff:
    a=resolve(tag);head=rd(a,0x60);n=u(a+0x48,'q');assert 0<=n<=64
    name=rd(a+0x30+u(a+0x30,'q'),100).split(b'\0')[0].decode('ascii')
    out['scenario']={'address':hex(a),'name':name,'head':head.hex(),'private':hex(u(a+12,'I')),'public':hex(u(a+16,'I')),'bubbles':n,'bubbles_raw':rd(a+0x50+u(a+0x50,'q')+16,n*24).hex()}
root=u(base+0x26527f8)
bucket_tag=u(root+0x108,'I');a=resolve(bucket_tag);n=u(a+0x8c,'I');assert n==39
data=a+0x90
out['bucket_tag']=hex(bucket_tag)
out['bucket_head']=rd(a,0x80).hex()
out['ship_buckets']=[]
for i in range(n):
    q=data+20*i
    if u(q,'B')==12:
        out['ship_buckets'].append({'id':12,'first':u(q+4,'i'),'count':u(q+8,'i'),'scope':u(q+16,'B')})
out['ui_scoreboard_count']=u(u(base+0x26adb30)+0xcc8,'I')
index=u(base+0x25d6ff8,'H');table=base+0x1ad5000
content=u(table+u(table,'q')+index*u(table+0x10)+0x30)
out['replicas']=[]
for kind in (15,16):
    start,count=struct.unpack('<hh',rd(content+0x2bd1c+kind*4,4));assert 0<=count<=8
    for j in range(count):
        slot=u(content+0x2a51c+2*(start+j),'H');rec=content+0x510+slot*0x38
        component=resolve(u(rec+16,'I'))+u(rec+24)
        out['replicas'].append({'kind':kind,'component':hex(component),'row_count':u(component+0x1b0,'I')} if kind==15 else {'kind':kind,'component':hex(component),'phase':u(component+0x1ac,'b')})
col=u(base+0x26bc040)
if col:out['collector']={'state':u(col,'b'),'target':u(col+1,'b'),'activity':u(col+2,'h'),'xp':struct.unpack('<III',rd(col+0x10,12)),'rewards':u(col+0x258,'I')}
dest=P/('world-record-readonly-%d.json'%time.time());dest.write_text(json.dumps(out,indent=2))
print(dest);print(json.dumps(out,indent=2))
