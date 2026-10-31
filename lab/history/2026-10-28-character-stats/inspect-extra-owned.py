from pathlib import Path
import csv,hashlib,json,os,struct,subprocess
root=Path(r'lab/history\2026-10-28-character-stats')
base=Path(r'F:\d1\d1preservation-main');exe=base/'d1-server/target/release/d1-oracle.exe'
env=dict(os.environ,D1_BUILDS=str(base/'capture-rig/builds.json'),D1_BUILD='later-56578')
table={int(r['content_index']):r for r in csv.DictReader([l for l in (base/'capture-rig/builds/later-56578/item-content-index.tsv').read_text(encoding='utf-8').splitlines() if not l.startswith('#')],delimiter='\t')}
def pairs(b,field):
 rel=struct.unpack_from('<q',b,field)[0]
 if not rel:return None
 rec=field+rel;assert 0<=rec<=len(b)-16
 n,rel2=struct.unpack_from('<qq',b,rec);assert 0<=n<=64
 start=rec+8+rel2+16;assert 0<=start<=len(b) and start+8*n<=len(b)
 return [struct.unpack_from('<II',b,start+8*i) for i in range(n)]
out=[]
for ci in [4531,4286]:
 row=table[ci];tag=int(row['world_tag'],0);dest=root/f'owned-definition-ci{ci}.bin'
 assert not dest.exists()
 r=subprocess.run([str(exe),'oodle','entry','--tag',hex(tag),'--out',str(dest)],env=env,encoding='utf-8',capture_output=True,timeout=90);assert r.returncode==0,r.stderr
 b=dest.read_bytes();h=struct.unpack_from('<I',b,0x78)[0];assert h==int(row['item_hash'],0)
 rec={'ci':ci,'tag':hex(tag),'hash':hex(h),'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest(),'base_stats':pairs(b,0x50),'instance_defaults':pairs(b,0x68)}
 out.append(rec);print(json.dumps(rec),flush=True)
(root/'owned-extra-definitions.json').write_text(json.dumps(out,indent=2),encoding='utf-8')
