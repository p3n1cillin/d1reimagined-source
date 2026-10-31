"""Walk speed probe: hold W for N ms, report distance / wall time from the unit position (game-speed check)."""
import importlib.util, math, sys, time
s = importlib.util.spec_from_file_location('c', 'combat2.py'); c = importlib.util.module_from_spec(s); s.loader.exec_module(c)
W = c.World(); ms = int(sys.argv[1]) if len(sys.argv) > 1 else 3000
def pos():
    for _ in range(40):
        p = W.me()
        if p: return p[0]
        time.sleep(0.1)
for turn in (0, 90, 90, 90):
    if turn: c.hold('J', math.radians(turn) / c.YAW_RATE); time.sleep(0.5)
    a = pos(); t0 = time.time(); c.hold('W', ms); time.sleep(0.6); b = pos()
    d = math.dist(a[:2], b[:2]); print(f'walked {d:.2f} m in {ms} ms hold -> {d / (ms / 1000):.2f} m/s')
