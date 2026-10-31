import json,struct
from pathlib import Path
rows=json.loads(Path(__file__).with_name('namespace-20.json').read_text())['rows']
for r in rows:
    raw=bytes.fromhex(r['data'])
    print(r['dx'],'state',raw[0x1b0], 'player_mask',raw[0x470:0x474].hex(), 'text-index',raw[0x1ec:0x1f0].hex())
run=Path(r'F:\d1\d1preservation-main\cfw-staging\2026-10-27-divide16')
for l in (run/'codex-obj17-handover.txt').read_text().splitlines():
    c=l.split()
    if len(c)>=7 and c[:2]==['0x992bb471','0x20']: print(c[2],c[6][:80],len(c[6])//2)
