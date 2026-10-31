"""Native aim: turn yaw (forward vector 0x230/0x3A0 xy) and pitch (0x3A0 z) onto a world point, then fire.
    naim.py X Y Z [--fire-ms 1500] [--eye 1.4]"""
import argparse, importlib.util, math, os, struct, sys, time
HERE = os.path.dirname(os.path.abspath(__file__)); SENSE = os.path.join(HERE, 'pr57'); sys.path.insert(0, SENSE)
s = importlib.util.spec_from_file_location('g', os.path.join(SENSE, 'd1-winguest.py')); g = importlib.util.module_from_spec(s); s.loader.exec_module(g)
import d1_unit_tracking as ut
s = importlib.util.spec_from_file_location('fo', os.path.join(HERE, 'follow.py')); fo = importlib.util.module_from_spec(s); s.loader.exec_module(fo)
YAW_RATE, PITCH_RATE = 2.76e-3, 1.2e-3
def wrap(a): return (a + math.pi) % (2 * math.pi) - math.pi
class Aim:
    def __init__(self):
        self.p, self.base = g.open_client()
    def pose(self):
        u = ut.read_unit_position(self.p, self.base)
        if u is None: raise RuntimeError('no unit')
        x, y, z = struct.unpack('<3f', self.p.read(u.address + ut.FORWARD_VECTORS[1], 12))
        return u.position, math.atan2(y, x), math.asin(max(-1, min(1, z)))
    def onto(self, t, eye=1.4, tol=0.035, tries=6):
        for _ in range(tries):
            pos, yaw, pitch = self.pose()
            dx, dy, dz = t[0] - pos[0], t[1] - pos[1], t[2] - (pos[2] + eye)
            ty, tp = math.atan2(dy, dx), math.atan2(dz, math.hypot(dx, dy))
            ey, ep = wrap(ty - yaw), tp - pitch
            if abs(ey) < tol and abs(ep) < tol: return True, math.hypot(dx, dy)
            keys = []
            if abs(ey) >= tol: keys.append('Hold:%s:%d' % ('L' if ey < 0 else 'J', max(25, int(abs(ey) / YAW_RATE))))
            if abs(ep) >= tol: keys.append('Hold:%s:%d' % ('I' if ep > 0 else 'K', max(25, int(abs(ep) / PITCH_RATE))))
            fo.fight.pad(','.join(keys))
        return False, None
    def fire(self, ms, ads=True):
        try:
            if ads: fo.key(0x45, True); time.sleep(0.3)
            fo.key(0x4F, True); time.sleep(ms / 1000)
        finally:
            fo.key(0x4F, False); fo.key(0x45, False)
if __name__ == '__main__':
    ap = argparse.ArgumentParser(); ap.add_argument('x', type=float); ap.add_argument('y', type=float); ap.add_argument('z', type=float)
    ap.add_argument('--fire-ms', type=int, default=1500); ap.add_argument('--eye', type=float, default=1.4); a = ap.parse_args()
    A = Aim(); ok, d = A.onto((a.x, a.y, a.z), a.eye); print('aimed', ok, 'dist', d, flush=True)
    if ok and a.fire_ms: A.fire(a.fire_ms)
