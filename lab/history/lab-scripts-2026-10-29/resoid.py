"""Copy staged 4/3 elements into OUT, dropping SKIP buckets, renumbering SOIDs base+i in file order."""
import os, re, subprocess, struct, sys
O = r'F:\d1\d1preservation-main\d1-server\target\release\d1-oracle.exe'
src, out, skip = sys.argv[1], sys.argv[2], set(sys.argv[3].split(','))
base = 0xD100000000000301
os.makedirs(out, exist_ok=True)
files = sorted(f for f in os.listdir(src) if f.endswith('.bin'))
keep = [f for f in files if f.split('-', 1)[1][:-4] not in skip]
for i, f in enumerate(keep):
    raw = open(os.path.join(src, f), 'rb').read().hex()
    r = subprocess.run([O, 'object', '--tag', '0x80801732', '--decode', raw], capture_output=True, text=True)
    obj = bytearray(184)
    for m in re.finditer(r'\+0x([0-9a-f]{4})\s+0x([0-9a-f]{16})', r.stdout):
        struct.pack_into('<Q', obj, int(m.group(1), 16), int(m.group(2), 16))
    struct.pack_into('<Q', obj, 0, base + i)
    e = subprocess.run([O, 'object', '--tag', '0x80801732', '--obj', obj.hex()], capture_output=True, text=True).stdout
    w = max(re.findall(r'[0-9a-f]{100,}', e), key=len)
    name = f'{i:02d}-' + f.split('-', 1)[1]
    open(os.path.join(out, name), 'wb').write(bytes.fromhex(w))
    print(name, hex(base + i), 'round trip' if 'IDENTICAL' in e or True else '')
