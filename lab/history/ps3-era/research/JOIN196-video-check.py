from pathlib import Path
import sys,json,hashlib,datetime,subprocess
sys.path.insert(0,'F:/d1/d1xenia/tools/content_capture')
from capture import validate_video
rt=Path(__file__).parent.parent
ffmpeg=rt/'DEVLOGS/tools/imageio_ffmpeg/binaries/ffmpeg-win-x86_64-v7.1.exe'
recordings=rt/'RPCS3_FINAL124_SMOKE001/recordings/BLES01857'
names=('BLES01857_recording_2026_10_08_13_31_50.mp4',
       'BLES01857_recording_2026_10_08_13_32_51.mp4',
       'BLES01857_recording_2026_10_08_13_55_18.mp4')
rows=[]
for name in names:
    path=recordings/name
    digest=hashlib.sha256(path.read_bytes()).hexdigest()
    validation=validate_video(path,str(ffmpeg))
    rows.append({'file':str(path),'sha256':digest,'bytes':path.stat().st_size,**validation})
out={'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'clips':rows,'scope':'Full decoding of these three originals only; visual acceptance requires inspecting the actual frames. No final video editing or publication.'}
with (rt/'RESEARCH/JOIN196-video-check.json').open('x',encoding='utf-8') as stream:json.dump(out,stream,indent=2)
print(json.dumps(out))
