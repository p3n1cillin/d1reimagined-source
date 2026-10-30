"""Follow the game's own objective waypoint by pad input, and record the way as a route (56578).

    d1-navfollow.py --tracked tracked.json --template diamond.png --out route.json
                    [--max-seconds 900] [--shots DIR]

The objective's waypoint diamond is shown in Ghost Nav Mode (touchpad, keyboard Space). Each step
takes a screenshot of the game window, finds the diamond by normalised cross-correlation against
a template cut from an earlier frame (a white diamond with a dark outline), turns toward it by its
horizontal offset, and walks when it is roughly centred. A push that does not move gets a jump,
then a side-step. When no diamond is found, Nav Mode is re-opened, then the camera turns 60
degrees. The player's positions (d1-walk.py tracking) are written as a d1-walk.py route. Pad
input only; guest memory is read, never written.
"""
import argparse, importlib.util, json, math, os, subprocess, sys, time
import numpy as np
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location('d1walk', os.path.join(HERE, 'd1-walk.py'))
walk = importlib.util.module_from_spec(spec)
spec.loader.exec_module(walk)

sys.path.insert(0, HERE)
import d1paths  # sibling: local paths from D1_ROOT or paths.json
RATE = 2.26 / 1000 * 1.14   # rad per ms of right-stick hold, as measured with d1-step.py
HFOV = math.radians(78)     # horizontal field of view of the game camera (approximate)


def grab(path):
    subprocess.run(['powershell', '-NoProfile', '-File', str(d1paths.tools() / 'd1-shot.ps1'), '-Out', path], capture_output=True)
    return np.asarray(Image.open(path).convert('L'), dtype=np.float32)


def find(img, tpl, threshold):
    """Best normalised cross-correlation match of tpl in img (FFT, 2x downscaled): (x, y, score)."""
    a = img[::2, ::2].astype(np.float64)
    t = tpl[::2, ::2].astype(np.float64)
    t = (t - t.mean()) / (t.std() + 1e-6)
    th, tw = t.shape
    shape = (a.shape[0] + th, a.shape[1] + tw)
    corr = np.fft.irfft2(np.fft.rfft2(a, shape) * np.conj(np.fft.rfft2(t, shape)), shape)
    corr = corr[:a.shape[0] - th, :a.shape[1] - tw]
    # Local mean and energy of the image under the template, by cumulative sums.
    c1 = np.pad(a.cumsum(0).cumsum(1), ((1, 0), (1, 0)))
    c2 = np.pad((a * a).cumsum(0).cumsum(1), ((1, 0), (1, 0)))
    s1 = c1[th:, tw:] - c1[:-th, tw:] - c1[th:, :-tw] + c1[:-th, :-tw]
    s2 = c2[th:, tw:] - c2[:-th, tw:] - c2[th:, :-tw] + c2[:-th, :-tw]
    n = th * tw
    var = np.maximum(s2[:corr.shape[0], :corr.shape[1]] - s1[:corr.shape[0], :corr.shape[1]] ** 2 / n, 1e-3)
    score = corr / np.sqrt(var) / math.sqrt(n)
    score[var < n * 25] = 0    # flat areas (local std under 5 grey levels) cannot hold the outlined diamond
    y, x = np.unravel_index(np.argmax(score), score.shape)
    s = float(score[y, x])
    if s < threshold:
        return None
    return (x + tw / 2) * 2, (y + th / 2) * 2, s


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--tracked', required=True)
    ap.add_argument('--template', required=True)
    ap.add_argument('--out', required=True)
    ap.add_argument('--max-seconds', type=float, default=900)
    ap.add_argument('--threshold', type=float, default=0.7)
    ap.add_argument('--shots', help='keep each step\'s screenshot here')
    a = ap.parse_args()
    tpl = np.asarray(Image.open(a.template).convert('L'), dtype=np.float32)
    c = walk.Client()
    tracked = json.load(open(a.tracked))
    here = walk.position(c, tracked)
    trail = [list(here)]
    shot = os.path.join(a.shots or os.path.dirname(os.path.abspath(a.out)), 'navfollow.png')
    misses, stuck, step = 0, 0, 0
    t_end = time.time() + a.max_seconds
    print('start', here, flush=True)
    while time.time() < t_end:
        step += 1
        img = grab(shot)
        if a.shots:
            Image.fromarray(img.astype(np.uint8)).save(os.path.join(a.shots, f'step{step:04d}.png'))
        hit = find(img, tpl, a.threshold)
        if not hit:
            misses += 1
            walk.pad('Space' if misses % 3 == 1 else 'Hold:L:%d' % int(math.radians(60) / RATE))
            print(f'step {step}: no waypoint ({"nav mode" if misses % 3 == 1 else "turn 60"})', flush=True)
            continue
        misses = 0
        x, y, s = hit
        w = img.shape[1]
        ang = (x - w / 2) / (w / 2) * (HFOV / 2)
        if abs(ang) > math.radians(7):
            walk.pad(f'Hold:{"L" if ang > 0 else "J"}:{int(abs(ang) / RATE)}')
            print(f'step {step}: waypoint at x={x:.0f} (score {s:.2f}), turn {math.degrees(ang):+.0f} deg', flush=True)
            continue
        p0 = here
        walk.pad('Hold:W:1200')
        here = walk.position(c, tracked, p0) or p0
        d = math.dist(p0[:2], here[:2]) + abs(p0[2] - here[2])
        if d < 0.6:
            stuck += 1
            walk.pad('NumPad2,Hold:W:800' if stuck % 2 else ('Hold:A:700' if stuck % 4 == 2 else 'Hold:D:700'))
            here = walk.position(c, tracked, here) or here
        else:
            stuck = 0
        if math.dist(trail[-1], here) > 30:
            print(f'step {step}: jumped to {here} (respawn?); trail restarts', flush=True)
            trail = []
        trail.append([round(v, 2) for v in here])
        json.dump(trail, open(a.out, 'w'))
        walk.active(force=False)
        print(f'step {step}: walked to ({here[0]:.1f}, {here[1]:.1f}, {here[2]:.1f}) moved {d:.1f}', flush=True)
    print('stopped', here, f'trail {len(trail)} -> {a.out}')


if __name__ == '__main__':
    main()
