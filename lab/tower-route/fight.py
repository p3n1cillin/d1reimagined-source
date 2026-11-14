"""Prototype: turn toward the radar's red enemy sector and fire aimed bursts (56578, pad input only).

    fight.py --seconds 90 [--shots DIR]
"""
import argparse, math, os, subprocess, time
import numpy as np
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
import sys; sys.path.insert(0, HERE); import routepaths  # noqa: E402
SHOT = str(routepaths.SHOT)
PAD = str(routepaths.PAD)
RATE = 2.26 / 1000 * 1.14          # rad per ms of right-stick hold (d1-navfollow.py)
CX, CY, R_IN, R_OUT = 185, 189, 14, 95   # radar centre and the ring searched, full-res window capture


SHOT_GAP = 3.1                     # s between emulator screenshots while they carry a notification
SHOT_GAP_QUIET = 0.75              # s between them without (D1_SCREENSHOT_TOAST=0)
_last_shot = (0.0, None)


def _emulator_shot(h, rect, timeout=3.0):
    """The emulator's own game-only screenshot (its F12 hotkey), placed where the game's frame sits
    in the window, so the result lines up with a desktop grab of the window. None on failure.

    The desktop grab reads what Windows composed for the screen. With the monitor off or
    disconnected that is black whatever the game draws: every lab frame from about 04:15 on
    7 November was black (agr32-agr40), the combat bot fought blind, and a black screen after the
    Preacher (F55) was read into it. The emulator's screenshot is read back from the GPU instead.
    It lands in <shadPS4>/user/screenshots as <title>_<time>_game_<seq>.png; it is moved out once
    read, so the folder doesn't fill up.

    Each screenshot also queues a 3 s "Saved screenshot" notification holding the picture as a GPU
    texture, shown one at a time: several a second ran the GPU out of memory and lost the device
    (agr41, 7 November). So at most one per SHOT_GAP seconds, the last one standing in between,
    unless the bot's environment has D1_SCREENSHOT_TOAST=0, which says the emulator was started
    with fork #11's knob of that name (no notification): pass emulator:D1_SCREENSHOT_TOAST=0 to
    d1-full-game.ps1 and export it for the bot.

    Even without it, each screenshot is a PNG encode and a 2-3 MB write inside the emulator's GPU
    scheduler; several a second on the lab's spinning disk (F:) stalled the client into "Contacting
    Destiny Servers" and error weasel (agr42, 10:05). So never more than one per SHOT_GAP_QUIET s,
    and keep <shadPS4>/user/screenshots on an SSD (the lab's is a junction to E:)."""
    global _last_shot
    gap = SHOT_GAP_QUIET if os.environ.get('D1_SCREENSHOT_TOAST') == '0' else SHOT_GAP
    if _last_shot[1] is not None and time.time() - _last_shot[0] < gap:
        return _last_shot[1].copy()
    import ctypes, ctypes.wintypes as wt
    import d1_keys, d1paths
    shots = os.path.join(str(d1paths.shadps4()), 'user', 'screenshots')
    os.makedirs(shots, exist_ok=True)
    before = set(os.listdir(shots))
    d1_keys.press('F12', gap=0)
    deadline = time.time() + timeout
    while time.time() < deadline:
        new = sorted(f for f in os.listdir(shots) if f not in before and f.endswith('.png'))
        if new:
            f = os.path.join(shots, new[-1])
            try:
                im = Image.open(f)
                im.load()                      # raises while the emulator is still writing it
            except Exception:
                time.sleep(0.05)
                continue
            u32 = ctypes.windll.user32
            c, pt = wt.RECT(), wt.POINT(0, 0)
            u32.GetClientRect(h, ctypes.byref(c))
            u32.ClientToScreen(h, ctypes.byref(pt))
            cw, ch = c.right - c.left, c.bottom - c.top
            im = im.convert('RGB')
            if im.size != (cw, ch) and cw > 0 and ch > 0:
                im = im.resize((cw, ch))
            out = Image.new('RGB', (rect.right - rect.left, rect.bottom - rect.top))
            out.paste(im, (pt.x - rect.left, pt.y - rect.top))
            try:
                os.remove(f)
            except OSError:
                pass
            _last_shot = (time.time(), out)
            return out
        time.sleep(0.05)
    return None


