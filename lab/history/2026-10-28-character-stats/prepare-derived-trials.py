from pathlib import Path
import hashlib,json,os,shutil,struct,subprocess

root=Path(__file__).resolve().parent
base=Path(r'F:\d1\d1preservation-main')
build=base/'capture-rig/builds/later-56578'
old=root.parent/'2026-10-27-weapon-stats'
exe=base/'d1-server/target/release/d1-oracle.exe'
env=dict(os.environ,D1_BUILD='later-56578',D1_BUILDS=str(base/'capture-rig/builds.json'))

def command(args, ok=True):
 r=subprocess.run([str(exe),*map(str,args)],cwd=base,env=env,text=True,capture_output=True,timeout=120)
 if ok: assert r.returncode==0,r.stderr[-1800:]
 return r

def decode(f,tag=0x808017fd):
 r=command(['--json','object','--tag',hex(tag),'--decode',f.read_bytes().hex()])
 x=json.loads(next(l for l in r.stdout.splitlines() if l.startswith('{')))
 b=bytearray(x['struct_size'])
 for off,val in x['nonzero']:
  at=int(off,0);b[at:at+8]=int(val,0).to_bytes(8,'little')
 return b

inventory=root/'inventory-with-grant';inventory.mkdir(exist_ok=True)
for f in (build/'inventory-tutorial').glob('*.bin'):shutil.copy2(f,inventory/f.name)
shutil.copy2(build/'grants/khvostov-3598.bin',inventory/'khvostov-3598.bin')
rifle=decode(inventory/'khvostov-3598.bin',0x80801732)
(root/'instance-ci3598.decoded.bin').write_bytes(rifle)
shutil.copy2(old/'owned-khvostov-definition.bin',root/'owned-definition-ci3598.bin')
report=[]
for name,source,table,key in [
 ('roster-defense1200',old/'roster-base-stats','character',1),
 ('roster-defense1200-unarmed',build/'rosters-f0-tutorial','character',1),
 ('roster-attack-derived',old/'roster-base-stats','primary',11),
]:
 name+='-scalar-corrected'
 out=root/name;out.mkdir(exist_ok=True)
 for f in source.glob('*.bin'):
  if not f.name.startswith('f0t1-'):shutil.copy2(f,out/f.name)
 peer=source/'f0t1-d100000000000501.bin';dest=out/peer.name
 r=command(['character','derived-stat','--input',peer,'--out',dest,'--inventory',inventory,'--table',table,'--stat',key])
 (root/(name+'-cli.log')).write_text(r.stdout+r.stderr)
 result=json.loads(next(l for l in r.stdout.splitlines() if l.startswith('{')))
 before,after=decode(peer),decode(dest)
 allowed=range(0x840,0x848) if table=='character' else range(0x8b8,0x938)
 diff=[i for i,(a,b) in enumerate(zip(before,after)) if a!=b]
 assert len(before)==len(after)==2632 and diff and all(i in allowed for i in diff)
 assert before[0x838:0x840]==after[0x838:0x840]
 refusal=command(['character','derived-stat','--input',peer,'--out',dest,'--inventory',inventory,'--table',table,'--stat',key],ok=False)
 assert refusal.returncode!=0
 report.append(dict(name=name,result=result,decoded_changed_offsets=[hex(i) for i in diff],light_unchanged=True,existing_output_refused=True,sha256=hashlib.sha256(dest.read_bytes()).hexdigest(),native='NOT SERVED'))
(root/'derived-candidates-corrected.json').write_text(json.dumps(report,indent=2))
print(json.dumps(report))
