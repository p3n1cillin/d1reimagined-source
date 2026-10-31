"""Third-person check in the Tower: d-pad up (wave) and d-pad down (sit) through the pad config's
arrow keys, with a stick nudge first. Grabs a frame during each emote.  Usage: tower-emote.py OUTDIR"""
import ctypes, importlib.util, os, sys, time
os.environ['D1_LAB_DRIVER'] = '1'
HERE = os.path.dirname(os.path.abspath(__file__))
s = importlib.util.spec_from_file_location('c', os.path.join(HERE, 'combat2.py')); c = importlib.util.module_from_spec(s); s.loader.exec_module(c)
u = ctypes.WinDLL('user32')
def tap(vk, ms=150):
    u.keybd_event(vk, u.MapVirtualKeyW(vk, 0), 1, None); time.sleep(ms / 1000); u.keybd_event(vk, u.MapVirtualKeyW(vk, 0), 3, None)
out = sys.argv[1]; c._gf.ensure(); time.sleep(0.5)
c.hold('S', 600); time.sleep(1.0)                 # stick nudge, step back from walls
for name, vk in (('wave', 0x26), ('sit', 0x28)):  # VK_UP, VK_DOWN (extended keys)
    tap(vk); time.sleep(1.5); c.fight.grab(os.path.join(out, f'tower-{name}-a.png'))
    time.sleep(1.5); c.fight.grab(os.path.join(out, f'tower-{name}-b.png'))
    time.sleep(3.0); c.hold('A', 120); time.sleep(1.0)
print('done')
