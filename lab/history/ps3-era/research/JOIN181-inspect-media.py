import json,sys,hashlib,datetime
from pathlib import Path
root=Path(__file__).parent;rt=root.parent
sys.path.insert(0,'F:/d1/d1xenia')
sys.path.insert(0,'F:/d1/d1xenia/tools/content_capture')
from tools.content_capture.capture import validate_video
rows=[]
for path in (rt/'content-capture/native-segments').glob('*.json'):
    data=json.loads(path.read_text(encoding='utf-8-sig'))
    if 'JOIN181' not in json.dumps(data):continue
    rows.append({'sidecar':str(path),'data':data,'indexed':path.with_suffix('.indexed').exists()})
rows.sort(key=lambda x:json.dumps(x['data'].get('recorded_at','')))
out={'at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'run':'FINAL124_JOIN181','segments':rows}
target=root/'JOIN181-media-inventory.json'
target.write_text(json.dumps(out,indent=2)+'\n')
print('segments',len(rows),'indexed',sum(x['indexed'] for x in rows))
for row in rows[-4:]:print(json.dumps(row))
