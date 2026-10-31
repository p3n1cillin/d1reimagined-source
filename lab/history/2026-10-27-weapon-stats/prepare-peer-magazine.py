from pathlib import Path
import os,json,re,struct,subprocess,shutil,hashlib
root=Path(r'lab/history\2026-10-27-weapon-stats')
source=Path(r'F:\d1\d1preservation-main\capture-rig\builds\later-56578\grants\rosters-f0-tutorial-khv')
out=root/'roster-magazine44';out.mkdir()
for f in source.iterdir():
 if f.is_file():shutil.copy2(f,out/f.name)
peer=source/'f0t1-d100000000000501.bin'
oracle=r'F:\d1\d1preservation-main\d1-server\target\release\d1-oracle.exe'
env=dict(os.environ,D1_BUILDS=r'F:\d1\d1preservation-main\capture-rig\builds.json',D1_BUILD='later-56578')
def command(args):
 r=subprocess.run([oracle,*args],env=env,capture_output=True,text=True,timeout=90)
 assert r.returncode==0,r.stderr[:1000]
 return r.stdout
s=command(['--json','object','--tag','0x808017fd','--decode',peer.read_bytes().hex()])
x=json.loads(next(l for l in s.splitlines() if l.startswith('{')))
b=bytearray(x['struct_size'])
for off,value in x['nonzero']:
 o=int(off,0);b[o:o+8]=int(value,0).to_bytes(8,'little')
assert len(b)==2632
assert struct.unpack_from('<H',b,0x30+0x1e8+6*64+8)[0]==3598
assert not any(b[0x8b8:0x938])
for name,new in [('baseline',b),('magazine44',bytearray(b))]:
 if name=='magazine44':new[0x8b8]=20;struct.pack_into('<I',new,0x8bc,44)
 text=command(['object','--tag','0x808017fd','--obj',new.hex()]);assert 'object IDENTICAL' in text
 m=re.search(r'payload (\d+) bytes / (\d+) bits\s+([0-9a-f]+)',text);assert m
 wire=bytes.fromhex(m[3]);assert len(wire)==int(m[1])
 if name=='baseline':assert wire==peer.read_bytes()
 else:(out/peer.name).write_bytes(wire)
 (root/f'peer-{name}-codec.log').write_text(text)
(root/'peer-baseline.decoded.bin').write_bytes(b)
(root/'peer-magazine44.decoded.bin').write_bytes(new)
report={'changed_decoded_field':'family0/type1 primary weapon-stat table first pair at+0x8B8/+0x8BC','candidate':[20,44],'content_source':'owned Khvostov def+0x50 stat20=44 (56578); not display capacity25','other_decoded_bytes_unchanged':all(a==z for i,(a,z) in enumerate(zip(b,new)) if i not in range(0x8b8,0x8c0)),'baseline_wire_identical':True,'native_test':'NOT RUN','published':False}
(root/'peer-magazine44-candidate.json').write_text(json.dumps(report,indent=2));print(json.dumps(report))
