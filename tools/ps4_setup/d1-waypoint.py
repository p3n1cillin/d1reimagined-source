"""Follow the objective waypoint on the 56578 client (shadPS4) by screen and pad input only.

The waypoint is a white diamond with a black ring inside a grey outer diamond. It is shown when an objective
updates, after a respawn, and after the touchpad (nav mode on and off). Off screen it is pinned to the nearest
edge with an arrow. Each look: touchpad twice, capture the game window, find the diamond.
  - in view: turn to centre it and sprint (left stick + L3); jump while sprinting if the last look did not move
  - pinned to an edge: turn 60 degrees toward it
  - not shown: turn 45 degrees
  - no movement for four looks (respawn prompt, or wedged): square, then drop stale position copies

With --fight, an enemy nameplate (a solid red health bar with the name in white above it) takes priority:
aim below the plate's left part (the body is under the level badge) and fire an aim-down-sights burst.

  d1-waypoint.py --tracked tracked.json --seconds 180 [--shots DIR] [--fight]
  d1-waypoint.py --find frame.png        (print the diamonds and nameplates found in a capture)
"""
import argparse, ctypes, importlib.util, json, math, os, subprocess, sys, time

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import d1paths  # sibling: local paths from D1_ROOT or paths.json
SHOT = os.environ.get('D1_SHOT')   # else D1_TOOLS/d1-shot.ps1
PAD = os.environ.get('D1_PAD')     # else D1_TOOLS/d1-pad.ps1
RATE = 2.26 * 1.14 * 110 / 90 / 1000     # rad per ms of right stick (a 90-degree command turned 110)
RX, RY, HALF_W, HALF_H = 968, 571, 960, 540     # frame centre and half extents of the 1936x1119 window capture
AIM_Y = 652     # the reticle (hip and ADS) sits 81 px below the frame centre; aiming at RY fired over heads
HFOV, VFOV = math.radians(78), math.radians(48)


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


def plates(img):
    """[(cx, cy, width)] of enemy nameplate health bars in an RGB capture, outside the fixed HUD.

    A bar is a solid rectangle of one red (190, 54, 42), 3-16 px tall, with white name text just above it.
    A damaged bar is shorter than its plate; one down to 8 px wide still counts.
    """
    a = img.astype(int)
    H, W, _ = a.shape
    r, g, b = a[:, :, 0], a[:, :, 1], a[:, :, 2]
    red = (r > 150) & (g < 90) & (b < 90) & (r > 2.2 * g)
    red[150:205, W // 2 - 420:W // 2 + 420] = False   # the player's own health bar
    yy, xx = np.ogrid[:340, :340]
    red[:340, :340] &= (yy - 189) ** 2 + (xx - 185) ** 2 > 115 ** 2   # the radar disc only
    red[H - 330:, :480] = False                       # weapon and ammo
    red[H - 260:, W - 480:] = False                   # overlay corner
    rows = np.nonzero(red.sum(axis=1) >= 8)[0]
    if not len(rows):
        return []
    bands, start = [], rows[0]
    for p, q in zip(rows, rows[1:]):
        if q != p + 1:
            bands.append((start, p)); start = q
    bands.append((start, rows[-1]))
    out = []
    for y0, y1 in bands:
        if not 3 <= y1 - y0 + 1 <= 16 or y0 < 30:
            continue
        cols = np.nonzero(red[y0:y1 + 1].any(axis=0))[0]
        runs, s = [], cols[0]
        for p, q in zip(cols, cols[1:]):
            if q > p + 3:
                runs.append((s, p)); s = q
        runs.append((s, cols[-1]))
        for x0, x1 in runs:
            if not 8 <= x1 - x0 + 1 <= 300:
                continue
            box = a[y0:y1 + 1, x0:x1 + 1]
            solid = ((abs(box[:, :, 0] - 190) < 30) & (abs(box[:, :, 1] - 54) < 25) & (abs(box[:, :, 2] - 42) < 25)).mean()
            if solid < 0.75:
                continue                 # damage-direction arrows are triangles, not solid bars
            if (a[y0 - 30:y0 - 3, x0:min(x0 + 120, W)].min(axis=2) > 200).sum() < 25:
                continue                 # no name above it
            out.append((int(x0 + x1) // 2, int(y0 + y1) // 2, int(x1 - x0 + 1)))
    return out


def aim_point(cx, cy, w):
    """Where the body is for a plate (centre cx, cy, width w): under its left part, scaled by its full width."""
    full = max(w, 150)                   # the plate's left end stays put as the bar shrinks
    return cx - w // 2 + int(0.3 * full), cy + int(0.6 * full)


def ads_burst(ms=600):
    """Aim down sights (L2 = E), fire (R2 = O) for ms, release."""
    pad('NumPad4')
    key(0x45, True); time.sleep(0.3); key(0x4F, True); time.sleep(ms / 1000); key(0x4F, False); key(0x45, False)


def pad(keys):
    subprocess.run(['powershell', '-NoProfile', '-File', PAD or str(d1paths.tools() / 'd1-pad.ps1'), '-Keys', keys], capture_output=True)


def grab(path):
    from PIL import Image
    subprocess.run(['powershell', '-NoProfile', '-File', SHOT or str(d1paths.tools() / 'd1-shot.ps1'), '-Out', path], capture_output=True)
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
    ap.add_argument('--find', help='print the waypoint diamonds and nameplates in this capture and exit')
    ap.add_argument('--fight', action='store_true', help='engage enemy nameplates before following the waypoint')
    ap.add_argument('--tracked', help="d1-walk's tracked position copies, to report and to detect a stall")
    ap.add_argument('--seconds', type=float, default=180)
    ap.add_argument('--run-ms', type=int, default=1500)
    ap.add_argument('--shots', help='keep every capture here')
    a = ap.parse_args()
    if a.find:
        from PIL import Image
        img = np.asarray(Image.open(a.find).convert('RGB'))
        print('waypoints', markers(img), 'nameplates', plates(img))
        return
    walk = load_walk() if a.tracked else None
    client = walk.Client() if walk else None
    tracked = json.load(open(a.tracked)) if a.tracked else None
    t_end, step, still, last = time.time() + a.seconds, 0, 0, None
    while time.time() < t_end:
        step += 1
        # Both button and stick activity are required by the client's idle check, including
        # repeated combat looks whose target is already centred and needs no camera turn.
        pad('Space,Space,Hold:A:80,Hold:D:80')
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
        enemies = plates(img) if a.fight else []
        if enemies:
            cx, cy, w = min(enemies, key=lambda p: math.hypot(p[0] - RX, p[1] - AIM_Y))
            ax, ay = aim_point(cx, cy, w)
            keys = []
            if abs(ax - RX) > 20:
                keys.append(f'Hold:{"L" if ax > RX else "J"}:{max(15, int(abs((ax - RX) / HALF_W * HFOV / 2) / RATE))}')
            if abs(ay - AIM_Y) > 20:
                keys.append(f'Hold:{"K" if ay > AIM_Y else "I"}:{max(15, int(abs((ay - AIM_Y) / HALF_H * VFOV / 2) / RATE))}')
            if keys:
                pad(','.join(keys))
            ads_burst()
            still = 0
            print(f'{step}: nameplate ({cx}, {cy}) width {w}; aim {",".join(keys) or "on"}; burst{where}', flush=True)
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
