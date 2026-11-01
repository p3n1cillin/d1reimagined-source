"""Print the Guardian's position as 'x,y,z' (read-only memory, no input). where.py"""
import importlib.util, os

HERE = os.path.dirname(os.path.abspath(__file__))
s = importlib.util.spec_from_file_location('c', os.path.join(HERE, 'combat2.py')); c = importlib.util.module_from_spec(s); s.loader.exec_module(c)
st = c.World().me()
if st:
    print(','.join(f'{v:.1f}' for v in st[0]))
