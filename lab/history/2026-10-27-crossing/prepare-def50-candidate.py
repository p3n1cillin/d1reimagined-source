"""Prepare the def+0x50 rifle-grant candidate: grid 279 plus the first eight entries of the definition's own stat list.

The Khvostov definition (package entry 0x80A64722, reached from the item table at content index 3598) carries at
def+0x50 a self-relative record of 13 {u32 stat index, u32 value} pairs. The manifest's 13 stats for the 7G-02 match
them as investment values (Magazine 44 shows as 25 through the stat group). On 56578 def+0x68 is NULL, so the stat
list a native constructor copies into +0x88 (indices) / +0x98 (values) is probably this one. Values are read from
the package, not typed in. No file is served by this script.
"""
import hashlib, json, os, re, struct, subprocess
from pathlib import Path
root = Path(__file__).parent
oracle = r'F:\d1\d1preservation-main\d1-server\target\release\d1-oracle.exe'
env = dict(os.environ, D1_BUILDS=r'F:\d1\d1preservation-main\capture-rig\builds.json', D1_BUILD='later-56578')
source = Path(r'F:\d1\d1preservation-main\capture-rig\builds\later-56578\grants\khvostov-3598.bin')

d = (root / 'khvostov-def-80a64722.bin').read_bytes()
rec = 0x50 + struct.unpack_from('<q', d, 0x50)[0]
count, rel = struct.unpack_from('<qq', d, rec)
def50 = [struct.unpack_from('<II', d, rec + 8 + rel + 0x10 + 8 * i) for i in range(count)]
assert count == 13 and def50[0] == (11, 0) and def50[7] == (20, 44), def50
keys, values = [k for k, _ in def50[:8]], [v for _, v in def50[:8]]


def command(args):
    p = subprocess.run([oracle, *args], env=env, capture_output=True, text=True, timeout=90)
    if p.returncode:
        raise RuntimeError('client codec failed: ' + p.stderr[:300])
    return p.stdout


text = command(['--json', 'object', '--tag', '0x80801732', '--decode', source.read_bytes().hex()])
obj = json.loads(next(l for l in text.splitlines() if l.startswith('{')))
raw = bytearray(obj['struct_size'])
for off, value in obj['nonzero']:
    o = int(off, 0); raw[o:o + 8] = int(value, 0).to_bytes(8, 'little')
assert struct.unpack_from('<H', raw, 0x24)[0] == 3598 and struct.unpack_from('<H', raw, 0x58)[0] == 0xFFFF
out = {}
for candidate in (False, True):
    body = bytearray(raw)
    if candidate:
        struct.pack_into('<H', body, 0x58, 279)
        struct.pack_into('<8H', body, 0x88, *keys); struct.pack_into('<8I', body, 0x98, *values)
    result = command(['object', '--tag', '0x80801732', '--obj', body.hex()])
    assert 'object IDENTICAL' in result, result[-300:]
    wire = bytes.fromhex(re.search(r'payload (\d+) bytes / (\d+) bits\s+([0-9a-f]+)', result)[3])
    if not candidate:
        assert wire == source.read_bytes(), 'baseline round trip did not reproduce the served grant'
        continue
    path = root / 'grant-def50-candidate.bin'
    path.write_bytes(wire)
    out = {'source_sha256': hashlib.sha256(source.read_bytes()).hexdigest(), 'def50': def50,
           'changed': '+0x58 grid 279; +0x88/+0x98 = def+0x50 entries 0-7', 'keys': keys, 'values': values,
           'bytes': len(wire), 'sha256': hashlib.sha256(wire).hexdigest(), 'file': str(path)}
print(json.dumps(out))
