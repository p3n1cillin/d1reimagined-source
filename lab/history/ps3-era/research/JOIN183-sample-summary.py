import json,struct
from pathlib import Path
root=Path(__file__).parent
for line in (root/'JOIN183-performance-samples.jsonl').read_text(encoding='utf-8').splitlines():
    sample=json.loads(line)
    rows=[]
    for r in sample['records']:
        record=bytes.fromhex(r['record'])
        if record[8:12] not in (bytes.fromhex('80cb8d5b'),bytes.fromhex('80cb8d65')):continue
        data=bytes.fromhex(r.get('data_bytes',''))
        if len(data)<0xd0:continue
        rows.append({'tag':record[8:12].hex(),'data':r['data'],
                     'bytes20_23':data[0x20:0x24].hex(),
                     'words24_38':list(struct.unpack('>6I',data[0x24:0x3c])),
                     'tailb0_d0':data[0xb0:0xd0].hex()})
    print(sample['at'],json.dumps(rows))
