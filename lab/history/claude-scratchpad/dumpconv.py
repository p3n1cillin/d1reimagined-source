import sys, os, re, struct
import numpy as np
from PIL import Image
d = sys.argv[1]; out = os.path.join(d, 'png'); os.makedirs(out, exist_ok=True)
pat = re.compile(r'(\d+)_([0-9a-f]+)_([0-9a-f]+)_(\d+)x(\d+)_(\w+)_(\d+)\.raw')
rows = []
for f in sorted(os.listdir(d)):
    m = pat.fullmatch(f)
    if not m: continue
    seq, addr, sh, w, h, fmt, bpp = m.groups(); w, h, bpp = int(w), int(h), int(bpp); fmt = 'e' + fmt
    raw = open(os.path.join(d, f), 'rb').read()
    if len(raw) < w*h*bpp: continue
    a = None
    if fmt in ('eR8G8B8A8Unorm', 'eR8G8B8A8Srgb', 'eB8G8R8A8Unorm', 'eB8G8R8A8Srgb'):
        a = np.frombuffer(raw, np.uint8).reshape(h, w, 4).astype(np.float32) / 255
        if fmt.startswith('eB'): a = a[..., [2, 1, 0, 3]]
    elif fmt == 'eR16G16B16A16Sfloat':
        a = np.frombuffer(raw, np.float16).reshape(h, w, 4).astype(np.float32)
    elif fmt == 'eR32G32B32A32Sfloat':
        a = np.frombuffer(raw, np.float32).reshape(h, w, 4)
    elif fmt == 'eR16G16Unorm':
        a = np.frombuffer(raw, np.uint16).reshape(h, w, 2).astype(np.float32) / 65535
    elif fmt == 'eR16G16Sfloat':
        a = np.frombuffer(raw, np.float16).reshape(h, w, 2).astype(np.float32)
    elif fmt in ('eR32Sfloat', 'eD32Sfloat', 'eD32SfloatS8Uint'):
        a = np.frombuffer(raw[:w*h*4], np.float32).reshape(h, w, 1)
    elif fmt == 'eR8Unorm' or fmt == 'eR8Uint':
        a = np.frombuffer(raw, np.uint8).reshape(h, w, 1).astype(np.float32) / (255 if fmt == 'eR8Unorm' else 1)
    elif fmt == 'eR16Sfloat':
        a = np.frombuffer(raw, np.float16).reshape(h, w, 1).astype(np.float32)
    elif fmt == 'eR8G8Unorm':
        a = np.frombuffer(raw, np.uint8).reshape(h, w, 2).astype(np.float32) / 255
    elif fmt == 'eB10G11R11UfloatPack32':
        v = np.frombuffer(raw, np.uint32).reshape(h, w)
        def uf(bits, mb):
            e = (bits >> mb) & 31; m = bits & ((1 << mb) - 1)
            return np.where(e == 0, m / (1 << mb) * 2.0**-14, (1 + m / (1 << mb)) * 2.0**(e.astype(np.float64) - 15))
        a = np.stack([uf(v & 0x7ff, 6), uf((v >> 11) & 0x7ff, 6), uf((v >> 22) & 0x3ff, 5)], -1).astype(np.float32)
    if a is None:
        rows.append((seq, addr, sh, w, h, fmt, 'unsupported')); continue
    a = np.nan_to_num(a, nan=0, posinf=65504, neginf=-65504)
    stats = []
    for c in range(a.shape[2]):
        ch = a[..., c]; stats.append(f'{ch.min():.3g}/{ch.mean():.3g}/{ch.max():.3g}')
    rows.append((seq, addr, sh, w, h, fmt, ' '.join(stats)))
    # normalise each channel to its 99.5th percentile for viewing
    img = a[..., :3] if a.shape[2] >= 3 else np.repeat(a[..., :1], 3, -1) if a.shape[2] == 1 else np.concatenate([a, np.zeros_like(a[..., :1])], -1)
    hi = np.percentile(img, 99.5); lo = min(0, img.min())
    if hi <= lo: hi = lo + 1
    v = np.clip((img - lo) / (hi - lo), 0, 1) ** (1 / 2.2 if fmt.endswith('Sfloat') else 1)
    Image.fromarray((v * 255).astype(np.uint8)).save(os.path.join(out, f[:-4] + '.png'))
with open(os.path.join(d, 'index.txt'), 'w') as o:
    for r in rows: o.write(' '.join(map(str, r)) + '\n')
print(len(rows))
