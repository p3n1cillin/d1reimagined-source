# scene.py DUMP_DIR OUT.png: the HDR scene image the tonemap pass samples, tonemapped for viewing
import os, re, sys, numpy as np
from PIL import Image
d, out = sys.argv[1], sys.argv[2]
cands = [f for f in os.listdir(d) if '_27d596fb07cd3321_' in f and '_1920x1080_R16G16B16A16Sfloat_' in f]
if not cands:
    cands = [f for f in os.listdir(d) if '_1920x1080_R16G16B16A16Sfloat_' in f]
f = sorted(cands)[-1]
a = np.frombuffer(open(os.path.join(d, f), 'rb').read(), np.float16).reshape(1080, 1920, 4).astype(np.float32)[..., :3]
a = np.nan_to_num(a, nan=0, posinf=0, neginf=0).clip(0, None)
s = np.percentile(a, 99.5) or 1
img = (np.clip(a / s, 0, 1) ** (1 / 2.2) * 255).astype(np.uint8)
Image.fromarray(img).resize((1280, 720)).save(out); print(f, 'scale', s)
