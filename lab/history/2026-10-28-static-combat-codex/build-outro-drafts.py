from pathlib import Path
import struct,json,datetime,subprocess,os,hashlib
P=Path(r'lab/history\2026-10-28-static-combat-codex');S=Path(r'F:\d1\d1preservation-main\cfw-staging')
source=S/'agr-dock13-obj26-m52.txt';text=source.read_text();identity=json.loads((P/'outro-player-identity-readonly.json').read_text())['oracle_identity'];ident=bytes.fromhex(identity)
assert ident==bytes.fromhex('d100000000000201')

def key(line):
 r=line.partition('#')[0].split()
 try:return tuple(int(v,0) for v in r[:3]) if len(r)>=5 else None
 except ValueError:return None

def change(lines,wanted,tag,image):
 hit=0;result=[]
 for l in lines:
  if key(l)==wanted:
   fields=l.partition('#')[0].split();l=' '.join(fields[:5]+[hex(tag),image.hex()]+fields[7:]);hit+=1
  result.append(l)
 assert hit==1,(wanted,hit)
 return result

def cine(counter,playing):
 b=bytearray(0xd4);struct.pack_into('<I',b,4,counter);b[8]=int(playing)
 struct.pack_into('<IBxH',b,12,0x811c9dc5,0xff,0xffff);b[20]=0xff
 if playing:
  struct.pack_into('<I',b,0x9c,1);b[0xa0:0xa8]=ident
 return b

header=['# PRIVATE UNPUBLISHED DRAFT. No live control or canonical schedule was changed.', '# Source authority13 is provisional for future arrivals: recheck local machine and pending clients.']
clear=bytearray(0x2f0);clear[0x2c0:0x2c4]=b'\xff'*4
lines=change(text.splitlines(),(0x992bb471,0x20,26),0x80802a1b,clear)
files={'agr-outro104-clear26-request105.draft.txt':'\n'.join(header+lines)+'\n'}
# Slice105 loads four groups: 96e0a5e5(empty), 992bb471(only cinematic29),
# 3c2f5ed5(no active sensor definitions), 4786c0e0(common game-mode clients).
# Preserve the 21 canonical common rows. The first cinematic cast identity is at A0, not A8.
story=[line for line in text.splitlines() if key(line) and key(line)[0]==0x4786c0e0]
assert len(story)==21
bundle=[bytes.fromhex(line.split()[6])[0x40:0x48] for line in story if len(line.split())>6 and line.split()[5].lower()=='0x808022a5']
assert bundle==[ident],bundle
for label,counter,playing in [('register',1,False),('play',2,True),('stop',3,False)]:
 image=cine(counter,playing);(P/f'outro-cine29-{label}-A.bin').write_bytes(image)
 files[f'agr-outro105-{label}.draft.txt']='\n'.join(header+['@authority 13,2,0', '# PREDICTED 22-client set, not a fresh native pending capture. Reconcile after arriving in105.']+story+[f'0x992bb471 0x6 29 1 0 0x8080254d {image.hex()}'])+'\n'
for name,data in files.items():
 dest=S/name
 if dest.exists() and dest.read_text()!=data:raise FileExistsError(dest)
 if not dest.exists():dest.write_text(data,encoding='utf-8')
steps=f'''# PRIVATE UNPUBLISHED: not the active agr-steps-divide.txt. PR63 host required.
# Phase A: this ONE line can be appended after the objective26 step only after review.
# Fresh current-world admissions required for every Dock13 squad; deadline HOLDs forever.
164 !0x062dd95d:5,6,7,8,9,10,11,12|600 0x4786c0e0 {S.as_posix()}/agr-outro104-clear26-request105.draft.txt 105
# STOP HERE. Verify actual CURRENT slice105 (udt21), current local authority/generation,
# fresh pending-client list and player identity. Do NOT append %105|N -> play:
# that old step kind still has a timeout fallback, which is not proof of arrival.
# Phase B (a separate supervised schedule, or manual clients publication after validation):
# 164 0 0x4786c0e0 {S.as_posix()}/agr-outro105-register.draft.txt
# 164 +2 0x4786c0e0 {S.as_posix()}/agr-outro105-play.draft.txt
# 164 @891,1726|150 0x4786c0e0 {S.as_posix()}/agr-outro105-stop.draft.txt
# The 150s timeout is a bounded diagnostic stop, NEVER mission success, rewards or progression.
# Do not replace a live run's immutable loaded steps; use its owner-controlled test path.
'''
dest=S/'agr-outro-steps.draft.txt'
if dest.exists() and dest.read_text()!=steps:raise FileExistsError(dest)
if not dest.exists():dest.write_text(steps,encoding='utf-8')
manifest={'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'source':str(source),'source_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),'identity':identity,'cast_image_offset':'0xa0','cast_native_instructions':['0x4037e7 count component+0x248','0x40380f load component+0x24c','0x40316a image at component+0x1ac'],'files':[{'path':str(S/n),'sha256':hashlib.sha256(d.encode()).hexdigest()} for n,d in files.items()],'predicted_slice105_clients':22,'published':False}
(P/'outro-draft-manifest.json').write_text(json.dumps(manifest,indent=2));print(json.dumps(manifest,indent=2))
