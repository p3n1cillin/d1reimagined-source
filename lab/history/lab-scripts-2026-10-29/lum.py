import sys, statistics
from PIL import Image
for p in sys.argv[1:]:
    im=Image.open(p).convert('RGB'); w,h=im.size; im=im.crop((10,40,w-10,h-10))
    px=list(im.getdata()); lum=[0.299*r+0.587*g+0.114*b for r,g,b in px]
    print(p.split('/')[-2]+'/'+p.split('/')[-1], 'mean', round(statistics.mean(lum),1), 'black%', round(100*sum(v<8 for v in lum)/len(lum),1), 'white%', round(100*sum(v>245 for v in lum)/len(lum),1), 'RGB', [round(statistics.mean(c),1) for c in zip(*px)])
