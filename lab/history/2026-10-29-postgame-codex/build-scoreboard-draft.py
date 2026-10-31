"""Create an UNSERVED scoreboard list; no game/control writes or lab input."""
from pathlib import Path
import struct,json,os,subprocess,hashlib
P=Path(__file__).parent
S=Path(r'F:\d1\d1preservation-main\cfw-staging')
O=Path(r'F:\d1\public-work\target-claude\release\d1-oracle.exe')
env=dict(os.environ,D1_BUILD='later-56578',D1_BUILDS=r'F:\d1\d1preservation-main\capture-rig\builds.json')
cfg=(P/'owned-80aa3ee6.bin').read_bytes()
n,rel=struct.unpack_from('<Qq',cfg,0x40)
stats=0x48+rel+16
hashes=[struct.unpack_from('<I',cfg,stats+i*20)[0] for i in range(n)]
assert n==23 and hashes[0]==0x232708fc and hashes[18]==0x0ea6079d and hashes[22]==0xcfc7f8d0
identity=json.loads(Path(r'lab/history\2026-10-28-static-combat-codex\outro-player-identity-readonly.json').read_text())
b=bytearray(0x4d50)
struct.pack_into('<I',b,0,1)
b[0x10:0x18]=bytes.fromhex(identity['oracle_identity'])
struct.pack_into('<I',b,0x1c,n)
for index,value in [(0,50),(18,15),(22,2)]:struct.pack_into('<f',b,0x20+index*4,value)
struct.pack_into('<I',b,0x4d3c,0x811c9dc5)
obj=P/'scoreboard-one-player-A.bin';obj.write_bytes(b)
check=subprocess.run([str(O),'object','--tag','0x808008d0','--obj-file',str(obj),'--out',str(P/'scoreboard-one-player-A.wire.bin')],env=env,capture_output=True,text=True,timeout=90)
(P/'scoreboard-object-codec-check.txt').write_text(check.stdout+'\n'+check.stderr)
assert check.returncode==0 and 'object IDENTICAL' in check.stdout,(check.returncode,check.stdout,check.stderr)
base=(S/'agr-outro105-stop.draft.txt').read_text().splitlines()
rows=[];seen=0
for line in base:
 f=line.split('#',1)[0].split()
 if len(f)>=5 and f[0].startswith('0x'):
  if tuple(int(x,0) for x in f[:3])==(0x4786c0e0,15,0):
   line=' '.join(f[:5]+['0x808008d0',b.hex()]);seen+=1
  rows.append(line)
assert seen==1 and len(rows)==22
header='''# PRIVATE UNSERVED DRAFT. Owner publishes after outro, while phase is STILL 3.
# Do NOT combine @phase 7 with the first scoreboard delivery.
# Publish this list, wait >= one sensor resend (~10 s), verify native/UI counts=1,
# THEN publish phase 7 separately, preferably retaining this scoreboard A row.
# Recheck authority/generation/group and identity on sense50 before publishing.
# @authority 13,2,0 is the content/lab-derived outro setting from sense44..49.
# Row identity is codec-image d100000000000201 (native 01020000000000d1),
# the same kind-0x123 native player identity as cinematic A+0xa0 and bundle A+0x40.
# Recheck against the fresh player: do not substitute the account SOID.
# Content-derived: kind15/sub0 schema808008d0, size4d50; rows+0, row0+10 stride4a0;
# row identity+0, bool+8, statcount+c, stats+10; row rewards count+110/entries+120.
# Content-derived scoreboard config80aa3ee6 stats count23: kills index0,
# precision_kills index18, deaths index22; FNV1 hashes232708fc/0ea6079d/cfc7f8d0.
# Content-derived current display category is empty FNV1 811c9dc5 at A+4d3c.
# INFERRED/SYNTHETIC: one row, row bool0 (meaning unresolved), kills50,
# precision15, deaths2, all other 20 stats0; rewards0 (empty, no invented grant).
# INFERRED: team rows0, other aggregate counts0, time fields0; padding0.
# Retains the 21 common clients + cine29 stop counter3 from the prior outro draft.
# Their identities/images and authority must be reconciled with fresh sense50.
# The native scoreboard definition handle component+4f00 is selected locally,
# not an A-image field. It was valid80aa3ee6 on sense49; setter bumps+4f08 itself.
@authority 13,2,0
'''
draft=S/'agr-postgame-scoreboard.draft.txt'
draft.write_text(header+'\n'.join(rows)+'\n',encoding='utf-8')
manifest={'published':False,'draft':str(draft),'row_count':len(rows),'schema':'0x808008d0','size':len(b),'identity':identity,'synthetic_stats':{'kills':50,'precision_kills':15,'deaths':2},'stat_hashes':[hex(x) for x in hashes],'object_sha256':hashlib.sha256(b).hexdigest(),'draft_sha256':hashlib.sha256(draft.read_bytes()).hexdigest(),'codec_stdout':check.stdout}
(P/'scoreboard-draft-manifest.json').write_text(json.dumps(manifest,indent=2))
print(json.dumps({k:v for k,v in manifest.items() if k not in ('stat_hashes','codec_stdout')},indent=2));print(check.stdout[:1600])
