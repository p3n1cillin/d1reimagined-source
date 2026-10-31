"""Click through menus with the PC mouse (D1_MOUSE_CURSOR + D1_MOUSE_BUTTONS): for each X,Y (screen pixels of
the 1936x1119 capture) push the game cursor into the top-left corner, move it to X,Y, left-click, grab.
Usage: mouse-nav.py OUTDIR X,Y [X,Y ...]"""
import ctypes, importlib.util, os, sys, time
os.environ['D1_LAB_DRIVER'] = '1'
HERE = os.path.dirname(os.path.abspath(__file__))
s = importlib.util.spec_from_file_location('c', os.path.join(HERE, 'combat2.py')); c = importlib.util.module_from_spec(s); s.loader.exec_module(c)
u = ctypes.WinDLL('user32')
class P(ctypes.Structure): _fields_ = [('x', ctypes.c_long), ('y', ctypes.c_long)]
def move(dx, dy, steps=25):
    for i in range(steps):
        p = P(); u.GetCursorPos(ctypes.byref(p)); u.SetCursorPos(p.x + dx // steps, p.y + dy // steps); time.sleep(0.03)
def goto(x, y, steps=30):
    p = P(); u.GetCursorPos(ctypes.byref(p))
    for i in range(1, steps + 1):
        u.SetCursorPos(p.x + (x - p.x) * i // steps, p.y + (y - p.y) * i // steps); time.sleep(0.03)
out = sys.argv[1]
c._gf.ensure(); time.sleep(0.5)
for n, xy in enumerate(sys.argv[2:]):
    x, y = (int(v) for v in xy.split(','))
    # the detour turns host cursor deltas into game cursor moves; stay on the host screen so no delta is clipped
    u.SetCursorPos(1900, 1060); time.sleep(0.3)    # game cursor clamps bottom-right
    goto(5, 5); time.sleep(0.3)                   # ...then top-left
    goto(5 + x, 5 + y); time.sleep(0.6)
    c.fight.grab(os.path.join(out, f'nav-{n}-hover.png'))
    u.mouse_event(0x0002, 0, 0, 0, 0); time.sleep(0.15); u.mouse_event(0x0004, 0, 0, 0, 0)
    time.sleep(3.5)
    c.fight.grab(os.path.join(out, f'nav-{n}-after.png'))
    print('clicked', xy, flush=True)
