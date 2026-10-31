from pathlib import Path
from collections import Counter
from datetime import datetime, timezone
import json
import re
import subprocess

rt = Path(r'F:\d1\<pc>-import\d1xenia\local')
logs = rt/'SIGNON_LOCAL/logs'
cleanup = json.loads((logs/'JOIN174-cleanup-result.json').read_text(encoding='utf-8-sig'))
assert cleanup['all_owned_closed'] and not cleanup['errors']
rows = [json.loads(s) for s in (rt/'RESEARCH/JOIN174-session-timeline.jsonl').read_text().splitlines()]
reset = next(r['utc'] for r in rows if r['sessions'][0]['state'] != 6)
loaded = [r for r in rows if r['loaded_words'][3] & 65536 and r['active_loading'] == [-1, -1]]
first_loaded = [r for r in loaded if r['utc'] < reset]
native = (logs/'JOIN174-pre-stop-native.log').read_text(encoding='utf-8', errors='replace')
hits = []
for line in native.splitlines():
    if 'PPU: D1P ' in line:
        fields = dict(re.findall(r'\b(pc|lr|r[0-9]+)=(0x[0-9a-f]+)', line))
        fields['time'] = re.search(r'\d+:\d\d:\d\d\.\d+', line)[0]
        fields['dumps'] = dict(re.findall(r'\b(m[0-9]+(?:\+[0-9a-f]+)?)=([0-9a-f]+)', line))
        hits.append(fields)
allocations = [h for h in hits if h['pc'] == '0x2bb60' and h['dumps'].get('m5') == '01000000150000002200000000']
consumers = [h for h in hits if h['pc'] == '0x82db0']
matches = [(a,c) for a in allocations for c in consumers if a['dumps']['m1+70'] == c['dumps']['m26']]
pending = [r for r in rows if any(s.get('context', {}).get('pending_20') == 2 for s in r['sessions'])]
last_before = next(r for r in reversed(rows) if r['utc'] < reset)
wire = [s for s in (logs/'FINAL124_JOIN174-ahl.log').read_text().splitlines() if re.search(r'c->ah not host .*type 21|teleport request|10054', s)]
result = {'at':datetime.now(timezone.utc).isoformat(),'samples':len(rows),'span':[rows[0]['utc'],rows[-1]['utc']],
          'first_reset_sample':reset,'context_errors':sum('context_error' in s for r in rows for s in r['sessions']),
          'recheck_errors':sum(not r['pointers_unchanged'] for r in rows)+sum(not s['context']['unchanged_handle_state'] or not s['context']['lookup']['unchanged_during_lookup'] for r in rows for s in r['sessions'] if 'context' in s),
          'loaded_inactive_first':{'count':len(first_loaded),'span':[first_loaded[0]['utc'],first_loaded[-1]['utc']] if first_loaded else None},
          'loaded_inactive_all':len(loaded),'idle112':sum(r['active_loading'][0] == 112 and r['flags_88_8b'][1] == 0 for r in rows),
          'watch_counts':dict(Counter(h['pc'] for h in hits)), 'target_allocations':allocations,'target_consumers':consumers,
          'identical_record_prefix_pairs':len(matches),'first_pending':pending[0], 'last_before_first_reset':last_before,
          'wire':wire,'validation_ids':native.count('VUID-'),'device_loss':native.count('VK_ERROR_DEVICE_LOST'),
          'fatal_entries':len(re.findall(r'^.*[·]F ',native,re.M)),
          'input_scope':'An accidental dot-source invokes the helper default X150ms to matching windows; only this exact owned RPCS3 is live before/after. Subsequent attempted function invocation fails because the helper is a script. No 400ms or helper-identity validation claimed. Inspected before urgent/outdoor and after black/loading. No walking input.',
          'next':'Verify BAP factory3 encoder5b2610/8c8220 and forwarding queues; mapper17 selects codec8, not codec17. Actual allocation and consumer entry do not prove wire delivery.'}
(rt/'RESEARCH/JOIN174-final-acceptance.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
print(json.dumps({k:v for k,v in result.items() if k not in ('first_pending','last_before_first_reset','target_allocations','target_consumers','wire')},indent=2))
print(json.dumps({'matched_pairs': [{'allocation':a['time'],'consumer':c['time'],'prefix':c['dumps']['m26']} for a,c in matches]},indent=2))

out = rt/'RESEARCH/JOIN174-filter-reproduction'
out.mkdir(exist_ok=False)
subprocess.run(['git','init','--quiet',str(out)],check=True)
for relative, original in [('rpcs3/Emu/RSX/d1diag.h','d1diag.h'),('rpcs3/Emu/Cell/PPUThread.cpp','PPUThread.cpp')]:
    dest = out/relative
    dest.parent.mkdir(parents=True,exist_ok=True)
    dest.write_text((rt/'RESEARCH/JOIN172-source-before'/original).read_text(encoding='utf-8'),encoding='utf-8')
patch = Path(r'F:\d1\d1xenia\patches\rpcs3-ppu-watch-filters.patch')
subprocess.run(['git','-c','core.autocrlf=false','apply','--check','--whitespace=error-all',str(patch)],cwd=out,check=True)
subprocess.run(['git','-c','core.autocrlf=false','apply','--whitespace=error-all',str(patch)],cwd=out,check=True)
root = Path(r'F:\d1\<pc>-import\dev\rpcs3')
checks = {f:(out/f).read_text(encoding='utf-8') == (root/f).read_text(encoding='utf-8')
          for f in ('rpcs3/Emu/RSX/d1diag.h','rpcs3/Emu/Cell/PPUThread.cpp','rpcs3/Emu/RSX/Common/ppu_watch_filter.h')}
assert all(checks.values()), checks
receipt = {'at':datetime.now(timezone.utc).isoformat(),'git_apply_check_whitespace':'PASS','normalized_files_reproduced':checks}
(out/'receipt.json').write_text(json.dumps(receipt,indent=2),encoding='utf-8')
print(json.dumps(receipt,indent=2))
