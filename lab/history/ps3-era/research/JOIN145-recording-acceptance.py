import json
from pathlib import Path
import re
import struct
import subprocess
import sys

sys.path.insert(0, r'F:\d1\d1xenia\tools\content_capture')
from capture import Capture, atomic_json, closed_source_hashes, copy_closed, digest

root = Path(r'F:\d1\<pc>-import\d1xenia\local\content-capture')
research = root.parent / 'RESEARCH'
capture = Capture(root / 'config.json')
receipt = root / 'native-segments' / '16c76f59ef5f472f968c4964dad99859.json'
native = json.loads(receipt.read_text(encoding='utf-8-sig'))
assert native['run'] == 'FINAL124_JOIN145' and native['ownership']['id'] == 11008
original = Path(native['file'])
indexed = json.loads(receipt.with_suffix('.indexed').read_text())
proof = closed_source_hashes(original, indexed['retained_bytes'])
assert proof['source_sha256'] == 'ee2ed70ac8074e48af4d0d65f9cca75e854bd253468eb4afce5a62465d2c8d6d'
assert proof['retained_sha256'] == indexed['retained_sha256']
entries = [json.loads(p.read_text()) for p in capture.buffer.glob('*.json')]
entry = next(e for e in entries if e.get('run') == native['run']
             and e.get('source_recovery', {}).get('retained_sha256') == proof['retained_sha256'])
recovered = capture.buffer / entry['file']
assert digest(recovered) == proof['retained_sha256']
result = subprocess.run([capture.ffmpeg, '-hide_banner', '-nostdin', '-v', 'error', '-xerror',
                         '-threads', '2', '-i', str(recovered), '-map', '0:v:0', '-map', '0:a:0',
                         '-progress', 'pipe:1', '-f', 'null', '-'],
                        capture_output=True, text=True, errors='replace', timeout=120)
assert result.returncode == 0 and not result.stderr.strip(), result.stderr
frames = re.findall(r'^frame=(\d+)$', result.stdout, flags=re.M)
assert frames and int(frames[-1]) > 0
boxes = []
with original.open('rb') as stream:
    while stream.tell() + 8 <= proof['original_bytes']:
        offset = stream.tell()
        size, kind = struct.unpack('>I4s', stream.read(8))
        if size == 1:
            extra = stream.read(8)
            if len(extra) != 8:
                break
            size = struct.unpack('>Q', extra)[0]
        if size < 8 or offset + size > proof['original_bytes']:
            break
        boxes.append(kind.decode('ascii', errors='replace'))
        stream.seek(offset + size)
assert 'moov' in boxes and 'moof' in boxes and 'mfra' not in boxes
pending_path = root / 'pending-observations.json'
pending = json.loads(pending_path.read_text())
event = next(e for e in pending if e['event_id'] == 'e10992dda6414c7c90e3509c40f72888')
assert event['run'] == native['run'] and event['feature'] == 'native-recording-recovery'
old_at = event['at']
backup = research / 'JOIN145-pending-before-timestamp-correction.json'
if not backup.exists():
    copy_closed(pending_path, backup)
event['detected_at'] = old_at
event['at'] = entry['ended_at']
atomic_json(pending_path, pending)
acceptance = {'run': native['run'], 'controlled_native_interruption': True,
              'strict_decode_exit': result.returncode, 'strict_decode_error': result.stderr,
              'decoded_frames': int(frames[-1]), 'contains_audio': entry['contains_audio'],
              'decoded_duration_seconds': entry['duration_seconds'],
              'original_unchanged': digest(original) == proof['source_sha256'],
              'finished_trailer_present': 'mfra' in boxes, 'source_recovery': entry['source_recovery'],
              'prior_import_time': old_at, 'recording_observed_at': event['at'],
              'correction_reason': 'Use the recovered recording end, rather than delayed import time, for footage selection.'}
atomic_json(research / 'JOIN145-recording-acceptance.json', acceptance)
print(json.dumps(acceptance, indent=2))
