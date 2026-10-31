from pathlib import Path
from datetime import datetime, timezone
import hashlib
import json
import subprocess
import shutil
import sys

sys.path.insert(0, r'F:\d1\d1xenia')
sys.path.insert(0, r'F:\d1\d1xenia\tools\content_capture')
from tools.content_capture.capture import validate_video

rt = Path(r'F:\d1\<pc>-import\d1xenia\local')
buffer_source = rt / 'content-capture/buffer/3958f8a7799b4a13808f094a41fb0da4.mp4'
source = rt / 'RESEARCH/JOIN159-preserved-native-20261007T074231.mp4'
expected_hash = 'e94c1315bce52616839ff559ac7b9c508a0a822f9be994112a79cc6fd68096c4'
assert hashlib.sha256(buffer_source.read_bytes()).hexdigest() == expected_hash
assert not source.exists()
shutil.copy2(buffer_source, source)
assert hashlib.sha256(source.read_bytes()).hexdigest() == expected_hash
ffmpeg = rt / 'DEVLOGS/tools/imageio_ffmpeg/binaries/ffmpeg-win-x86_64-v7.1.exe'
output = rt / 'RESEARCH/JOIN159-video-inspection-final'
output.mkdir(exist_ok=False)
media = validate_video(source, str(ffmpeg))
result = subprocess.run([str(ffmpeg), '-v', 'error', '-nostdin', '-i', str(source),
                         '-vf', 'fps=1/4', '-frames:v', '6', str(output / 'frame-%02d.png')],
                        capture_output=True, text=True)
if result.returncode:
    raise RuntimeError(result.stderr)
receipt = {'at': datetime.now(timezone.utc).isoformat(),
           'run': 'FINAL124_JOIN159', 'build_sha256': '55efc9072019124be3e880ca8c881339d057390c47d3bf77c799ccb6de920259',
           'source': str(source), 'source_sha256': hashlib.sha256(source.read_bytes()).hexdigest(),
           'provenance': 'Byte-identical finalized native recording preserved from its verified rolling-buffer copy. The earlier worker removed the recorder-path original; no repair, cut or re-encode applied.',
           'recorded_at': '2026-10-07T07:42:31.271953+00:00',
           'media': media, 'inspection_frames': sorted(p.name for p in output.glob('frame-*.png')),
           'scope': 'Private video decoding and sampled-frame QA; not an edited production clip or continuous visual acceptance',
           'privacy': 'Account label and native overlay paths are present; private diagnostic evidence only'}
(output / 'receipt.json').write_text(json.dumps(receipt, indent=2), encoding='utf-8')
print(json.dumps(receipt, indent=2))
