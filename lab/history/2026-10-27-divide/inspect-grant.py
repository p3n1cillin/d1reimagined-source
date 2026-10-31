"""Decode the owned grant with its build's codec; print only relevant field values."""
import json, os, struct, subprocess
from pathlib import Path
root = Path(r'F:\d1\d1preservation-main')
env = dict(os.environ, D1_BUILDS=str(root/'capture-rig/builds.json'), D1_BUILD='later-56578')
item = root/'capture-rig/builds/later-56578/grants/khvostov-3598.bin'
args = [str(root/'d1-server/target/release/d1-oracle.exe'), '--json', 'object',
        '--tag', '0x80801732', '--decode', item.read_bytes().hex()]
result = subprocess.run(args, env=env, capture_output=True, text=True, timeout=90)
if result.returncode:
    print('Owned item decode failed:', result.returncode, result.stderr[:1000])
    raise SystemExit(1)
Path(__file__).with_name('grant-decoded-private.txt').write_text(result.stdout)
line = next(line for line in result.stdout.splitlines() if line.startswith('{'))
obj = json.loads(line)
raw = bytearray(obj['struct_size'])
for offset, value in obj['nonzero']:
    at = int(offset, 0)
    raw[at:at+8] = int(value, 0).to_bytes(8, 'little')
def unpack(fmt, at):
    return struct.unpack_from('<'+fmt, raw, at)
print('level', unpack('i', 0x18)[0], 'equip-min', unpack('I', 0x1C)[0],
      'curve-offset', unpack('h', 0x20)[0], 'content-index', unpack('H', 0x24)[0],
      'progression', unpack('H', 0x44)[0], 'talent-grid', unpack('H', 0x58)[0])
print('stat keys', unpack('8H', 0x88), 'stat values', unpack('8I', 0x98))
print('node states nonzero', [(i,v) for i,v in enumerate(raw[0x5A:0x7E]) if v])
