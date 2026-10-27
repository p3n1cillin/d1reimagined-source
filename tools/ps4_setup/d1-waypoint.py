"""Follow the objective waypoint on the 56578 client (shadPS4) by screen and pad input only.

The waypoint is a white diamond with a black ring inside a grey outer diamond. It is shown when an objective
updates, after a respawn, and after the touchpad (nav mode on and off). Off screen it is pinned to the nearest
edge with an arrow. Each look: touchpad twice, capture the game window, find the diamond.
  - in view: turn to centre it and sprint (left stick + L3); jump while sprinting if the last look did not move
  - pinned to an edge: turn 60 degrees toward it
  - not shown: turn 45 degrees
  - no movement for four looks (respawn prompt, or wedged): square, then drop stale position copies

  d1-waypoint.py --tracked tracked.json --seconds 180 [--shots DIR]
  d1-waypoint.py --find frame.png        (print the diamonds found in a capture)
"""
import argparse, ctypes, importlib.util, json, math, os, subprocess, sys, time

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
SHOT = os.environ.get('D1_SHOT', r'F:\d1\tools\d1-shot.ps1')
PAD = os.environ.get('D1_PAD', r'F:\d1\tools\d1-pad.ps1')
RATE = 2.26 * 1.14 * 110 / 90 / 1000     # rad per ms of right stick (a 90-degree command turned 110)
RX, HALF_W, HFOV = 968, 960, math.radians(78)   # reticle and half width of the 1936x1119 window capture


def _blobs(mask, limit=600):
    """4-connected components of a boolean mask as (ys, xs) arrays; stops growing a component at `limit`."""
    seen = np.zeros_like(mask)
    out = []
    for y0, x0 in zip(*np.nonzero(mask)):
        if seen[y0, x0]:
            continue
        stack, ys, xs = [(y0, x0)], [], []
        seen[y0, x0] = True
        while stack and len(ys) <= limit:
            y, x = stack.pop(); ys.append(y); xs.append(x)
            for v, u in ((y + 1, x), (y - 1, x), (y, x + 1), (y, x - 1)):
                if 0 <= v < mask.shape[0] and 0 <= u < mask.shape[1] and mask[v, u] and not seen[v, u]:
                    seen[v, u] = True; stack.append((v, u))
        out.append((np.array(ys), np.array(xs)))
    return out


def markers(img):
    """[(cx, cy, size)] of waypoint diamonds in an RGB capture, outside the fixed HUD."""
    a = img.astype(int)
    H, W, _ = a.shape
    white = a.min(axis=2) > 240
    white[:340, :340] = False            # radar
    white[H - 330:, :480] = False        # weapon and ammo
    white[H - 260:, W - 480:] = False    # overlay corner
    white[140:215, :] = False            # the player's health bar
    out = []
    for ys, xs in _blobs(white):
        w, h = xs.max() - xs.min() + 1, ys.max() - ys.min() + 1
        if not (6 <= w <= 28 and 6 <= h <= 28 and 20 <= len(ys) <= 450):
            continue
        if abs(w - h) > 3 or not 0.38 <= len(ys) / (w * h) <= 0.7:
            continue                     # a diamond core: square bounding box, about half filled
        cy, cx, r = int(ys.mean()), int(xs.mean()), max(w, h) // 2
        dark_axes, outer = 0, []
        for dy, dx in ((0, 1), (0, -1), (1, 0), (-1, 0)):
            ring = [a[cy + dy * k, cx + dx * k].max() for k in range(r + 1, r + 5)
                    if 0 <= cy + dy * k < H and 0 <= cx + dx * k < W]
            dark_axes += bool(ring) and min(ring) <= 70
            outer += [a[cy + dy * k, cx + dx * k] for k in range(r + 6, r + 11)
                      if 0 <= cy + dy * k < H and 0 <= cx + dx * k < W]
        if dark_axes < 4:
            continue                     # the black ring on all four axes
        g = np.array(outer)
        if ((g.max(axis=1) - g.min(axis=1) < 30) & (g.mean(axis=1) < 245)).mean() < 0.5:
            continue                     # the outer diamond is neutral grey
        out.append((cx, cy, int(max(w, h))))
    return out


