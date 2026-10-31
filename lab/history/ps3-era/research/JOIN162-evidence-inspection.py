from pathlib import Path
from datetime import datetime, timezone
import hashlib
import json
import subprocess
import sys

sys.path.insert(0, r'F:\d1\d1xenia')
sys.path.insert(0, r'F:\d1\d1xenia\tools\content_capture')
from tools.content_capture.capture import validate_video

rt = Path(r'F:\d1\<pc>-import\d1xenia\local')
segments = rt / 'content-capture/native-segments'
output = rt / 'RESEARCH/JOIN162-evidence-inspection'
output.mkdir(exist_ok=True)
assert not (output / 'receipt.json').exists()
ffmpeg = str(rt / 'DEVLOGS/tools/imageio_ffmpeg/binaries/ffmpeg-win-x86_64-v7.1.exe')
retained = []
latest = []
unindexed = []
for path in segments.glob('*.json'):
    info = json.loads(path.read_text(encoding='utf-8-sig'))
    if info.get('run') != 'FINAL124_JOIN162':
        continue
    source = Path(info['file'])
    proof = path.with_suffix('.indexed')
    if not proof.exists():
        failed = path.with_suffix('.failed')
        unindexed.append({'segment': path.stem, 'source': str(source),
                          'recorded_at': info['recorded_at'], 'source_exists': source.exists(),
                          'failure': json.loads(failed.read_text(encoding='utf-8-sig')) if failed.exists() else None})
        continue
    indexed = json.loads(proof.read_text(encoding='utf-8-sig'))
    actual_hash = hashlib.sha256(source.read_bytes()).hexdigest()
    expected_hash = indexed.get('source_sha256')
    retained.append({'segment': path.stem, 'source': str(source),
                     'recorded_at': info['recorded_at'], 'bytes': source.stat().st_size,
                     'original_retained': indexed.get('original_retained'),
                     'sha256': actual_hash, 'hash_matches_index': actual_hash == expected_hash})
    latest.append((info['recorded_at'], path.stem, source))
samples = []
for recorded_at, segment, source in sorted(latest)[-2:]:
    media = validate_video(source, ffmpeg)
    args = [ffmpeg, '-v', 'error', '-nostdin', '-i', str(source), '-vf', 'fps=1/5',
            '-frames:v', '7', str(output / (segment + '-%02d.png'))]
    proc = subprocess.run(args, capture_output=True, text=True)
    if proc.returncode:
        raise RuntimeError(proc.stderr)
    samples.append({'segment': segment, 'source': str(source), 'recorded_at': recorded_at,
                    'media': media, 'full_decode': 'PASS',
                    'frames': sorted(p.name for p in output.glob(segment + '-*.png'))})
result = {'at': datetime.now(timezone.utc).isoformat(), 'run': 'FINAL124_JOIN162',
          'retention_checks': sorted(retained, key=lambda x: x['recorded_at']),
          'unindexed': unindexed,
          'video_inspection': samples,
          'scope': 'Untouched originals retained; full decoding and private sampled-frame QA of final two segments. No repair, cut, re-encode or publication.'}
(output / 'receipt.json').write_text(json.dumps(result, indent=2), encoding='utf-8')
print(json.dumps({'retained_count': len(retained), 'unindexed': unindexed, 'all_hashes_match': all(x['hash_matches_index'] for x in retained), 'samples': samples}, indent=2))

