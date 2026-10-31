from pathlib import Path
import struct,json
P=Path(r'lab/history\2026-10-29-postgame-codex')
b=(P/'owned-80a5ffcf.bin').read_bytes(); n=struct.unpack_from('<I',b,8)[0]; assert n==124 and len(b)==0x30+n*24
rows=[{'index':i,'hash':f'{struct.unpack_from("<I",b,0x30+24*i)[0]:08x}','tag':f'{struct.unpack_from("<I",b,0x40+24*i)[0]:08x}'}for i in range(n)]
hits=[]
for p in (P/'tower-owned').glob('*.bin'):
 d=p.read_bytes()
 for r in rows:
  needle=bytes.fromhex(r['hash'])[::-1]; start=0
  while (off:=d.find(needle,start))>=0:
   hits.append(dict(r,asset=p.stem,offset=hex(off)));start=off+4
out=Path(__file__).parent
(out/'vendor-hash-table.json').write_text(json.dumps(rows,indent=2))
(out/'vendor-hash-hits.json').write_text(json.dumps(hits,indent=2))
print('rows',n,'hits',len(hits)); print(json.dumps([x for x in hits if x['asset'] in ('80c880b7','80c885aa','80c88171')],indent=2));print(json.dumps(hits[:20],indent=2))
