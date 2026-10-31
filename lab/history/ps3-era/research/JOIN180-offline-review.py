"""Private bounded static review; leaves package and media inputs untouched."""
import json, sys
from pathlib import Path
repo = Path('F:/d1/d1xenia')
data = Path('F:/d1/d1xenia-data')
root = Path(__file__).parent
types = json.loads((data / 'ps3_updates/types_01.24.json').read_text())
seen = set()
def show(key, depth=0):
    if key in seen or depth > 2: return
    seen.add(key)
    value = types.get(key)
    print(key, json.dumps(value))
    if value:
        for field in value.get('fields', []):
            for name in ('type', 'nested', 'struct'):
                child = field.get(name)
                if isinstance(child, int) and child > 0x80000000:
                    show(f'0x{child:08x}', depth+1)
for key in ('0x80802a1b','0x8080293c','0x8080254d','0x80804c23','0x80804bda'):
    show(key)
catalog = json.loads((root / 'ps3_updates/mission-catalog-0124/catalog.json').read_text())
print('matching packages', json.dumps([p for p in catalog['packages'] if p['id'] == '0x025c']))
