#!/bin/bash
S=lab/history/lab-scripts-2026-10-29; P=lab/history/2026-10-28-static-combat-codex
cd $P && PYTHONPATH=$P timeout 60 python $S/read-pilots.py $S/pilots-$1.json >/dev/null 2>&1
python -c "
import json,struct
d=json.load(open(r'$S/pilots-$1.json'))
for r in d['rows']:
    if r.get('kind')!=2 or 'state_hex' not in r: continue
    b=bytes.fromhex(r['state_hex'])
    print(r['owned_tag'],'gen',struct.unpack_from('<I',b,0)[0],'enabled',b[6],'rev',r['program_revision'],'count',r['program_count'],'entity',r['entity_handle'],'progress',r['progress'][:16])"
