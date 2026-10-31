from pathlib import Path
from datetime import datetime, timezone
from collections import Counter
import json
import re
import struct
import sys

sys.path.insert(0, r'F:\d1\d1xenia\server')
import bap_server
import investment

snapshot = Path(sys.argv[1])
output = Path(sys.argv[2])
schema = investment.load_schema(Path(r'F:\d1\d1xenia-data\ps3_updates\types_01.24.json'))
hits = []
for line in snapshot.read_text(encoding='utf-8', errors='replace').splitlines():
    if 'PPU: D1P ' not in line:
        continue
    h = dict(re.findall(r'\b(pc|lr|r[0-9]+)=(0x[0-9a-f]+)', line))
    h['elapsed'] = re.search(r'\d+:\d\d:\d\d\.\d+', line)[0]
    h['dumps'] = dict(re.findall(r'\b(m[0-9]+(?:\+[0-9a-f]+)?)=([0-9a-f]+)', line))
    hits.append(h)
allocations = []
for h in hits:
    if h['pc'] != '0x2bb60' or h['lr'] != '0x479d14':
        continue
    data = bytes.fromhex(h['dumps']['m5'])
    assert len(data) == 47 and data[:13] == bytes.fromhex('01000000150000002200000000')
    h['decoded_slice'] = bap_server.client_slice_record(schema, data[13:])
    h['record_prefix'] = struct.unpack('>6I', bytes.fromhex(h['dumps']['m1+70']))
    allocations.append(h)
consumers = [h for h in hits if h['pc'] == '0x82db0']
encoders = [h for h in hits if h['pc'] == '0x5b2610']
returns = [h for h in hits if h['pc'] == '0xd98a68']
correlations = []
for a in allocations:
    prefix = a['dumps']['m1+70']
    body_ptr = a['record_prefix'][2]
    def same_body(h):
        raw = bytes.fromhex(h['dumps'].get('m1+70', ''))
        return len(raw) == 8 and struct.unpack('>II', raw) == (47, body_ptr)
    correlated = {'allocation': a, 'consumer_matches': [h for h in consumers if h['dumps'].get('m26') == prefix],
                  'encoder_matches': [h for h in encoders if same_body(h)],
                  'return_matches': [h for h in returns if same_body(h)]}
    for h in correlated['return_matches']:
        header = bytes.fromhex(h['dumps']['m30'])
        h['decoded_header'] = dict(zip(('wire_type','id','host'), struct.unpack('>HIQ', header)))
    correlations.append(correlated)
result = {'at': datetime.now(timezone.utc).isoformat(), 'snapshot': str(snapshot),
          'scope': 'Bounded entry observations; equal initialized body pointer/length and record prefix correlate the queue with header encoding. Entry-only generic encoder has no native return hook; actual factory return is separately watched. No socket receipt or gameplay proof.',
          'watch_counts': dict(Counter(h['pc'] for h in hits)), 'correlations': correlations,
          'generic_encoder_entries': [h for h in hits if h['pc'] == '0x8c8220']}
with output.open('x', encoding='utf-8') as stream:
    json.dump(result, stream, indent=2)
print(json.dumps({'watch_counts': result['watch_counts'], 'decoded_slices': [h['decoded_slice'] for h in allocations],
                  'matches': [{k: len(c[k]) for k in ('consumer_matches','encoder_matches','return_matches')} for c in correlations]}, indent=2))
