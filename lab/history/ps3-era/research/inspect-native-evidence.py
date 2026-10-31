from pathlib import Path
from datetime import datetime, timezone
import hashlib
import json
import shutil
import subprocess
import sys

sys.path.insert(0, r'F:\d1\d1xenia')
sys.path.insert(0, r'F:\d1\d1xenia\tools\content_capture')
from tools.content_capture.capture import validate_video

name, folder = sys.argv[1:]
rt = Path(r'F:\d1\<pc>-import\d1xenia\local')
logs = rt / 'SIGNON_LOCAL/logs'
cleanup = json.loads((logs / f'{name}-cleanup-result.json').read_text(encoding='utf-8-sig'))
assert cleanup['fixture_completed'] and cleanup['all_owned_closed'] and not cleanup['errors']
output = rt / 'RESEARCH' / folder
output.mkdir(exist_ok=False)
ffmpeg = str(rt / 'DEVLOGS/tools/imageio_ffmpeg/binaries/ffmpeg-win-x86_64-v7.1.exe')
segments = rt / 'content-capture/native-segments'
retained, latest, unindexed = [], [], []
for path in segments.glob('*.json'):
    info = json.loads(path.read_text(encoding='utf-8-sig'))
    if info.get('run') != 'FINAL124_' + name:
        continue
    source = Path(info['file'])
    proof = path.with_suffix('.indexed')
    if not proof.exists():
        failed = path.with_suffix('.failed')
        unindexed.append({'segment': path.stem, 'source': str(source), 'recorded_at': info['recorded_at'],
                          'original_present': source.exists(),
                          'failure': json.loads(failed.read_text(encoding='utf-8-sig')) if failed.exists() else None})
        continue
    indexed = json.loads(proof.read_text(encoding='utf-8-sig'))
    actual = hashlib.sha256(source.read_bytes()).hexdigest()
    retained.append({'segment': path.stem, 'source': str(source), 'recorded_at': info['recorded_at'],
                     'bytes': source.stat().st_size, 'sha256': actual,
                     'hash_matches_index': actual == indexed['source_sha256'],
                     'original_retained': indexed.get('original_retained'),
                     'source_recovery': indexed.get('source_recovery')})
    latest.append((info['recorded_at'], path.stem, source, indexed))
samples = []
for at, segment, source, indexed in sorted(latest)[-2:]:
    sample = {'segment': segment, 'source': str(source), 'recorded_at': at}
    try:
        sample['original_media'] = validate_video(source, ffmpeg)
        sample['original_full_decode'] = 'PASS'
        usable = source
    except ValueError as error:
        sample['original_full_decode'] = 'FAIL'
        sample['original_error'] = str(error)
        raw = subprocess.run([ffmpeg, '-hide_banner', '-nostdin', '-xerror', '-threads', '2',
                              '-i', str(source), '-map', '0:v:0', '-map', '0:a?', '-f', 'null', '-'],
                             capture_output=True, text=True, errors='replace', timeout=600)
        (output / (segment + '-original-decode.log')).write_text(raw.stderr, encoding='utf-8')
        sample['original_decode_exit'] = raw.returncode
        usable = None
        recovery = indexed.get('source_recovery')
        # Preserve an already generated derivative byte-for-byte before preview expiry.
        # Do not reconstruct, truncate, repair or edit any original here.
        if recovery:
            for entry in (rt / 'content-capture/buffer').glob('*.json'):
                buf = json.loads(entry.read_text(encoding='utf-8-sig'))
                rec = buf.get('source_recovery') or {}
                if rec.get('retained_sha256') != indexed['retained_sha256']:
                    continue
                prior = entry.parent / buf['file']
                if not prior.exists():
                    continue
                assert hashlib.sha256(prior.read_bytes()).hexdigest() == indexed['retained_sha256']
                usable = output / (segment + '-existing-recovered-derivative.mp4')
                shutil.copy2(prior, usable)
                assert hashlib.sha256(usable.read_bytes()).hexdigest() == indexed['retained_sha256']
                sample['existing_derivative'] = {'file': str(usable), 'sha256': indexed['retained_sha256'],
                                                  'recovery': recovery, 'original_unchanged': True}
                sample['derivative_media'] = validate_video(usable, ffmpeg)
                sample['derivative_full_decode'] = 'PASS'
                break
    if usable:
        proc = subprocess.run([ffmpeg, '-v', 'error', '-nostdin', '-i', str(usable), '-vf', 'fps=1/5',
                               '-frames:v', '7', str(output / (segment + '-%02d.png'))], capture_output=True, text=True)
        assert proc.returncode == 0, proc.stderr
        sample['frames'] = sorted(p.name for p in output.glob(segment + '-*.png'))
        sample['sampled_file'] = str(usable)
    else:
        sample['frames'] = []
        sample['derivative_full_decode'] = 'NOT RUN; existing derivative unavailable'
    samples.append(sample)
result = {'at': datetime.now(timezone.utc).isoformat(), 'run': 'FINAL124_' + name,
          'retention_checks': sorted(retained, key=lambda x: x['recorded_at']), 'unindexed': unindexed,
          'video_inspection': samples,
          'scope': 'Untouched originals and truthful decode outcomes; existing recovered derivative copied unchanged when available. Private sampled QA only.'}
(output / 'receipt.json').write_text(json.dumps(result, indent=2), encoding='utf-8')
print(json.dumps({'retained': len(retained), 'unindexed': len(unindexed),
                  'all_original_hashes_match': all(x['hash_matches_index'] for x in retained), 'samples': samples}, indent=2))