_last_good = [None, 0.0]           # the last emulator frame and when it was taken
_reuse = [0]                       # frames reused in a row
_warned = [False]
MAX_FRAME_AGE = 5.0                # s: an older frame is not reused


def _no_posted_frame(why):
    """Posted mode has no desktop fallback (a locked desktop gives lock-screen pixels the readers
    would misread). With no usable frame, fail as a grab does today when the script leaves no
    file: Image.open raises FileNotFoundError."""
    if not _warned[0]:
        _warned[0] = True
        print('grab: no emulator frame (posted mode); no desktop fallback', flush=True)
    raise FileNotFoundError('no emulator frame (posted mode): ' + why)


def _posted_grab(path):
    import ctypes, ctypes.wintypes as wt
    import d1_keys
    im, why = None, 'no window'
    try:
        h = d1_keys._window()
        if h:
            r = wt.RECT()
            ctypes.windll.user32.GetWindowRect(h, ctypes.byref(r))
            im = _emulator_shot(h, r)
            why = 'screenshot failed'
    except Exception as e:
        im, why = None, 'screenshot error: %s' % e
    if im is not None:
        _last_good[0], _last_good[1], _reuse[0] = im, time.time(), 0
    else:
        age = time.time() - _last_good[1]
        if _last_good[0] is None or age > MAX_FRAME_AGE:
            _no_posted_frame(why)
        if _reuse[0] == 0:
            print('grab: reusing a frame %.1f s old' % age, flush=True)
        _reuse[0] += 1
        im = _last_good[0]
    im.save(path, compress_level=1)
    return np.asarray(im).astype(int)


def grab(path):
    """The shadPS4 window as an RGB int array, also saved to `path`.

    In-process (0.02 s, plus the PNG): d1-shot.ps1 took 1.0 s a frame (a PowerShell start, an
    Add-Type compile and a 400 ms wait for focus), so every combat pass saw the fight a second late.
    When the desktop grab is black (the monitor off, see _emulator_shot) the emulator's own
    screenshot stands in; the script is the fallback when both fail.

    Posted mode (D1_KEYS_POST=1, a locked desktop) never touches the desktop or the script: see
    _posted_grab."""
    import d1_keys
    if d1_keys.post_mode():
        return _posted_grab(path)
    try:
        import ctypes, ctypes.wintypes as wt
        from PIL import ImageGrab
        import d1_keys
        h = d1_keys._window()
        if h and d1_keys._focus(h):
            r = wt.RECT()
            ctypes.windll.user32.GetWindowRect(h, ctypes.byref(r))
            im = ImageGrab.grab((r.left, r.top, r.right, r.bottom), all_screens=True).convert('RGB')
            if max(hi for _, hi in im.getextrema()) < 64:
                im = _emulator_shot(h, r) or im
            im.save(path, compress_level=1)
            return np.asarray(im).astype(int)
    except Exception:
        pass
    subprocess.run(['powershell', '-NoProfile', '-File', SHOT, '-Out', path], capture_output=True)
    return np.asarray(Image.open(path).convert('RGB')).astype(int)


