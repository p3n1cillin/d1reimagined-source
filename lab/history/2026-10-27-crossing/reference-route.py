import subprocess
from pathlib import Path
from PIL import Image,ImageDraw
root=Path(__file__).parent/'reference-route'; root.mkdir(exist_ok=True)
subprocess.run(['ffmpeg','-hide_banner','-loglevel','error','-ss','722','-i',r'%USERPROFILE%\Downloads\destiny-gameplay-walkthrough-part-1-review-mission-1-ps4-ytmp4.savetube.vip.mp4','-t','48','-vf','fps=1/3,scale=480:270','-y',str(root/'frame-%02d.jpg')],check=True)
im=Image.new('RGB',(1920,1176),'#111111'); d=ImageDraw.Draw(im)
for i,p in enumerate(sorted(root.glob('frame-*.jpg'))[:16]):
    x,y=i%4*480,i//4*294; im.paste(Image.open(p),(x,y)); t=722+3*i; d.text((x+5,y+272),f'{t//60}:{t%60:02}',fill='white')
out=root/'sheet.jpg'; im.save(out); print(out)
