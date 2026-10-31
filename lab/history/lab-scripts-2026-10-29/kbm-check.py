"""Native keyboard layer check (D1_KBM): with W unmapped in the emulator's pad config and the pad's
stick-up on another key, hold W (only the game's own PC key layer sees it), then the pad key as a
control, and report how far the Guardian moved each time.  Usage: kbm-check.py OUTDIR [PADKEY]"""
import ctypes, importlib.util, math, os, sys, time
os.environ['D1_LAB_DRIVER'] = '1'
HERE = os.path.dirname(os.path.abspath(__file__))
s = importlib.util.spec_from_file_location('c', os.path.join(HERE, 'combat2.py')); c = importlib.util.module_from_spec(s); s.loader.exec_module(c)
out = sys.argv[1]; padkey = sys.argv[2] if len(sys.argv) > 2 else 'Y'
u = ctypes.WinDLL('user32')
def vk(k, down): u.keybd_event(ord(k), u.MapVirtualKeyW(ord(k), 0), 0 if down else 2, None)
W = c.World()
def pos(wait=1.0):
    t0 = time.time()
    while time.time() - t0 < wait:
        p = W.me()
        if p: return p[0]
        time.sleep(0.1)
t0 = time.time()
while pos() is None and time.time() - t0 < 240: time.sleep(2)
if pos() is None: sys.exit('no player unit')
c._gf.ensure(); time.sleep(4)
c.fight.grab(os.path.join(out, 'kbm-0.png'))
for name, k in (('W (game key layer)', 'W'), (f'{padkey} (pad stick-up)', padkey), ('W again', 'W')):
    a = pos(); vk(k, True); time.sleep(2.0); vk(k, False); time.sleep(0.8); b = pos()
    print(f'{name}: moved {math.dist(a[:2], b[:2]):.2f} m', flush=True)
    c.fight.grab(os.path.join(out, f'kbm-{k}.png'))
    time.sleep(1.0)
