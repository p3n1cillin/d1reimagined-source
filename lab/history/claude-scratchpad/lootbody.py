"""Encode a world-loot-drop incident body (0x80800CB6) for the AH control file.
usage: lootbody.py RECIPIENT WORLD X Y Z [KEY] [ID]  -> prints 'incident 1851 HEX'"""
import struct, subprocess, sys, zlib
O = r'F:\d1\d1preservation-main\d1-server\target\release\d1-oracle.exe'
rec, world = int(sys.argv[1], 0), int(sys.argv[2], 0)
x, y, z = map(float, sys.argv[3:6])
key = int(sys.argv[6], 0) if len(sys.argv) > 6 else 1
nid = int(sys.argv[7], 0) if len(sys.argv) > 7 else zlib.crc32(b'starter_rifle')
b = bytearray(128)
struct.pack_into('<I', b, 0, key); b[8] = 1; struct.pack_into('<Q', b, 0x10, nid)
b[0x18:0x20] = b'\xff' * 8
struct.pack_into('<Q', b, 0x40, rec)
struct.pack_into('<4f', b, 0x60, x, y, z, 1.0)
struct.pack_into('<I', b, 0x70, world)
r = subprocess.run([O, 'object', '--tag', '0x80800cb6', '--obj', b.hex()], capture_output=True, text=True)
out = r.stdout + r.stderr
hexes = [w for l in out.splitlines() for w in l.replace(':', ' ').split() if len(w) > 60 and all(c in '0123456789abcdef' for c in w)]
print(f'incident 1851 {hexes[0]}' if hexes else out[:800])
