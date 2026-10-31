"""Decode staged inventory elements with the build's own codec; print level, content index, grid and stats."""
import glob, json, os, struct, subprocess, sys
from pathlib import Path
root = Path(r'F:\d1\d1preservation-main')
env = dict(os.environ, D1_BUILDS=str(root / 'capture-rig/builds.json'), D1_BUILD='later-56578')
for item in [Path(p) for p in sys.argv[1:]]:
    r = subprocess.run([str(root / 'd1-server/target/release/d1-oracle.exe'), '--json', 'object', '--tag', '0x80801732',
                        '--decode', item.read_bytes().hex()], env=env, capture_output=True, text=True, timeout=90)
    line = next((l for l in r.stdout.splitlines() if l.startswith('{')), None)
    if r.returncode or not line:
        print(item.name, 'decode failed', r.returncode, r.stderr[:200]); continue
    obj = json.loads(line)
    raw = bytearray(obj['struct_size'])
    for off, val in obj['nonzero']:
        at = int(off, 0); raw[at:at + 8] = int(val, 0).to_bytes(8, 'little')
    u = lambda f, a: struct.unpack_from('<' + f, raw, a)
    print(f'{item.name}: level {u("i", 0x18)[0]} ci {u("H", 0x24)[0]} grid {u("H", 0x58)[0]:#x} '
          f'stat keys {u("8H", 0x88)} values {u("8I", 0x98)}')
