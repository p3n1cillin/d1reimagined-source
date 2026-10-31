"""Private unpublished drafts derived from owned metadata. No live control."""
import struct,json,subprocess,os,importlib.util
from pathlib import Path
P=Path(__file__).parent;G=0x062dd95d
records={}
for file in P.glob('owned-definition-*.bin'):
 b=file.read_bytes();needle=struct.pack('<I',G)
 for pos in range(0,len(b)-32,8):
  if b[pos:pos+4]!=needle:continue
  key,kind,sub=struct.unpack_from('<IBxH',b,pos)
  # Metadata registers its own namespace id, domain, sense tag and auth tag.
  if (kind,sub) not in [(4,0),(4,1),(4,2),(4,3),(3,4),*[(1,i) for i in range(5,13)],(4,13),(0x21,14),*[(0x1d,i) for i in range(15,19)],(0x2b,42)]:continue
  domain,flags,_,sense,auth=struct.unpack_from('<IIIII',b,pos+8)
  if domain not in (13,0xffffffff) or not 0x80800000<=auth<=0x80810000:continue
  row={'kind':kind,'sub':sub,'a':auth,'b':sense,'definition':file.name,'metadata_offset':pos}
  assert (kind,sub) not in records or records[kind,sub]==row
  records[kind,sub]=row
(P/'dock-client-metadata.json').write_text(json.dumps(list(records.values()),indent=2))
base=(P/'skiff-candidate-base.txt').read_text()
lines=base.splitlines();lines.insert(1,'# DRAFT: recheck authority machine on actual slice104 arrival. Existing 12 is provisional.')
for (kind,sub),row in sorted(records.items()):
 lines.append(f'{G:#010x} {kind:#x} {sub} 1 {int(row["b"]!=0xffffffff)}')
# Arm first-use trigger40 at simulation tick0. Native initial report time is -1.
def set_a(lines,g,k,sub,tag,raw):
 hit=0;out=[]
 for line in lines:
  fields=line.partition('#')[0].split()
  try:match=len(fields)>=5 and tuple(int(v,0) for v in fields[:3])==(g,k,sub)
  except ValueError:match=False
  if match:
   fields[5:7]=[hex(tag),raw.hex()];line=' '.join(fields);hit+=1
  out.append(line)
 if not hit:out.append(f'{g:#010x} {k:#x} {sub} 1 0 {tag:#x} {raw.hex()}')
 return out
lines=set_a(lines,0x992bb471,0x1d,40,0x80800463,struct.pack('<II',1,0))
(P/'agr-dock-register-arm40.draft.txt').write_text('\n'.join(lines)+'\n')
env=dict(os.environ,D1_BUILD='later-56578',D1_BUILDS=r'F:\d1\d1preservation-main\capture-rig\builds.json')
exe=r'F:\d1\public-work\target-codex-static\release\d1-oracle.exe'
def objective(source,out,active,clear,marker):
 r=subprocess.run([exe,'sensor','objective','--input',str(source),'--out',str(out),'--group','0x992bb471','--active',str(active),'--clear',str(clear),'--marker',str(marker),'--machine','12'],capture_output=True,text=True,env=env,timeout=60)
 assert not r.returncode,r.stderr;print(r.stdout.strip())
find=P/'agr-dock-find23-m60.draft.txt';clear=P/'agr-dock-clear26-m52.draft.txt'
objective(P/'agr-dock-register-arm40.draft.txt',find,23,20,60)
fl=find.read_text().splitlines();fl=set_a(fl,0x992bb471,0x1d,40,0x80800463,b'\0'*8)
fl=set_a(fl,G,0x1d,16,0x80800463,struct.pack('<II',1,0))
find.write_text('\n'.join(fl)+'\n')
objective(find,clear,26,23,52)
cl=set_a(clear.read_text().splitlines(),G,0x1d,16,0x80800463,b'\0'*8)
cl=set_a(cl,0x992bb471,0x1d,41,0x80800463,b'\0'*8)
clear.write_text('\n'.join(cl)+'\n')
# Diagnostic wave only: not an asserted retail spawn count or all-eight cohort.
spec=importlib.util.spec_from_file_location('sq',r'F:\d1\public-work\d1reimagined-sense\tools\ps4_setup\d1-squad-states.py');sq=importlib.util.module_from_spec(spec);spec.loader.exec_module(sq)
wave=clear.read_text().splitlines()
for sub in (5,6):wave=set_a(wave,G,1,sub,0x8080071e,sq.squad(1,1))
(P/'agr-dock-clear26-init-wave.draft.txt').write_text('\n'.join(wave)+'\n')
schedule=(P/'agr-dock-steps.draft.txt')
schedule.write_text(f'''# UNPUBLISHED. Graft after validated Find-the-Ship step17. No active steps file edited.
# Register actual pending Dock clients at slice104, after rechecking local authority.
164 %104|14400 0x4786c0e0 {(P/'agr-dock-register-arm40.draft.txt').as_posix()}
164 ^0x992bb471/0x1d/40|14400 0x4786c0e0 {find.as_posix()}
164 ^0x062dd95d/0x1d/16|14400 0x4786c0e0 {clear.as_posix()}
# ALTERNATE trigger41 is also authored for objective23; choose from native reports,
# do not require both sequentially or use timeout as proof of entering the room.
# No executable !GROUP completion: it currently times out and counts only the
# seen-alive subset. Need expected admitted-wave cohort + strict zero receipt.
# Then clear objective26, verify actual mission completion, request phase04/outro105.
# Do not auto-launch cinematic29 from the current !GROUP timeout.
''')
print('Dock metadata records',len(records),'(not a live pending-client capture)')
