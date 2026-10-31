"""Inspect the user-provided reference locally; no download or public media output."""
import subprocess
from pathlib import Path
from PIL import Image, ImageDraw
root=Path(__file__).parent/'reference'; root.mkdir(exist_ok=True)
video=Path(r'%USERPROFILE%\Downloads\destiny-gameplay-walkthrough-part-1-review-mission-1-ps4-ytmp4.savetube.vip.mp4')
subprocess.run(['ffmpeg','-hide_banner','-loglevel','error','-ss','660','-i',str(video),'-t','240','-vf','fps=1/15,scale=384:216','-y',str(root/'frame-%02d.jpg')],check=True)
files=sorted(root.glob('frame-*.jpg'))
canvas=Image.new('RGB',(4*384,4*240),'#151515'); draw=ImageDraw.Draw(canvas)
for i,p in enumerate(files[:16]):
    x,y=(i%4)*384,(i//4)*240
    canvas.paste(Image.open(p),(x,y)); seconds=660+15*i
    draw.text((x+6,y+219),f'{seconds//60:02d}:{seconds%60:02d}',fill='white')
out=root/'contact-sheet.jpg'; canvas.save(out)
print(out)
