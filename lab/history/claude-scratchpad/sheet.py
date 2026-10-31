import sys, os, numpy as np
from PIL import Image, ImageDraw
d = sys.argv[1]; out = sys.argv[2]; specs = sys.argv[3:]
tiles = []
for sp in specs:
    seq, ch = sp.split(':') if ':' in sp else (sp, 'rgb')
    f = [x for x in os.listdir(d) if x.startswith(seq + '_') and x.endswith('.raw')][0]
    parts = f[:-4].split('_'); w, h = map(int, parts[3].split('x')); fmt = parts[4]
    raw = open(os.path.join(d, f), 'rb').read()
    if fmt == 'R16G16B16A16Sfloat': a = np.frombuffer(raw, np.float16).reshape(h, w, 4).astype(np.float32)
    elif fmt.startswith('R8G8B8A8'): a = np.frombuffer(raw, np.uint8).reshape(h, w, 4) / 255.0
    elif fmt == 'R16G16Sfloat': a = np.frombuffer(raw, np.float16).reshape(h, w, 2).astype(np.float32)
    elif fmt in ('D32SfloatS8Uint', 'R32Sfloat'): a = np.frombuffer(raw[:w*h*4], np.float32).reshape(h, w, 1)
    elif fmt == 'R8Unorm': a = np.frombuffer(raw, np.uint8).reshape(h, w, 1) / 255.0
    a = np.nan_to_num(a)
    if ch == 'rgb': img = a[..., :3] if a.shape[2] >= 3 else np.repeat(a[..., :1], 3, -1)
    else: img = np.repeat(a[..., 'rgba'.index(ch)][..., None], 3, -1)
    hi = np.percentile(img, 99.5) or 1
    img = np.clip(img / hi, 0, 1)
    im = Image.fromarray((img * 255).astype(np.uint8)).resize((480, 270))
    ImageDraw.Draw(im).text((4, 4), f'{seq}:{ch} {fmt} max{hi:.3g}', fill=(255, 0, 255))
    tiles.append(im)
cols = 3; rows = (len(tiles) + cols - 1) // cols
sheet = Image.new('RGB', (480 * cols, 270 * rows))
for i, t in enumerate(tiles): sheet.paste(t, ((i % cols) * 480, (i // cols) * 270))
sheet.save(out)
