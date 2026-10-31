"""Fighter v2 (56578 prototype, pad input only).

    fight2.py --seconds 180 [--shots DIR] [--retreat-ms 900] [--low 0.6]
Each look: read the player's health bar (health.py) and enemy nameplates (plates.py).
  - health shown and below --low: back off (left stick down) and wait for the bar to clear, then step back in
  - a nameplate: aim below it (scaled by the plate's width), fire a short burst, look again
  - a plate seen in the last few looks but not now: small alternating search turns, no sweep
  - otherwise: turn toward the radar's red sector, or sweep 30 degrees
"""
import argparse, importlib.util, math, os, time
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))


def load(name):
    s = importlib.util.spec_from_file_location(name, os.path.join(HERE, name + '.py'))
    m = importlib.util.module_from_spec(s); s.loader.exec_module(m)
    return m


plates, fight, hp = load('plates'), load('fight'), load('health')
RATE = 2.26 * 1.14 * 110 / 90 / 1000     # rad per ms of stick, measured (a 90-degree command turned 110)
RX, RY = 968, 571                          # the reticle in the 1936x1119 window capture
HALF_W, HALF_H = 960, 540
HFOV, VFOV = math.radians(78), math.radians(48)


def turn_keys(dx, dy, dead=24):
    keys = []
    yaw = dx / HALF_W * HFOV / 2
    pitch = dy / HALF_H * VFOV / 2
    if abs(dx) > dead:
        keys.append(f'Hold:{"L" if yaw > 0 else "J"}:{max(15, int(abs(yaw) / RATE))}')
    if abs(dy) > dead:
        keys.append(f'Hold:{"K" if pitch > 0 else "I"}:{max(15, int(abs(pitch) / RATE))}')
    return keys


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--seconds', type=float, default=180)
    ap.add_argument('--shots')
    ap.add_argument('--retreat-ms', type=int, default=900)
    ap.add_argument('--low', type=float, default=0.6)
    a = ap.parse_args()
    t_end, step, last_seen, search = time.time() + a.seconds, 0, -99, 1
    while time.time() < t_end:
        step += 1
        path = os.path.join(a.shots or HERE, f'f2-{step:03d}.png' if a.shots else 'f2.png')
        img = fight.grab(path)
        h = hp.health(img)
        if h is not None and h < a.low:
            fight.pad(f'Hold:S:{a.retreat_ms}')
            waited = 0
            while waited < 10:
                time.sleep(1.5); waited += 1.5
                if hp.health(fight.grab(os.path.join(HERE, 'f2-wait.png'))) is None:
                    break
            fight.pad(f'Hold:W:{a.retreat_ms // 2}')
            print(f'{step}: health {h:.2f}; backed off, waited {waited:.0f} s', flush=True)
            continue
        found = plates.plates(img)
        if found:
            last_seen = step
            cx, cy, w = min(found, key=lambda p: math.hypot(p[0] - RX, p[1] - RY))[:3]
            keys = turn_keys(cx - RX, cy + max(30, int(w * 0.4)) - RY)
            if keys:
                fight.pad(','.join(keys))
            fight.pad('Hold:O:450')
            print(f'{step}: plate ({cx}, {cy}) w {w}; health {h}; aim {",".join(keys) or "on"}; burst', flush=True)
            continue
        if step - last_seen <= 3:
            search = -search
            fight.pad('Hold:%s:%d' % ('L' if search > 0 else 'J', int(math.radians(8 * (step - last_seen)) / RATE)))
            print(f'{step}: lost the plate; search', flush=True)
            continue
        ang, n, near = fight.enemy_angle(img)
        if ang is None and near < 300:
            fight.pad('Hold:L:%d' % int(math.radians(30) / RATE))
            print(f'{step}: nothing; sweep 30', flush=True)
        elif ang is not None and abs(ang) > math.radians(12):
            fight.pad('Hold:%s:%d' % ('L' if ang > 0 else 'J', int(abs(ang) / RATE)))
            print(f'{step}: radar enemy at {math.degrees(ang):+.0f} deg; turn', flush=True)
        else:
            fight.pad('Hold:O:450')
            print(f'{step}: radar enemy ahead or close; burst', flush=True)


if __name__ == '__main__':
    main()
