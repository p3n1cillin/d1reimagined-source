import datetime,json,subprocess,sys
from pathlib import Path
from PIL import Image,ImageDraw
sys.path.insert(0,'F:/d1/d1xenia')
sys.path.insert(0,'F:/d1/d1xenia/tools/content_capture')
from tools.content_capture.capture import validate_video
root=Path(__file__).parent;rt=root.parent
ffmpeg=rt/'DEVLOGS/tools/imageio_ffmpeg/binaries/ffmpeg-win-x86_64-v7.1.exe'
out=root/'JOIN185-fp-inspection';out.mkdir(exist_ok=True)
paths=[rt/'RPCS3_FINAL124_SMOKE001/recordings/BLES01857'/f'BLES01857_recording_2026_10_08_{name}.mp4' for name in ('09_50_53','09_51_23')]
reports=[];frames=[]
for index,path in enumerate(paths):
    reports.append({'source':str(path),'verification':validate_video(path,str(ffmpeg))})
    for sec in (4,10,16,22,28):
        image_path=out/f'{index}-{sec}.png'
        result=subprocess.run([str(ffmpeg),'-hide_banner','-loglevel','error','-ss',str(sec),'-i',str(path),'-frames:v','1',str(image_path)],capture_output=True,text=True)
        if result.returncode:raise RuntimeError(result.stderr)
        frames.append((image_path,f'{path.stem} +{sec}s'))
sheet=Image.new('RGB',(1280,5*384),'#101010');draw=ImageDraw.Draw(sheet)
for index,(path,label) in enumerate(frames):
    x=(index%2)*640;y=(index//2)*384
    sheet.paste(Image.open(path).resize((640,360)),(x,y));draw.text((x+8,y+363),label,fill='white')
sheet.save(out/'sheet.jpg')
(out/'receipt.json').write_text(json.dumps({'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'reports':reports,'frames':[str(path) for path,label in frames],'scope':'Private engineering frame sampling; originals untouched. Not clean publication footage.'},indent=2)+'\n',encoding='utf-8')
print(out/'sheet.jpg')
