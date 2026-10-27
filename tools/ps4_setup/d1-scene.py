"""See the game without a display: dump one frame and save the HDR scene image as a PNG (56578).

    d1-scene.py OUT.png [--dump-dir F:/d1/tools/shadPS4/dump-live] [--wait 4]

Needs the emulator started with D1_DUMP_DIR=<dump dir> (the D1 shadPS4 build). Creating
<dump dir>.flag dumps every texture the shaders sample for about 0.2 s; the full-resolution
R16G16B16A16 image read by the tonemap pass (shader 27d596fb07cd3321) is the lit scene before
colour grading and the HUD, so it works while the monitor is off or the session is locked, when
window and screen captures come back black. The image is scaled to its 99.5th percentile with a
2.2 gamma: brightness is relative, not the game's exposure.
"""
import argparse, os, sys, time
import numpy as np
from PIL import Image

TONEMAP = '_27d596fb07cd3321_'


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('out')
    ap.add_argument('--dump-dir', default=r'F:\d1\tools\shadPS4\dump-live')
    ap.add_argument('--wait', type=float, default=4.0)
    ap.add_argument('--width', type=int, default=1280)
    a = ap.parse_args()
    d = a.dump_dir
    os.makedirs(d, exist_ok=True)
    for f in os.listdir(d):
        if f.endswith('.raw'):
            os.remove(os.path.join(d, f))
    open(d.rstrip('\\/') + '.flag', 'w').close()
    time.sleep(a.wait)
    try:
        os.remove(d.rstrip('\\/') + '.flag')   # the emulator may already have consumed it
    except FileNotFoundError:
        pass
    time.sleep(1)
    scene = [f for f in os.listdir(d) if TONEMAP in f and '_R16G16B16A16Sfloat_' in f]
    if not scene:
        sys.exit('no tonemap input in the dump: is the emulator running with D1_DUMP_DIR set?')
    f = max(scene, key=lambda n: os.path.getsize(os.path.join(d, n)))
    w, h = (int(v) for v in f.split('_')[3].split('x'))
    px = np.frombuffer(open(os.path.join(d, f), 'rb').read(), np.float16).reshape(h, w, 4)[..., :3]
    px = np.nan_to_num(px.astype(np.float32), nan=0, posinf=0, neginf=0).clip(0, None)
    scale = float(np.percentile(px, 99.5)) or 1.0
    img = (np.clip(px / scale, 0, 1) ** (1 / 2.2) * 255).astype(np.uint8)
    Image.fromarray(img).resize((a.width, a.width * h // w)).save(a.out)
    print(a.out, f, f'scale {scale:.3f}')


if __name__ == '__main__':
    main()
