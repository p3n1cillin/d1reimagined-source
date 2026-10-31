"""Seek a world point through unknown geometry (bug-style): face it natively and walk; when blocked,
follow the wall on one side, switching side after repeated failure. Logs the trail (for new nav edges).

    seek.py X,Y [--max-s 120] [--tol 2.5] [--trail FILE]
Exit 0 arrived, 1 timeout, 3 unit changed (death).
"""
import argparse, importlib.util, json, math, os, random, sys, time

HERE = os.path.dirname(os.path.abspath(__file__))
s = importlib.util.spec_from_file_location('c', os.path.join(HERE, 'combat2.py')); c = importlib.util.module_from_spec(s); s.loader.exec_module(c)


def face(W, yaw_goal):
    for _ in range(3):
        _, yaw, _ = W.me()
        e = c.wrap(yaw_goal - yaw)
        if abs(e) < 0.08: return
        c.hold('J' if e > 0 else 'L', max(25, abs(e) / c.YAW_RATE))


def main():
    ap = argparse.ArgumentParser(); ap.add_argument('target'); ap.add_argument('--max-s', type=float, default=120)
    ap.add_argument('--tol', type=float, default=2.5); ap.add_argument('--trail'); ap.add_argument('--no-nav', action='store_true'); a = ap.parse_args()
    tx, ty = map(float, a.target.split(','))
    start = time.time()
    W = c.World(); c.fight.pad('NumPad4')
    # Walk the known graph to the node nearest the target first; free (bug) walking only covers the rest.
    # The whole run stays within --max-s: nav gets 60% of it and is stopped (keys released) if it
    # overruns, so no walker outlives this process and drives alongside the next tool.
    import subprocess
    nav_s = min(150, a.max_s * 0.6)
    try:
        r = subprocess.CompletedProcess([], 0) if a.no_nav else subprocess.run(
            [sys.executable, '-u', os.path.join(HERE, 'nav.py'), a.target, '--max-s', str(nav_s)],
            capture_output=True, text=True, timeout=nav_s + 45)
    except subprocess.TimeoutExpired:
        c.release_all(); r = subprocess.CompletedProcess([], 1)
    if r.returncode == 3: print('stopped: unit changed (death) during nav', flush=True); sys.exit(3)
    trail, t_end, side, fails = [], start + a.max_s, random.choice((1, -1)), 0
    try:
        while time.time() < t_end:
            me, _, pitch = W.me(); trail.append([round(v, 2) for v in me])
            d = math.dist(me[:2], (tx, ty))
            if d < a.tol:
                print(f'arrived {me[0]:.1f},{me[1]:.1f},{me[2]:.1f} d {d:.1f}', flush=True); break
            if abs(pitch) > 0.1: c.hold('I' if pitch < 0 else 'K', abs(pitch) / c.PITCH_RATE)
            face(W, math.atan2(ty - me[1], tx - me[0]))
            c.hold('W', 900)
            me2 = W.me()[0]
            if math.dist(me2[:2], (tx, ty)) < d - 0.4:
                fails = 0; continue
            # Blocked: slide along the obstacle on one side, then retry the direct heading.
            fails += 1
            if fails % 3 == 0: side = -side
            ang = math.atan2(ty - me2[1], tx - me2[0]) + side * math.radians(70 + 15 * (fails % 3))
            face(W, ang); c.hold('W', 1200 + 300 * (fails % 4))
            if fails % 4 == 3: c.tap('JUMP', 80); c.hold('W', 600)
        else:
            me = W.me()[0]; print(f'timeout {me[0]:.1f},{me[1]:.1f},{me[2]:.1f} d {math.dist(me[:2], (tx, ty)):.1f}', flush=True)
            sys.exit(1)
    except RuntimeError as e:
        print('stopped:', e, flush=True); c.release_all(); sys.exit(3)
    finally:
        c.release_all()
        if a.trail: json.dump(trail, open(a.trail, 'a')); open(a.trail, 'a').write('\n')


if __name__ == '__main__':
    main()
