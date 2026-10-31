"""Small driving steps for the lab: drive.py SHOT turn:DEG walk:MS strafe:MS back:MS ... ; prints pose, grabs SHOT.
turn:+DEG turns left (counter-clockwise), negative right. Holds the pause itself (D1_LAB_DRIVER)."""
import importlib.util, math, os, sys, time
os.environ['D1_LAB_DRIVER'] = '1'
HERE = os.path.dirname(os.path.abspath(__file__))
s = importlib.util.spec_from_file_location('c', os.path.join(HERE, 'combat2.py')); c = importlib.util.module_from_spec(s); s.loader.exec_module(c)
W = c.World()
for step in sys.argv[2:]:
    k, v = step.split(':'); v = float(v)
    if k == 'turn': c.hold('J' if v > 0 else 'L', abs(math.radians(v)) / c.YAW_RATE)
    elif k == 'walk': c.hold('W', v)
    elif k == 'back': c.hold('S', v)
    elif k == 'strafe': c.hold('D' if v > 0 else 'A', abs(v))
    elif k == 'face':                                   # absolute yaw in degrees
        for _ in range(3):
            p = W.me()
            if p is None: time.sleep(0.2); continue
            e = c.wrap(math.radians(v) - p[1])
            if abs(e) < 0.06: break
            c.hold('J' if e > 0 else 'L', max(25, abs(e) / c.YAW_RATE))
    time.sleep(0.4)
c.release_all(); time.sleep(0.5)
p = None
for _ in range(20):
    p = W.me()
    if p: break
    time.sleep(0.15)
if p: print('pos %.1f,%.1f,%.1f yaw %.0f' % (*p[0], math.degrees(p[1])))
c.fight.grab(sys.argv[1])
