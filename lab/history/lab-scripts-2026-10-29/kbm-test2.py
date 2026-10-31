"""emu79: the remapped keyboard layer (D1_KBM_MAP=R=E,E=E,1=R) with the keyboard-mode pad config.
Assumes the match is running and the pad config was already swapped and reloaded (F8).
Usage: kbm-test2.py OUTDIR [probe]"""
import ctypes, importlib.util, math, os, sys, time
os.environ['D1_LAB_DRIVER'] = '1'
HERE = os.path.dirname(os.path.abspath(__file__))
s = importlib.util.spec_from_file_location('c', os.path.join(HERE, 'combat2.py')); c = importlib.util.module_from_spec(s); s.loader.exec_module(c)
u = ctypes.WinDLL('user32')
EXT = {0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x2D, 0x2E}
def vk(k, d): u.keybd_event(k, u.MapVirtualKeyW(k, 0), (1 if k in EXT else 0) | (0 if d else 2), None)
def hold(k, ms): vk(k, True); time.sleep(ms / 1000); vk(k, False)
def click(): u.mouse_event(2, 0, 0, 0, 0); time.sleep(0.12); u.mouse_event(4, 0, 0, 0, 0)
W = c.World()
def pos():
    for _ in range(20):
        p = W.me()
        if p: return p[0]
        time.sleep(0.1)
out = sys.argv[1]; mode = sys.argv[2] if len(sys.argv) > 2 else 'basic'
t0 = time.time()
while pos() is None and time.time() - t0 < 240: time.sleep(2)
c._gf.ensure(); time.sleep(1)
g = lambda n: c.fight.grab(os.path.join(out, n + '.png'))
if mode == 'basic':
    a = pos(); hold(0x57, 2000); time.sleep(0.6); b = pos()
    print(f'W (engine only): {math.dist(a[:2], b[:2]):.2f} m', flush=True)
    a = pos(); vk(0x20, True); time.sleep(0.12); vk(0x20, False); time.sleep(0.35); b = pos()
    print(f'Space: rise {b[2] - a[2]:.2f} m', flush=True)
    click(); time.sleep(1.0); g('b-fired')
    hold(0x52, 150); time.sleep(3.5); g('b-after-R')
    hold(0x31, 150); time.sleep(2.0); g('b-after-1')
    hold(0x31, 150); time.sleep(2.0); g('b-after-1b')
    hold(0x54, 150); time.sleep(1.5); g('b-wave')                  # T: d-pad up (wave, third person)
    time.sleep(1.5); g('b-wave2')
else:
    for name, k in (('X', 0x58), ('C', 0x43), ('PgDn', 0x22), ('PgUp', 0x21), ('Z', 0x5A), ('F', 0x46),
                    ('V', 0x56), ('U', 0x55), ('J', 0x4A), ('K', 0x4B), ('N', 0x4E), ('M', 0x4D)):
        hold(0x41, 150); time.sleep(0.6)                           # A strafes (engine), nudge
        a = pos(); g(f'p-{name}-0'); hold(k, 150); time.sleep(0.4); g(f'p-{name}-1'); time.sleep(1.4)
        b = pos(); g(f'p-{name}-2')
        print(name, 'delta', [round(b[i] - a[i], 2) for i in range(3)] if a and b else None, flush=True)
        time.sleep(1.5)
print('done')
