"""Prepare one-variable item-level candidate; no files served to the current run."""
import os,json,re,struct,subprocess,hashlib
from pathlib import Path
root=Path(__file__).parent
oracle=r'F:\d1\d1preservation-main\d1-server\target\release\d1-oracle.exe'
env=dict(os.environ,D1_BUILDS=r'F:\d1\d1preservation-main\capture-rig\builds.json',D1_BUILD='later-56578')
source=Path(r'F:\d1\d1preservation-main\capture-rig\builds\later-56578\grants\khvostov-3598.bin')
def command(args):
 p=subprocess.run([oracle,*args],env=env,capture_output=True,text=True,timeout=90)
 if p.returncode: raise RuntimeError('Client codec failed; no served fixture changed')
 return p.stdout
text=command(['--json','object','--tag','0x80801732','--decode',source.read_bytes().hex()])
obj=json.loads(next(l for l in text.splitlines() if l.startswith('{')))
raw=bytearray(obj['struct_size'])
for off,value in obj['nonzero']:
 o=int(off,0);raw[o:o+8]=int(value,0).to_bytes(8,'little')
assert struct.unpack_from('<i',raw,0x18)[0]==40 and struct.unpack_from('<H',raw,0x24)[0]==3598
encoded=[]
for level in (40,1):
 body=bytearray(raw);struct.pack_into('<i',body,0x18,level)
 result=command(['object','--tag','0x80801732','--obj',body.hex()])
 assert 'object IDENTICAL' in result
 match=re.search(r'payload (\d+) bytes / (\d+) bits\s+([0-9a-f]+)',result)
 assert match
 wire=bytes.fromhex(match[3]);assert len(wire)==int(match[1])
 if level==40: assert wire==source.read_bytes(), 'Baseline roundtrip did not reproduce original wire'
 path=root/f'grant-level{level}-candidate.bin'
 with path.open('xb') as f:f.write(wire)
 with path.with_suffix('.log').open('x') as f:f.write(result)
 encoded.append({'level':level,'bytes':len(wire),'sha256':hashlib.sha256(wire).hexdigest(),'file':path.name})
report={'source_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),'content_index':3598,'changed_decoded_field':'+0x18 i32 item level 40 to 1 only','baseline_roundtrip_identical':True,'candidate_roundtrip_identical':True,'native_test':'NOT RUN','published':False,'variants':encoded}
with (root/'grant-level-candidate.json').open('x') as f:json.dump(report,f,indent=2)
print(json.dumps({k:v for k,v in report.items() if k!='variants'}))
