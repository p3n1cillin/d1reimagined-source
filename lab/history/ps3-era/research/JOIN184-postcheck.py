import collections,datetime,hashlib,json,re,sys
from pathlib import Path
root=Path(__file__).parent
rt=root.parent
sys.path.insert(0,'F:/d1/d1xenia')
sys.path.insert(0,'F:/d1/d1xenia/tools/content_capture')
from tools.content_capture.capture import validate_video
out={'at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'run':'FINAL124_JOIN184'}
samples=[json.loads(line) for line in (root/'JOIN184-session-timeline.jsonl').read_text().splitlines() if line.strip()]
pairs=collections.Counter((s.get('native_current',[None])[0],s.get('native_target',[None])[0],s.get('flags_88_8b',[None,None])[1]) for s in samples)
out['session']={'samples':len(samples),'pairs':{str(k):v for k,v in pairs.items()},'session_states':dict(collections.Counter(str(s['sessions'][0]['state']) for s in samples if s.get('sessions'))),'pointer_recheck_failures':sum(s.get('pointers_unchanged') is False for s in samples)}
native=(rt/'SIGNON_LOCAL/logs/JOIN184-pre-stop-native.log').read_text(errors='replace')
out['native_log']={'vuid_lines':sum('VUID-' in line for line in native.splitlines()),'device_loss_lines':sum('VK_ERROR_DEVICE_LOST' in line for line in native.splitlines()),'fatal_lines':sum(bool(re.match(r'^F\s',line)) for line in native.splitlines()),'last_timestamp':re.findall(r'\d+:\d{2}:\d{2}\.\d{6}',native)[-1:]}
rows=[]
for path in (rt/'content-capture/native-segments').glob('*.json'):
    data=json.loads(path.read_text(encoding='utf-8-sig'))
    if 'JOIN184' not in json.dumps(data):continue
    rows.append({'sidecar':str(path),'data':data,'indexed':path.with_suffix('.indexed').exists()})
rows.sort(key=lambda row:row['data'].get('recorded_at',''))
out['media']={'segments':len(rows),'indexed':sum(r['indexed'] for r in rows),'inventory':rows}
(root/'JOIN184-postcheck.json').write_text(json.dumps(out,indent=2)+'\n')
print(json.dumps({k:v for k,v in out.items() if k!='media'}))
print('segments',len(rows),'indexed',sum(r['indexed'] for r in rows))
for row in rows[-2:]:print(json.dumps(row))