def pad(keys):
    subprocess.run(['powershell', '-NoProfile', '-File', PAD, '-Keys', keys], capture_output=True)


def grab(path):
    from PIL import Image
    subprocess.run(['powershell', '-NoProfile', '-File', SHOT, '-Out', path], capture_output=True)
    return np.asarray(Image.open(path).convert('RGB'))


def key(vk, down):
    u32 = ctypes.WinDLL('user32')
    u32.keybd_event(vk, u32.MapVirtualKeyW(vk, 0), 0 if down else 2, None)


def sprint(ms, jump=False):
    """Left stick forward (W) with L3 (X) for ms; optionally a double jump (cross, NumPad2) early on."""
    pad('NumPad4')                       # focuses the emulator; square is a harmless reload
    key(0x57, True); time.sleep(0.12); key(0x58, True); time.sleep(0.09); key(0x58, False)
    spent = 0.21
    if jump:
        for _ in range(2):
            time.sleep(0.25); key(0x62, True); time.sleep(0.09); key(0x62, False)
        spent += 0.68
    time.sleep(max(0, ms / 1000 - spent)); key(0x57, False)


def load_walk():
    s = importlib.util.spec_from_file_location('d1walk', os.path.join(HERE, 'd1-walk.py'))
    m = importlib.util.module_from_spec(s); s.loader.exec_module(m)
    return m


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--find', help='print the waypoint diamonds in this capture and exit')
    ap.add_argument('--tracked', help="d1-walk's tracked position copies, to report and to detect a stall")
    ap.add_argument('--seconds', type=float, default=180)
    ap.add_argument('--run-ms', type=int, default=1500)
    ap.add_argument('--shots', help='keep every capture here')
    a = ap.parse_args()
    if a.find:
        from PIL import Image
        print(markers(np.asarray(Image.open(a.find).convert('RGB'))))
        return
    walk = load_walk() if a.tracked else None
    client = walk.Client() if walk else None
    tracked = json.load(open(a.tracked)) if a.tracked else None
    t_end, step, still, last = time.time() + a.seconds, 0, 0, None
    while time.time() < t_end:
        step += 1
        pad('Space,Space')               # touchpad twice: nav mode on and off leaves the waypoint shown
        time.sleep(0.4)
        img = grab(os.path.join(a.shots or HERE, f'waypoint-{step:03d}.png' if a.shots else 'waypoint.png'))
        found = markers(img)
        pos = walk.position(client, tracked) if walk else None
        still = still + 1 if (pos and last and math.dist(pos[:2], last[:2]) < 0.3) else 0
        last = pos
        where = f' at ({pos[0]:.1f}, {pos[1]:.1f}, {pos[2]:.1f})' if pos else ''
        if walk and still >= 4:
            pad('NumPad4')               # the respawn prompt is square
            time.sleep(2)
            tracked, still = walk.retrack(client, tracked), 0
            print(f'{step}: no movement for four looks; square, retrack', flush=True)
            continue
        if not found:
            pad('Hold:L:%d' % int(math.radians(45) / RATE))
            print(f'{step}: no waypoint; turn 45{where}', flush=True)
            continue
        cx, cy, size = min(found, key=lambda m: abs(m[0] - RX))
        if cx < 160 or cx > 2 * RX - 160:
            pad('Hold:%s:%d' % ('L' if cx > RX else 'J', int(math.radians(60) / RATE)))
            print(f'{step}: waypoint pinned at ({cx}, {cy}); turn 60{where}', flush=True)
            continue
        yaw = (cx - RX) / HALF_W * HFOV / 2
        if abs(cx - RX) > 40:
            pad(f'Hold:{"L" if yaw > 0 else "J"}:{max(15, int(abs(yaw) / RATE))}')
        sprint(a.run_ms, jump=still >= 1)
        print(f'{step}: waypoint ({cx}, {cy}) size {size}; turn {math.degrees(yaw):+.0f}; '
              f'sprint{" + jump" if still >= 1 else ""}{where}', flush=True)


if __name__ == '__main__':
    main()
