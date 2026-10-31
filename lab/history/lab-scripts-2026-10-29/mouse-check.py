"""Moves the host cursor while the game window has focus and grabs the screen before and after, to see the
menu cursor follow (D1_MOUSE_CURSOR). Usage: mouse-check.py OUTDIR [dx dy]"""
import ctypes, importlib.util, os, sys, time
os.environ['D1_LAB_DRIVER'] = '1'
HERE = os.path.dirname(os.path.abspath(__file__))
s = importlib.util.spec_from_file_location('c', os.path.join(HERE, 'combat2.py')); c = importlib.util.module_from_spec(s); s.loader.exec_module(c)
out = sys.argv[1]; dx = int(sys.argv[2]) if len(sys.argv) > 2 else 300; dy = int(sys.argv[3]) if len(sys.argv) > 3 else 150
u = ctypes.WinDLL('user32')
class P(ctypes.Structure): _fields_ = [('x', ctypes.c_long), ('y', ctypes.c_long)]
c._gf.ensure(); time.sleep(0.5)
p = P(); u.GetCursorPos(ctypes.byref(p)); print('cursor at', p.x, p.y)
c.fight.grab(os.path.join(out, 'mouse-before.png'))
for i in range(20):                                   # a smooth move over ~0.6 s
    u.SetCursorPos(p.x + dx * (i + 1) // 20, p.y + dy * (i + 1) // 20); time.sleep(0.03)
time.sleep(0.5)
c.fight.grab(os.path.join(out, 'mouse-after.png'))
u.SetCursorPos(p.x, p.y)
print('moved by', dx, dy)
