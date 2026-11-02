"""Print the Guardian's health as the screen shows it: a fraction, 'critical' (the red screen), or
'full' when no bar is drawn (the bar hides at full health). Read-only: no input is sent.

    python -I health_now.py [--wait-above 0.9 --max-s 10]

With --wait-above it polls until health is above that fraction (or the bar hides) or --max-s runs
out, and prints the last reading. It gives up after 3 s with no recovery (the Guardian is under
fire). The replay calls it before a fight: sense115 and sense117 stepped
out of the tripmine corridor at 0.12 health or less and died to the first Fallen they met.
"""
import argparse
import importlib.util
import os
import time

HERE = os.path.dirname(os.path.abspath(__file__))


def L(n):
    s = importlib.util.spec_from_file_location(n, os.path.join(HERE, f'{n}.py'))
    m = importlib.util.module_from_spec(s); s.loader.exec_module(m)
    return m


def reading(fight, hp, path):
    """(label, fraction): fraction is 1.0 for a hidden (full) bar and 0.0 for the red screen."""
    img = fight.grab(path)
    if hp.critical(img):
        return 'critical', 0.0
    h = hp.health(img)
    return ('full', 1.0) if h is None else (f'{h:.2f}', h)


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument('--wait-above', type=float)
    ap.add_argument('--max-s', type=float, default=10)
    a = ap.parse_args()
    fight, hp = L('fight'), L('health')
    import routepaths
    path = str(routepaths.probes() / 'health-now.png')
    label, h = reading(fight, hp, path)
    t0, h0 = time.time(), h
    t_end = t0 + a.max_s
    while a.wait_above is not None and h <= a.wait_above and time.time() < t_end:
        time.sleep(1.0)
        label, h = reading(fight, hp, path)
        # Only wait while the shield is coming back. Under fire it doesn't, and standing still
        # is the worst place to be: sense118 waited 10 s at 0.12 in the junction-west and died.
        # The fight that follows backs off to cover instead.
        if time.time() - t0 >= 3 and h <= h0 + 0.05:
            label += ' (not recovering: under fire)'
            break
    print(label, flush=True)


if __name__ == '__main__':
    import sys
    sys.path.insert(0, HERE)
    main()
