"""Bounded READ+QUERY-only sampler. No input, memory writes, debugger or native calls.
Run beside an OWNER-operated triangle test; never starts a test itself.
"""
import argparse,runpy,time,json,struct,datetime
from pathlib import Path
P=Path(__file__).parent
ap=argparse.ArgumentParser();ap.add_argument('--seconds',type=float,default=12);args=ap.parse_args()
assert 0<args.seconds<=30
# Reuse the audited READ+QUERY-only attachment and image validation.
g=runpy.run_path(str(P/'probe-launch-readonly.py'));u=g['u'];rd=g['rd'];b=g['base']
primary=u(u(b+0x26519d8)+0x790,'i');assert 0<=primary<4
inp=b+0x273ed40+0x2a0+primary*0x300
rows=[];previous=None;start=time.monotonic()
print('READY: read-only sampling for',args.seconds,'seconds; owner may operate input now.',flush=True)
while time.monotonic()-start<args.seconds:
 root=u(b+0x26ca4e0);slot=u(root+8,'i');assert 0<=slot<=1
 session=root+slot*0x98e0+0x10;par=session+0x6b28+0xeb8
 raw=rd(inp+0x100,0x20);flags=rd(b+0x279ce58,0x11);countdown=rd(par+0xe0,0x20)
 row={'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'held106':raw[6],
      'consumed108':struct.unpack_from('<I',raw,8)[0],
      'route110':struct.unpack_from('<I',raw,16)[0],'route114':struct.unpack_from('<I',raw,20)[0],
      'hold_elapsed5c':struct.unpack_from('<I',flags,4)[0],
      'ordinary64':flags[12],'alternate65':flags[13],'reward66':flags[14],
      'rewardhold67':flags[15],'edge68':flags[16],
      'screen_gate11b':u(b+0x2757090+0x11b,'B'),
      'context_state':u(b+0x279d158+0x104,'i'),
      'world_state':u(u(b+0x26ca078)+0x1b0,'i'),
      'handler31':u(b+0x26ce420+0x31,'B'),'handler32':u(b+0x26ce420+0x32,'B'),
      'recall_valid':countdown[0]&1,'recall_active':countdown[28],
      'recall_seconds':struct.unpack_from('<i',countdown,8)[0]}
 stable={k:v for k,v in row.items() if k!='utc'}
 if stable!=previous: rows.append(row);print(json.dumps(row),flush=True);previous=stable
 time.sleep(.05)
dest=P/f'postgame-input-sample-{int(time.time())}.json';dest.write_text(json.dumps({'base':hex(b),'primary':primary,'seconds':args.seconds,'changes':rows},indent=2));print(dest)
