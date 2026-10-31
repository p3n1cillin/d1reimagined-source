from pathlib import Path
import sqlite3,json,struct,re
P=Path(__file__).parent
def fnv(s):
 h=0x811c9dc5
 for c in s.encode():h=((h*0x1000193)&0xffffffff)^c
 return h
c=sqlite3.connect('file:F:/d1/d1preservation-main/manifest/data/world_en.sqlite?mode=ro',uri=True)
print('ships')
ships=[]
for i,j in c.execute('select id,json from DestinyInventoryItemDefinition where json like ?',('%Arcadia%',)):
 d=json.loads(j);v={k:v for k,v in d.items() if k in ['itemName','itemTypeName','itemDescription','itemHash','itemIndex','equippingBlock','bucketTypeHash']};ships.append({'hash':hex(i&0xffffffff),'definition':v});print(ships[-1])
print('activity names')
for h in (1686739444,1299744814):
 row=c.execute('select json from DestinyActivityTypeDefinition where id=?',(h,)).fetchone()
 print('activity type',hex(h),json.loads(row[0]) if row else None)
for h in (1315532320,):
 row=c.execute('select json from DestinyInventoryItemDefinition where id=?',(h,)).fetchone()
 print('reward',hex(h),json.loads(row[0]) if row else None)
for h in (0x6e0dae89,0x6eaf0959):
 row=c.execute('select json from DestinyActivityDefinition where id=?',(h,)).fetchone()
 d=json.loads(row[0]) if row else {}
 print(hex(h),{k:v for k,v in d.items() if k in ['activityName','activityDescription','activityTypeHash','activityLevel','placeHash','destinationHash','activityHash','rewards','isPlaylist','activityModeHash','activityUnlockStrings']})
src=Path(r'F:\d1\public-work\d1reimagined-sense\ps4\d1-server\crates\d1-activity-host\src\worlds.rs').read_text(encoding='utf-8')
print('world name hashes',[(name,hex(fnv(name))) for name in re.findall(r'name: "([^"]+)"',src)])
post=json.loads((P/'postgame-1793234336.json').read_text())
print('activity headers')
for a in post['activities']:
 b=bytes.fromhex(a['head'])
 print(a['index'],'\n'+'\n'.join(f'{i:02x} '+b[i:i+16].hex(' ') for i in range(0,0xb0,16)))
(P/'arcadia-naming-corpus.json').write_text(json.dumps(ships,indent=2))
