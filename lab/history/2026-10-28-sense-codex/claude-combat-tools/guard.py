"""Stand and fight (56578 prototype): engage nameplates in view, look around when none, back off when hurt.

    guard.py --seconds 120 [--shots DIR]
"""
import argparse, importlib.util, math, os, time

HERE = os.path.dirname(os.path.abspath(__file__))


def load(name, path=None):
    s = importlib.util.spec_from_file_location(name, path or os.path.join(HERE, name + '.py'))
    m = importlib.util.module_from_spec(s); s.loader.exec_module(m)
    return m


fight, hp, fo = load('fight'), load('health'), load('follow')
wp = load('wp', r'F:\d1\public-work\d1rs-claude\tools\ps4_setup\d1-waypoint.py')


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--seconds', type=float, default=120)
    ap.add_argument('--shots')
    a = ap.parse_args()
    t_end, step, engaged, quiet = time.time() + a.seconds, 0, 0, 0
    while time.time() < t_end:
        step += 1
        img = fight.grab(os.path.join(a.shots or HERE, f'g-{step:03d}.png' if a.shots else 'g.png'))
        h = hp.health(img)
        found = wp.plates(img)
        if h is not None and h < 0.6:
            # Break line of sight and let the shields come back (the bar disappears at full health),
            # then step forward to peek again - what the reference player does in this room.
            fight.pad('Hold:S:1000,Hold:A:400')
            # Shields need several seconds out of fire; the bar reading is unreliable while moving in
            # the dark, so wait a fixed 6 s (nudging the stick to stay active).
            waited = 0
            while waited < 6:
                time.sleep(1.5); waited += 1.5
                fight.pad('Hold:A:60')
            fight.pad('Hold:W:600')
            print(f'{step}: health {h:.2f}; retreated, waited {waited} s, peek', flush=True)
            continue
        if found:
            cx, cy, w, keys = fo.engage(found)
            engaged += 1; quiet = 0
            print(f'{step}: nameplate ({cx}, {cy}) width {w}; aim {",".join(keys) or "on"}; burst', flush=True)
            continue
        quiet += 1
        ang, n, near = fight.enemy_angle(img)
        if ang is not None and abs(ang) > math.radians(10):
            # Nameplates only show for enemies on screen: turn toward the radar's red sector first.
            fight.pad('Hold:%s:%d' % ('L' if ang > 0 else 'J', max(15, int(abs(ang) / fo.RATE))))
            print(f'{step}: radar enemy at {math.degrees(ang):+.0f} deg; turn', flush=True)
        elif ang is not None or near >= 300:
            # enemy ahead but no plate (out of sight or too dark): fire where we look
            fo.ads_burst(400)
            print(f'{step}: radar enemy ahead, no plate; burst', flush=True)
        else:
            fight.pad('Hold:L:%d' % int(math.radians(45) / fo.RATE))   # a stick move also keeps the client active
            print(f'{step}: none in view; look 45', flush=True)
    print(f'engagements {engaged}', flush=True)


if __name__ == '__main__':
    main()
