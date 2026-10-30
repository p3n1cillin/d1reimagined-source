"""One step of hand-guided exploration: turn, move, log the position, screenshot (56578).

    d1-step.py --tracked tracked.json --trail trail.json [--turn DEG] [--walk MS] [--jump]
               [--keys K1,K2] [--shot out.png]

For finding routes the explorer cannot: look at the screenshot, decide, take the next step.
--turn is degrees to the right (negative = left), using d1-walk.py's calibrated right-stick rate.
--walk holds forward for MS milliseconds (--jump presses cross first). Each call appends the
player's position to the trail (a JSON list usable as a d1-walk.py route once loops are cut with
d1-explore.py's simplify) and prints it. Pad input only; guest memory is read, never written.
"""
import argparse, importlib.util, json, math, os, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import d1paths  # sibling: local paths from D1_ROOT or paths.json


def load(name, file):
    spec = importlib.util.spec_from_file_location(name, os.path.join(HERE, file))
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


walk = load('d1walk', 'd1-walk.py')
RATE = 2.26 / 1000  # rad per ms of right-stick hold (calibrated by d1-walk.py, 26 Oct)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--tracked', required=True)
    ap.add_argument('--trail', required=True)
    ap.add_argument('--turn', type=float, default=0)
    ap.add_argument('--walk', type=int, default=0)
    ap.add_argument('--jump', action='store_true')
    ap.add_argument('--keys', default='')
    ap.add_argument('--shot')
    a = ap.parse_args()
    c = walk.Client()
    tracked = json.load(open(a.tracked))
    trail = json.load(open(a.trail)) if os.path.exists(a.trail) else []
    p0 = walk.position(c, tracked, trail[-1] if trail else None)
    keys = []
    if a.turn:
        ms = int(abs(math.radians(a.turn)) / RATE)
        keys.append(f'Hold:{"L" if a.turn > 0 else "J"}:{ms}')
    if a.jump:
        keys.append('NumPad2')
    if a.walk:
        keys.append(f'Hold:W:{a.walk}')
    keys += [k for k in a.keys.split(',') if k]
    if keys:
        walk.pad(*keys)
    p1 = walk.position(c, tracked, p0) or p0
    heading = None
    if p0 and p1 and math.dist(p0[:2], p1[:2]) > 0.6:
        heading = math.degrees(math.atan2(p1[1] - p0[1], p1[0] - p0[0]))
    if p1:
        trail.append([round(v, 2) for v in p1])
        json.dump(trail, open(a.trail, 'w'))
    moved = math.dist(p0[:2], p1[:2]) if p0 and p1 else 0
    print(f'at ({p1[0]:.1f}, {p1[1]:.1f}, {p1[2]:.1f}) moved {moved:.1f}'
          + (f' heading {heading:.0f} deg' if heading is not None else '') + f' trail {len(trail)}')
    if a.shot:
        subprocess.run(['powershell', '-NoProfile', '-File', str(d1paths.tools() / 'd1-shot.ps1'), '-Out', a.shot],
                       capture_output=True)


if __name__ == '__main__':
    main()
