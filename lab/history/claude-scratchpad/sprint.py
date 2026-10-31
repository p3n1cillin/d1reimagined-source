"""Sprint the Guardian through waypoints (56578 prototype, pad input only).

    sprint.py TRACKED.json '[[x,y],[x,y],...]' [--seconds 12]
Aims with the right stick by the memory facing (d1-walk.py), then holds the left stick forward and
taps L3 (X) to sprint, polling the position; re-aims between waypoints or if the heading drifts.
"""
import argparse, ctypes, importlib.util, json, math, time

spec = importlib.util.spec_from_file_location('walk', r'F:\d1\public-work\d1reimagined-source\tools\ps4_setup\d1-walk.py')
walk = importlib.util.module_from_spec(spec); spec.loader.exec_module(walk)
u32 = ctypes.WinDLL('user32')
VK_W, VK_X, VK_Q, VK_U = 0x57, 0x58, 0x51, 0x55
RATE = 2.26 / 1000          # rad per ms of right stick (d1-walk --turn-rate 2.26, sign +1)


def key(vk, down):
    u32.keybd_event(vk, u32.MapVirtualKeyW(vk, 0), 0 if down else 2, None)


def focus():
    import subprocess
    # d1-pad focuses the emulator; a harmless square press (reload) also counts as activity.
    walk.pad('NumPad4')


def cast_super():
    """L1 + R1 together (Q + U): the Striker's Fist of Havoc."""
    focus()
    key(VK_Q, True); key(VK_U, True); time.sleep(0.25); key(VK_U, False); key(VK_Q, False)
    time.sleep(3.5)


def aim(c, tracked, target, tol=0.15):
    for _ in range(6):
        here = walk.position(c, tracked)
        head = walk.facing(c, tracked)
        if here is None or head is None:
            return here
        err = walk.wrap(math.atan2(target[1] - here[1], target[0] - here[0]) - head)
        if abs(err) <= tol:
            return here
        walk.pad(f'Hold:{"L" if err < 0 else "J"}:{int(min(900, abs(err) / RATE))}')
        time.sleep(0.2)
    return walk.position(c, tracked)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('tracked')
    ap.add_argument('route')
    ap.add_argument('--seconds', type=float, default=12)
    ap.add_argument('--reach', type=float, default=2.5)
    ap.add_argument('--super-after', type=int, default=-1, help='cast the Super after reaching this waypoint index')
    a = ap.parse_args()
    tracked = json.load(open(a.tracked))
    c = walk.Client()
    for idx, target in enumerate(json.loads(a.route)):
        here = aim(c, tracked, target)
        print(f'aim at {target} from {here}', flush=True)
        focus()
        key(VK_W, True); time.sleep(0.12); key(VK_X, True); time.sleep(0.09); key(VK_X, False)
        t_end, best = time.time() + a.seconds, 1e9
        try:
            while time.time() < t_end:
                time.sleep(0.12)
                here = walk.position(c, tracked)
                if here is None:
                    continue
                d = math.dist(here[:2], target[:2])
                best = min(best, d)
                if d < a.reach or d > best + 3:
                    break
                head = walk.facing(c, tracked)
                if head is not None and abs(walk.wrap(math.atan2(target[1] - here[1], target[0] - here[0]) - head)) > 0.5 and d > 4:
                    break
        finally:
            key(VK_W, False)
        print(f'  at {here} dist {d:.1f} (best {best:.1f})', flush=True)
        if d > a.reach + 4:
            here = aim(c, tracked, target)
            focus()
            key(VK_W, True); time.sleep(0.12); key(VK_X, True); time.sleep(0.09); key(VK_X, False)
            t2 = time.time() + a.seconds
            try:
                while time.time() < t2:
                    time.sleep(0.12)
                    here = walk.position(c, tracked) or here
                    if math.dist(here[:2], target[:2]) < a.reach:
                        break
            finally:
                key(VK_W, False)
            print(f'  retry at {here}', flush=True)
        if idx == a.super_after:
            cast_super()
            print('  super cast', flush=True)


if __name__ == '__main__':
    main()
