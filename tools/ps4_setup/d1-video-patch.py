"""Write one 56578 client patch for a frame rate and a render size: d1-video-patch.py [--fps N] [--res WxH].

    d1-video-patch.py --fps 60 --res 2560x1440 --out lab-orbit-60hz-2560x1440.xml
    then: d1-lab-served.ps1 / d1-agr-run.ps1 ... -ClientPatch <that file>

Until the game's own settings menu can choose them (notes/ps4-56578-resolution-2026-11-03.md, "The
settings menu"), this is the one place to pick both. It runs d1-fps-patch.py N (the engine's time
control; 30 keeps the repo's lab patch, which has none) and then d1-res-patch.py W H on top, with the
surface pool and memory arena sized from what was measured on 3 November:
- the pool held 480 MB at 2560x1440 and 784 MB at 3840x2160, about 236 MB + 66 MB per megapixel
  (373 MB at 1080p, inside the game's 450);
- when that estimate passes 450 MB the pool gets 40% headroom (1152 MB at 4K, as tested) and memory
  arena 2 grows by the same amount plus 50 MB (3652 MB at 4K; 3600 tested).
Above 2048 wide or high the emulator fork's wide views are needed (automatic in the fork). The rate
can still change live (d1-frame-rate.py, D1_FPS); the render size only at start.
"""
import argparse
import math
import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
LAB = os.path.join(REPO, 'ps4', 'patches', 'CUSA00219-56578-lab-orbit.xml')
STOCK_POOL_MB, STOCK_ARENA_MB = 450, 2900


def pool_mb(w, h):
    """The surface pool for W x H, or None to keep the game's."""
    need = 236 + 66 * (w * h / 1e6)
    if need <= STOCK_POOL_MB:
        return None
    return math.ceil(need * 1.4 / 64) * 64


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('--fps', type=int, default=30, help='engine rate at start (30: the PS4 original)')
    ap.add_argument('--res', default='1920x1080', help='render size WxH, multiples of 4, up to 3840x2160')
    ap.add_argument('--out', required=True)
    a = ap.parse_args()
    w, h = (int(v) for v in a.res.lower().split('x'))
    with tempfile.TemporaryDirectory() as tmp:
        base = LAB
        if a.fps != 30:
            base = os.path.join(tmp, f'fps{a.fps}.xml')
            subprocess.run([sys.executable, os.path.join(HERE, 'd1-fps-patch.py'), str(a.fps), '--out', base],
                           check=True)
        if (w, h) == (1920, 1080):
            with open(base, 'rb') as f, open(a.out, 'wb') as o:
                o.write(f.read())
            print(f'{a.fps} fps, 1920x1080 -> {a.out}')
            return
        args = [sys.executable, os.path.join(HERE, 'd1-res-patch.py'), str(w), str(h), '--base', base, '--out', a.out]
        pool = pool_mb(w, h)
        if pool:
            args += ['--pool-mb', str(pool), '--arena-mb', str(STOCK_ARENA_MB + pool - STOCK_POOL_MB + 50)]
        subprocess.run(args, check=True)
        print(f'{a.fps} fps, {w}x{h}' + (f', pool {pool} MB, arena {STOCK_ARENA_MB + pool - STOCK_POOL_MB + 50} MB' if pool else ''))


if __name__ == '__main__':
    main()
