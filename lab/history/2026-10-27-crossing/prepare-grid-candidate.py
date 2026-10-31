"""Prepare a one-variable rifle-grant candidate: talent grid 0xFFFF -> 279 (the Khvostov 7G-02's own grid).

The manifest gives the Khvostov 7G-02 (0x1CBA8A98) talent grid 4247453613 (0xFD2AFFAD). The investment
globals talent-grid table (entry 0x80A5E227, class 0x808018C2: entries from +0x30, 24 bytes each, as the
item table that puts this item at content index 3598) has that hash at entry 279. Nothing else changes.
No file is served by this script.
"""
import hashlib, json, os, re, struct, subprocess
from pathlib import Path
root = Path(__file__).parent
oracle = r'F:\d1\d1preservation-main\d1-server\target\release\d1-oracle.exe'
env = dict(os.environ, D1_BUILDS=r'F:\d1\d1preservation-main\capture-rig\builds.json', D1_BUILD='later-56578')
source = Path(r'F:\d1\d1preservation-main\capture-rig\builds\later-56578\grants\khvostov-3598.bin')


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
for grid in (0xFFFF, 279):
    body = bytearray(raw); struct.pack_into('<H', body, 0x58, grid)
    result = command(['object', '--tag', '0x80801732', '--obj', body.hex()])
    assert 'object IDENTICAL' in result, result[-300:]
    m = re.search(r'payload (\d+) bytes / (\d+) bits\s+([0-9a-f]+)', result)
    wire = bytes.fromhex(m[3])
    if grid == 0xFFFF:
        assert wire == source.read_bytes(), 'baseline round trip did not reproduce the served grant'
        continue
    path = root / 'grant-grid279-candidate.bin'
    path.write_bytes(wire)
    out = {'source_sha256': hashlib.sha256(source.read_bytes()).hexdigest(), 'changed': '+0x58 u16 talent grid 0xFFFF -> 279',
           'bytes': len(wire), 'sha256': hashlib.sha256(wire).hexdigest(), 'file': str(path)}
print(json.dumps(out))
