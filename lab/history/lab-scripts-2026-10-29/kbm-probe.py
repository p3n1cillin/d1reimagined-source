"""Tap keys the pad config leaves unmapped (so only the game's own key layer sees them, D1_KBM=1) and
record the Guardian's position change plus before/after grabs.  Usage: kbm-probe.py OUTDIR KEY..."""
import ctypes, importlib.util, math, os, sys, time
os.environ['D1_LAB_DRIVER'] = '1'
HERE = os.path.dirname(os.path.abspath(__file__))
s = importlib.util.spec_from_file_location('c', os.path.join(HERE, 'combat2.py')); c = importlib.util.module_from_spec(s); s.loader.exec_module(c)
u = ctypes.WinDLL('user32')
NAMES = {'TAB': 0x09, 'SHIFT': 0xA0, 'CTRL': 0xA2, 'ALT': 0xA4, 'CAPS': 0x14, 'BACK': 0x08, 'GRAVE': 0xC0}
def code(k): return NAMES.get(k, ord(k) if len(k) == 1 else None)
def vk(k, d): u.keybd_event(k, u.MapVirtualKeyW(k, 0), 0 if d else 2, None)
W = c.World()
def pos():
    for _ in range(20):
        p = W.me()
        if p: return p[0]
        time.sleep(0.1)
out = sys.argv[1]
t0 = time.time()
while pos() is None and time.time() - t0 < 240: time.sleep(2)
if pos() is None: sys.exit('no player unit')
c._gf.ensure(); time.sleep(4)
for k in sys.argv[2:]:
    c.hold('A', 120); c.hold('D', 120); time.sleep(0.8)          # stick nudge, settle
    a = pos(); c.fight.grab(os.path.join(out, f'p-{k}-0.png'))
    vk(code(k), True); time.sleep(0.15); vk(code(k), False)
    time.sleep(0.35); c.fight.grab(os.path.join(out, f'p-{k}-1.png')); time.sleep(1.2)
    b = pos(); c.fight.grab(os.path.join(out, f'p-{k}-2.png'))
    d = [round(b[i] - a[i], 2) for i in range(3)] if a and b else None
    print(k, 'delta', d, flush=True)
    time.sleep(1.0)
