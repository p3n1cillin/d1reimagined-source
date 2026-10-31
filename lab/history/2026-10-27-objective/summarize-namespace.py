import json,struct
from pathlib import Path
root=Path(__file__).parent
for kind in ('2c','20'):
    rows=json.loads((root/f'namespace-{kind}.json').read_text())['rows']
    for row in rows:
        print(kind, row['dx'], 'source',row.get('source_tag'),row.get('source_offset'))
        if kind=='2c':
            print('  source',row.get('source_data'))
        elif row['dx'] in (14,17):
            for off,data in row.get('body',[]): print(' ',off,data)
