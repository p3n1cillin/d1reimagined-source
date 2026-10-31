"""Offline owned-byte inventory; does not attach or change a control list."""
from pathlib import Path
import struct,json,hashlib

P=Path(__file__).parent
S=P.parent/'2026-10-28-static-combat-codex'

def fnv1(text):
 h=0x811c9dc5
 for b in text.lower().encode():h=((h*0x1000193)&0xffffffff)^b
 return h

labels={fnv1(x):x for x in ('dreg','vandal','captain','shank','servitor','boss','default')}
rows=[]
for sq in range(5,13):
 entity_tag=0x80cb86bf+sq-5;template_tag=0x80cc3830+sq-5
 entity=(S/f'owned-definition-{entity_tag:08x}.bin').read_bytes()
 b=(P/f'owned-{template_tag:08x}.bin').read_bytes()
 Q=lambda o:struct.unpack_from('<Q',b,o)[0]
 q=lambda o:struct.unpack_from('<q',b,o)[0]
 I=lambda o:struct.unpack_from('<I',b,o)[0]
 assert struct.unpack_from('<I',entity,0x540)[0]==template_tag
 assert I(0)==len(b)
 vs=0x28+q(0x28)+16;ss=0x48+q(0x48)+16
 row={'squad':sq,'entity_definition':hex(entity_tag),'template':hex(template_tag),'template_sha256':hashlib.sha256(b).hexdigest(),'name_hash':hex(I(8)),'flags_14':hex(I(0x14)),'variants':[],'inline_spawn_points':[]}
 for i in range(Q(0x20)):
  o=vs+i*0x38;name=I(o)
  row['variants'].append({'index':i,'name_hash':hex(name),'name':labels.get(name)})
 for i in range(Q(0x40)):
  o=ss+i*0x90
  row['inline_spawn_points'].append({'offset':hex(o),'position':struct.unpack_from('<3f',b,o),'radius':struct.unpack_from('<f',b,o+12)[0],'field_80':hex(I(o+0x80)),'point_name_84':hex(I(o+0x84)),'field_88':hex(I(o+0x88)),'flags_8c':hex(I(o+0x8c))})
 snap=(P/'snapshot-1793214003'/f'062dd95d-{sq}-squad.bin').read_bytes()
 row['sense43_assignment']={'objective_ref_bytes':snap[0x3e0:0x3e8].hex(),'slot_3ec':struct.unpack_from('<i',snap,0x3ec)[0],'resolved_task_3f0':struct.unpack_from('<i',snap,0x3f0)[0]}
 rows.append(row)
(P/'authored-squads.json').write_text(json.dumps(rows,indent=2))
print(json.dumps(rows,indent=2))
