"""Print 'x,y' of the nearest live native squad member in the given squads (or nothing)."""
import importlib.util, math, os, sys
HERE = os.path.dirname(os.path.abspath(__file__))
s = importlib.util.spec_from_file_location('c', os.path.join(HERE, 'combat2.py')); c = importlib.util.module_from_spec(s); s.loader.exec_module(c)
only = {int(x) for x in sys.argv[1].split(',')}
W = c.World(); st = W.me()
if st:
    m = [x for x in W.members(st[0], 400) if x[1][2] in only]
    if m: print(f'{m[0][2][0]:.1f},{m[0][2][1]:.1f}')
