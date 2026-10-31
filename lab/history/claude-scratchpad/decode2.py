"""Decode inventory elements: level +0x18, primary stat +0x1C, ci, grid."""
import json, os, struct, subprocess, sys
from pathlib import Path
root = Path(r'F:\d1\d1preservation-main')
env = dict(os.environ, D1_BUILDS=str(root / 'capture-rig/builds.json'), D1_BUILD='later-56578')
for item in [Path(p) for p in sys.argv[1:]]:
    r = subprocess.run([str(root / 'd1-server/target/release/d1-oracle.exe'), '--json', 'object', '--tag', '0x80801732',
                        '--decode', item.read_bytes().hex()], env=env, capture_output=True, text=True, timeout=90)
    line = next((l for l in r.stdout.splitlines() if l.startswith('{')), None)
    if not line:
        print(item.name, 'decode failed'); continue
    obj = json.loads(line); raw = bytearray(obj['struct_size'])
    for off, val in obj['nonzero']:
        at = int(off, 0); raw[at:at + 8] = int(val, 0).to_bytes(8, 'little')
    u = lambda f, a: struct.unpack_from('<' + f, raw, a)[0]
    print(f'{item.name:22} soid {u("Q",0):#x} level {u("i",0x18)} +1C {u("I",0x1C)} +20 {u("h",0x20)} ci {u("H",0x24)} grid {u("H",0x58):#x} +44 {u("H",0x44):#x}')
