"""Hold probe for the engine keys with no visible tap effect (D1_KBM=1, keyboard-mode pad config):
sprint test (W alone vs W with Z clicked at the start), then each key held 600 ms with grabs.
Usage: kbm-hold.py OUTDIR"""
import ctypes, importlib.util, math, os, sys, time
os.environ['D1_LAB_DRIVER'] = '1'
HERE = os.path.dirname(os.path.abspath(__file__))
s = importlib.util.spec_from_file_location('c', os.path.join(HERE, 'combat2.py')); c = importlib.util.module_from_spec(s); s.loader.exec_module(c)
u = ctypes.WinDLL('user32')
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
c._gf.ensure(); time.sleep(3)
g = lambda n: c.fight.grab(os.path.join(out, n + '.png'))
for label, extra in (('W', None), ('W+Z', 0x5A), ('W+Shift', 0xA0)):
    a = pos(); vk(0x57, True); time.sleep(0.15)
    if extra: vk(extra, True); time.sleep(0.12); vk(extra, False)
    time.sleep(1.85); vk(0x57, False); time.sleep(0.6); b = pos()
    print(f'{label}: {math.dist(a[:2], b[:2]):.2f} m', flush=True)
    vk(0x53, True); time.sleep(1.9); vk(0x53, False); time.sleep(0.6)    # walk back (S)
for name, k in (('F', 0x46), ('V', 0x56), ('U', 0x55), ('J', 0x4A), ('K', 0x4B), ('N', 0x4E), ('M', 0x4D), ('Z', 0x5A), ('PgUp', 0x21)):
    vk(0x41, True); time.sleep(0.15); vk(0x41, False); time.sleep(0.8)   # A nudge
    g(f'h-{name}-0'); vk(k, True); time.sleep(0.35); g(f'h-{name}-1'); time.sleep(0.25); vk(k, False)
    time.sleep(0.6); g(f'h-{name}-2'); time.sleep(2.0)
    print(name, 'held', flush=True)
print('done')
