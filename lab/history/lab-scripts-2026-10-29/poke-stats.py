"""Diagnostic: write manifest stats (Attack idx 11 = 5, Magazine idx 20 = 25) into the live rifle instance."""
import importlib.util, struct
spec = importlib.util.spec_from_file_location('g', r'F:\d1\public-work\d1reimagined-source\tools\ps4_setup\d1-winguest.py')
g = importlib.util.module_from_spec(spec); spec.loader.exec_module(g)
p, base = g.open_client()
a = 0x37f910be8
assert struct.unpack('<H', p.read(a + 0x24, 2))[0] == 3598 and struct.unpack('<H', p.read(a + 0x58, 2))[0] == 279
p.write(a + 0x88, struct.pack('<8H', 11, 20, *([0xFFFF] * 6)))
p.write(a + 0x98, struct.pack('<8I', 5, 25, 0, 0, 0, 0, 0, 0))
print('keys', struct.unpack('<8H', p.read(a + 0x88, 16)), 'values', struct.unpack('<8I', p.read(a + 0x98, 32)))
