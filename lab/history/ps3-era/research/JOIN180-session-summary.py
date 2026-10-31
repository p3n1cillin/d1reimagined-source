import collections, datetime, json, re
from pathlib import Path
root=Path(__file__).parent
runtime=root.parent
rows=[json.loads(line) for line in (root/'JOIN180-session-timeline.jsonl').read_text().splitlines() if line.strip()]
counts=collections.Counter((tuple(r.get('native_current',[])),tuple(r.get('native_target',[])),r['flags_88_8b'][1]) for r in rows)
print('keys',list(rows[-1]))
print('states',counts)
result={'at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'samples':len(rows),
 'states':[{'current':c,'target':t,'engine_phase':p,'samples':n} for (c,t,p),n in counts.items()],
 'follow_phases':dict(collections.Counter(str(r.get('follow_player_phase_1d2')) for r in rows)),
 'session_states':dict(collections.Counter(str(r['sessions'][0]['state']) for r in rows)),
 'read_failures':[r for r in rows if r.get('error') or r.get('context_recheck_failed') or r.get('pointer_recheck_failed') or not r.get('pointers_unchanged')]}
log=(runtime/'SIGNON_LOCAL/logs/JOIN180-pre-stop-native.log').read_text(errors='replace')
result['validation_or_fatal_lines']=[s for s in log.splitlines() if re.search(r'VUID-|VK_ERROR_DEVICE_LOST|(?:^|\s)F\s',s)]
result['validation_enable_lines']=[s for s in log.splitlines() if 'validation' in s.lower() and ('enabled' in s.lower() or 'layer' in s.lower())][:6]
result['last_native_log_line']=log.splitlines()[-1] if log else None
(root/'JOIN180-session-summary.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items() if k!='states'},indent=2))
