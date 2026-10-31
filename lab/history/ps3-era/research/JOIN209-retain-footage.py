"""Retain unchanged native source segments for the next factual report."""
import concurrent.futures, hashlib, json, shutil, subprocess
from pathlib import Path
rt=Path(__file__).parent.parent
source=rt/'RPCS3_FINAL124_SMOKE001/recordings/BLES01857'
destination=rt/'PRIVATE_ENGINEERING/media/JOIN209'
destination.mkdir(parents=True,exist_ok=False)
ffmpeg=rt/'DEVLOGS/tools/imageio_ffmpeg/binaries/ffmpeg-win-x86_64-v7.1.exe'
names=['BLES01857_recording_2026_10_08_19_04_18.mp4',
       'BLES01857_recording_2026_10_08_19_04_48.mp4',
       'BLES01857_recording_2026_10_08_19_07_21.mp4']
def retain(name):
    src=source/name;dst=destination/name
    digest=lambda p:hashlib.file_digest(p.open('rb'),'sha256').hexdigest()
    before=digest(src);shutil.copy2(src,dst)
    if digest(src)!=before or digest(dst)!=before: raise RuntimeError('source changed or copy hash mismatch')
    result=subprocess.run([str(ffmpeg),'-v','error','-xerror','-i',str(dst),'-f','null','NUL'],capture_output=True,text=True)
    if result.returncode or result.stderr.strip(): raise RuntimeError(f'full decode failed: {name}: {result.stderr}')
    return {'source':str(src),'retained':str(dst),'bytes':dst.stat().st_size,'sha256':before,'copy_verified':True,'full_decode':'PASS','actual_filename_timezone':'Europe/London','audio_content_review':'NOT REVIEWED'}
with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
    clips=list(pool.map(retain,names))
receipt={'scope':'Unchanged full native source segments; no montage, trims or publishing. Allocation is verified by private native traces, not by these camera views. Pickup and lighting remain unaccepted.','reporting':'Late October 8 engineering evidence for next report; existing ready packages and today\u2019s report remain unchanged. No NAS delivery claimed for these copies.','clips':clips}
(destination/'retention.json').write_text(json.dumps(receipt,indent=2)+'\n')
print(f'PASS: retained and hash-verified {len(clips)} unchanged native segments; all video/audio fully decode.')
