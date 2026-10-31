import runpy,json,struct,time
from pathlib import Path
v=runpy.run_path(str(Path(__file__).with_name('probe-postgame.py')))
p,b,r,c,out=v['p'],v['b'],v['r'],v['c'],v['out']
a=p.u(b+0x26adb30,'Q')+0xcc8
result={'time':time.time(),'scoreboard':hex(a),'count':p.u(a,'I'),'head':p.read(a,0x470).hex(),'tail':p.read(a+0x4588,0x398).hex(),'kinds':[]}
for k in (15,16):
 start,n=struct.unpack('<hh',p.read(c+0x2bd1c+k*4,4));assert 0<=n<=8
 for j in range(n):
  dx=p.u(c+0x2a51c+2*(start+j),'H');rec=c+0x510+dx*0x38
  cp=r.resolve(p.u(rec+16,'I'))+p.u(rec+24,'Q')
  result['kinds'].append({'kind':k,'component':hex(cp),'head':p.read(cp,0x1b8).hex(),'scorecount':p.u(cp+0x1b0,'I') if k==15 else None,'tail':p.read(cp+0x4ef0,0x30).hex() if k==15 else None})
dest=Path(__file__).with_name('scoreboard-%d.json'%time.time());dest.write_text(json.dumps(result,indent=2));print('SCOREBOARD',dest,json.dumps(result,indent=2))
