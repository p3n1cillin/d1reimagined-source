"""Jump airtime probe (game-speed check): tap jump, sample the unit's z until it lands, report wall airtime."""
import ctypes, importlib.util, sys, time
s = importlib.util.spec_from_file_location('c', 'combat2.py'); c = importlib.util.module_from_spec(s); s.loader.exec_module(c)
W = c.World()
def z():
    p = W.me(); return p[0][2] if p else None
for n in range(4):
    time.sleep(1.5)
    z0 = None
    while z0 is None: z0 = z()
    c.key('JUMP', True); time.sleep(0.08); c.key('JUMP', False)
    t0 = time.time(); top = z0; up = None; land = None
    while time.time() - t0 < 4:
        v = z()
        if v is None: continue
        top = max(top, v)
        if up is None and v > z0 + 0.05: up = time.time()
        if up and v <= z0 + 0.02: land = time.time(); break
        time.sleep(0.01)
    print(f'jump {n}: rise {top - z0:.2f} m, airtime {(land - up) if (up and land) else float("nan"):.2f} s (wall)')
