"""One manual step: pad keys, then a screenshot (brightened copy for viewing). step.py NAME KEYS"""
import importlib.util, sys, os
from PIL import Image, ImageEnhance
HERE = os.path.dirname(os.path.abspath(__file__))
def L(n):
    s = importlib.util.spec_from_file_location(n, os.path.join(HERE, n + '.py')); m = importlib.util.module_from_spec(s); s.loader.exec_module(m); return m
f = L('fight'); hp = L('health'); pl = L('plates')
if sys.argv[2] != '-':
    f.pad(sys.argv[2])
import time; time.sleep(0.4)
out = os.path.join(HERE, 'man', sys.argv[1] + '.png'); os.makedirs(os.path.dirname(out), exist_ok=True)
img = f.grab(out)
print('health', hp.health(img), 'plates', pl.plates(img), 'radar', f.enemy_angle(img)[:2])
ImageEnhance.Brightness(Image.open(out)).enhance(2.2).resize((968, 560)).save(out.replace('.png', '-v.png'))
