"""Hill-climb the z-leg cell value: try 8 headings with short walks, keep the best, until the client
completes the transition. Args: tracked.json rounds"""
import json, math, os, struct, subprocess, sys, time
S = os.path.dirname(os.path.abspath(sys.argv[0]))
WALK = r'F:\d1\public-work\d1reimagined-source\tools\ps4_setup\d1-walk.py'
src = open(WALK).read()
g = {'__name__': 'lib', '__file__': WALK}
exec(compile(src, 'd1-walk.py', 'exec'), g)
LOG = r'F:\d1\tools\shadPS4\0.19.0\user\log\shad_log.txt'
c = g['Client']()
tracked = json.load(open(sys.argv[1]))
trail = []


def completed():
    with open(LOG, 'rb') as f:
        f.seek(0, 2); f.seek(max(0, f.tell() - 4_000_000))
        return b"normal_z_leg' due to" in f.read()


def probe():
    out = os.path.join(S, 'gp.jsonl')
    subprocess.run(['python', '-I', r'F:\d1\tools\d1-watchwrite.py', '--seconds', '3', '--max-hits', '1',
                    '--exec', '0x77fb05', '--peek', 'r15=', '--peek', 'rbx+0x378', '--out', out], capture_output=True)
    for l in open(out):
        e = json.loads(l)
        if e.get('event') == 'exec':
            v = struct.unpack('<f', struct.pack('<I', int(e['peek']['rbx+0x378'], 16)))[0]
            return int(e['peek']['r15='], 16), v
    return None, None


def go(target):
    r = os.path.join(S, 'gr.json')
    json.dump([target], open(r, 'w'))
    subprocess.run(['python', '-I', WALK, '--tracked', sys.argv[1], '--route', r, '--max-seconds', '25',
                    '--tolerance', '2'], capture_output=True)
    return g['position'](c, tracked)


here = g['position'](c, tracked)
lvl, val = probe()
print('start', here, lvl, val, flush=True)
for rnd in range(int(sys.argv[2])):
    if completed():
        break
    best = (val, here)
    for k in range(8):
        a = k * math.pi / 4
        t = [here[0] + 10 * math.cos(a), here[1] + 10 * math.sin(a), here[2]]
        p = go(t)
        l2, v2 = probe()
        print(f'  heading {k * 45:3d}: at ({p[0]:.1f}, {p[1]:.1f}, {p[2]:.1f}) level {l2} value {v2}', flush=True)
        trail.append([p, l2, v2])
        if completed():
            print('COMPLETED at', p); json.dump(trail, open(os.path.join(S, 'trail.json'), 'w')); sys.exit()
        if v2 is not None and v2 > best[0] + 0.5:
            best = (v2, p)
        go(here)   # back to the round's centre
    if best[1] == here:
        print('no better heading'); break
    val, here = best
    go(here)
    print('round', rnd, 'moved to', here, 'value', val, flush=True)
json.dump(trail, open(os.path.join(S, 'trail.json'), 'w'))
print('completed' if completed() else 'not completed')
