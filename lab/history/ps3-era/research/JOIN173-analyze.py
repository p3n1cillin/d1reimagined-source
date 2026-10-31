from pathlib import Path
from datetime import datetime, timezone
from collections import Counter
import json
import re
import struct
import sys

rt = Path(r'F:\d1\<pc>-import\d1xenia\local')
logs = rt / 'SIGNON_LOCAL/logs'
rows = [json.loads(line) for line in (rt/'RESEARCH/JOIN173-session-timeline.jsonl').read_text().splitlines()]
native = (logs/'JOIN173-pre-stop-native.log').read_text(encoding='utf-8', errors='replace')
hits = []
for line in native.splitlines():
    if 'PPU: D1P ' not in line:
        continue
    registers = dict(re.findall(r'\b(pc|lr|r[0-9]+)=(0x[0-9a-f]+)', line))
    hit = {'native_time': re.search(r'\d+:\d\d:\d\d\.\d+', line)[0], **registers,
           'memory': dict(re.findall(r'\b(m[0-9]+(?:\+[0-9a-f]+)?)=([0-9a-f]+)', line))}
    hits.append(hit)
sessions = [s for row in rows for s in row['sessions']]
loaded = [r for r in rows if r['loaded_words'][112//32] & (1 << (112 % 32)) and r['active_loading'] == [-1, -1]]
pending = [r for r in rows if any(s.get('context', {}).get('pending_20') == 2 for s in r['sessions'])]
state_counts = Counter(s['state'] for s in sessions if s['slot'] == 0)
wire_lines = [line for line in (logs/'FINAL124_JOIN173-ahl.log').read_text().splitlines()
              if re.search(r'c->ah not host .*type (17|21) len|teleport request|world state: group|10054', line)]
summary = {
    'at': datetime.now(timezone.utc).isoformat(), 'samples': len(rows),
    'supersedes': 'JOIN173-session-acceptance.json: loaded_words is a bitset, not a list of slice indices; the original count0 is invalid.',
    'sample_span': [rows[0]['utc'], rows[-1]['utc']], 'session_states': dict(state_counts),
    'context_errors': sum('context_error' in s for s in sessions),
    'unstable_pointers': sum(not r['pointers_unchanged'] for r in rows),
    'unstable_contexts': sum(not s['context']['unchanged_handle_state'] or not s['context']['lookup']['unchanged_during_lookup'] for s in sessions if 'context' in s),
    'loaded_inactive112': {'count': len(loaded), 'span': [loaded[0]['utc'], loaded[-1]['utc']] if loaded else None},
    'pending2_span': [pending[0]['utc'], pending[-1]['utc']] if pending else None,
    'first_pending': {'utc': pending[0]['utc'], 'clock': pending[0]['activity_clock'], 'sessions': pending[0]['sessions']} if pending else None,
    'last_before_first_reset': next((r for r in reversed(rows) if r['utc'] < '2026-10-07T19:03:57'), None),
    'idle112': sum(r['active_loading'][0] == 112 and r['flags_88_8b'][1] == 0 for r in rows),
    'watch_counts': dict(Counter(h['pc'] for h in hits)),
    'return_helpers': [h for h in hits if h['pc'] in ('0xd98a54', '0xd98a58')],
    'gate_inputs': [h for h in hits if h['pc'] in ('0x47b7ec', '0x8c6d4c')],
    'wire_lines': wire_lines,
    'validation_ids': len(re.findall(r'VUID-', native)),
    'device_loss': len(re.findall(r'VK_ERROR_DEVICE_LOST', native)),
    'fatal_lines': len(re.findall(r'^.*[·]F ', native, flags=re.M)),
    'scope': 'Non-atomic owned reader and actual filtered entry/epilogue hits; outer return1 does not prove direct queue consumption or wire delivery.'
}
out = rt/'RESEARCH/JOIN173-session-acceptance-corrected.json'
with out.open('x', encoding='utf-8') as stream:
    json.dump(summary, stream, indent=2)
print(json.dumps({k:v for k,v in summary.items() if k not in ('last_before_first_reset','gate_inputs','return_helpers','first_pending')}, indent=2))

sys.path.insert(0, r'F:\d1\d1xenia\scripts')
from d1_eboot import Eboot
elf = Eboot(rt/'RESEARCH/ps3_updates/01.24/USRDIR/EBOOT.elf')
for table in (0x13ed518, 0x13ea298):
    methods = {}
    for offset in range(0, 0x30, 4):
        descriptor = int.from_bytes(elf.read(table+offset, 4), 'big')
        raw = elf.read(descriptor, 8)
        if raw:
            entry, toc = struct.unpack('>II', raw)
            methods[hex(offset)] = {'descriptor': hex(descriptor), 'entry': hex(entry), 'toc': hex(toc)}
    print(json.dumps({'table':hex(table), 'methods':methods}))
print('queue count/head instruction references:')
for line in elf.disasm(0x479000, (0x47bdd4-0x479000)//4):
    if re.search(r'\b(lwz|stw) .*0x(118|11c|120)\(', line):
        pc = int(line[:8], 16)
        fn, end = elf.function_of(pc)
        print(hex(fn), hex(end), line)
for descriptor in elf.data_refs(0x47b7ec):
    for slot in elf.data_refs(descriptor):
        table = slot - 0x20
        print('candidate queue vtable', hex(table))
        for offset in range(0, 0x34, 4):
            desc = int.from_bytes(elf.read(table+offset, 4), 'big')
            raw = elf.read(desc, 8)
            if raw:
                entry, toc = struct.unpack('>II', raw)
                print(hex(offset), hex(desc), hex(entry), hex(toc))