def enemy_angle(img):
    """Mean bearing (radians, + = right of facing) of red radar pixels, their count, and the near-disc count."""
    reg = img[CY - R_OUT:CY + R_OUT, CX - R_OUT:CX + R_OUT]
    yy, xx = np.mgrid[-R_OUT:R_OUT, -R_OUT:R_OUT]
    r = np.hypot(xx, yy)
    # The damage tint reddens the whole screen; measure red against the frame just outside the radar.
    outer = img[CY - R_OUT - 30:CY + R_OUT + 30, CX - R_OUT - 30:CX + R_OUT + 30]
    oy, ox = np.mgrid[-R_OUT - 30:R_OUT + 30, -R_OUT - 30:R_OUT + 30]
    bg = float(np.median(outer[(np.hypot(ox, oy) > R_OUT + 8) & (np.hypot(ox, oy) < R_OUT + 28)][:, 0]))
    red = ((reg[:, :, 0] - bg > 35) & (reg[:, :, 0] > 2.2 * reg[:, :, 1]) & (reg[:, :, 0] > 2.2 * reg[:, :, 2]))
    ring = red & (r > R_IN + 25) & (r < R_OUT)
    near = int((red & (r <= R_IN + 25)).sum())
    if ring.sum() < 100:   # a faint arc measured 177, a quiet frame 42
        return None, int(ring.sum()), near
    ang = np.arctan2(xx[ring], -yy[ring])          # 0 = up (facing), + = right
    return float(np.arctan2(np.sin(ang).mean(), np.cos(ang).mean())), int(ring.sum()), near


def pad(keys):
    try:
        import d1_keys                  # in-process (d1-walk does the same); the script is the fallback
        d1_keys.press(keys)
        return
    except Exception:
        pass
    cmd = ['powershell', '-NoProfile', '-File', PAD, '-Keys', keys]
    try:
        import d1_keys
        if d1_keys.post_mode():
            cmd.append('-Post')         # never real input with the desktop locked
    except Exception:
        pass
    subprocess.run(cmd, capture_output=True)


def burst(ms):
    # Hip fire: a held R2 through the pad driver empties the magazine (measured 12 -> reload).
    pad('Hold:O:%d' % ms)


def engage(a, state, budget):
    """Fight until the radar has been clear for three looks or the budget runs out; True if anything was seen."""
    t_end, clear, seen = time.time() + budget, 0, False
    while time.time() < t_end and clear < 3:
        state['step'] += 1
        step = state['step']
        img = grab(os.path.join(a.shots or HERE, f'fight-{step:03d}.png' if a.shots else 'fight.png'))
        ang, n, near = enemy_angle(img)
        if ang is None and near < 300:
            clear += 1
            if seen:
                pad('Hold:L:%d' % int(math.radians(60) / RATE))
            print(f'{step}: radar clear (ring {n}, near {near})', flush=True)
            continue
        seen, clear = True, 0
        if ang is not None and abs(ang) > math.radians(12):
            pad('Hold:%s:%d' % ('L' if ang > 0 else 'J', int(abs(ang) / RATE)))
            print(f'{step}: enemy at {math.degrees(ang):+.0f} deg (ring {n}, near {near}); turn', flush=True)
        burst(900)
        print(f'{step}: burst (ring {n}, near {near})', flush=True)
    return seen


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--seconds', type=float, default=90)
    ap.add_argument('--shots')
    ap.add_argument('--route', help='advance along this d1-walk route two waypoints at a time, fighting between')
    ap.add_argument('--tracked')
    a = ap.parse_args()
    state = {'step': 0}
    if not a.route:
        engage(a, state, a.seconds)
        return
    import json
    walk = str(routepaths.WALK)
    points = json.load(open(a.route))
    t_end, i = time.time() + a.seconds, 0
    while i < len(points) and time.time() < t_end:
        leg = os.path.join(a.shots or HERE, 'leg.json')
        json.dump(points[i:i + 2], open(leg, 'w'))
        out = subprocess.run(['python', '-I', '-u', walk, '--tracked', a.tracked, '--route', leg, '--max-seconds', '25',
                              '--tolerance', '1.5', '--turn-sign', '1', '--turn-rate', '2.26'],
                             capture_output=True, text=True).stdout.strip().splitlines()
        print(f'leg {i}: {out[-1] if out else "(no output)"}', flush=True)
        i += 2
        engage(a, state, 40)


if __name__ == '__main__':
    main()
