from pathlib import Path
import os,subprocess,json,struct
root=Path(r'lab/history\2026-10-28-character-stats');base=Path(r'F:\d1\d1preservation-main');exe=base/'d1-server/target/release/d1-oracle.exe'
env=dict(os.environ,D1_BUILD='later-56578',D1_BUILDS=str(base/'capture-rig/builds.json'))
def decode(path,tag):
 r=subprocess.run([str(exe),'--json','object','--tag',hex(tag),'--decode',path.read_bytes().hex()],env=env,encoding='utf-8',capture_output=True,check=True)
 x=json.loads(next(l for l in r.stdout.splitlines() if l.startswith('{')));b=bytearray(x['struct_size'])
 for off,val in x['nonzero']:
  a=int(off,0);b[a:a+8]=int(val,0).to_bytes(8,'little')
 return b
items=[]
for f in (base/'capture-rig/builds/later-56578/inventory-tutorial').iterdir():
 if f.suffix!='.bin':continue
 b=decode(f,0x80801732);ci=struct.unpack_from('<H',b,0x24)[0];soid=struct.unpack_from('<Q',b)[0]
 r={'file':f.name,'ci':ci,'soid':hex(soid),'level':struct.unpack_from('<i',b,0x18)[0],'curve_offset':struct.unpack_from('<h',b,0x20)[0],'grid':struct.unpack_from('<H',b,0x58)[0]}
 (root/f'instance-ci{ci}.decoded.bin').write_bytes(b);items.append(r)
peer=decode(Path(r'lab/history\2026-10-27-weapon-stats\roster-base-stats\f0t1-d100000000000501.bin'),0x808017fd)
slots=[{'slot':i,'soid':hex(struct.unpack_from('<Q',peer,0x218+64*i)[0]),'ci':struct.unpack_from('<H',peer,0x220+64*i)[0]} for i in range(20) if struct.unpack_from('<H',peer,0x220+64*i)[0]!=0xffff]
(root/'staged-item-fields.json').write_text(json.dumps({'items':items,'peer_slots':slots},indent=2))
print(json.dumps({'items':items,'peer_slots':slots}))
