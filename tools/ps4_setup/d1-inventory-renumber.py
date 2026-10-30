"""Copy staged family-4/3 inventory elements, dropping some buckets and renumbering the SOIDs.

    python -I d1-inventory-renumber.py SRC OUT "Primary Weapons,Special Weapons,Heavy Weapons"

d1-responder keys each served element as D1_ITEM_SOID_BASE + its index in filename order, while
the element itself carries its SOID at +0x00 (and the profile and peer roster built from the
directory name that one). Dropping files shifts the indices, so every kept element is re-encoded
with SOID 0xD100000000000301 + i and renamed `ii-<bucket>.bin`. Each element is decoded and
re-encoded with the client's own codec (`d1-oracle object --tag 0x80801732`); only +0x00 changes.
"""
import os, re, struct, subprocess, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import d1paths  # sibling: local paths from D1_ROOT or paths.json


def oracle():
    return str(d1paths.workspace() / 'd1-server' / 'target' / 'release' / 'd1-oracle.exe')


BASE = 0xD100000000000301
TAG = '0x80801732'


def main():
    src, out, skip = sys.argv[1], sys.argv[2], {b.strip() for b in sys.argv[3].split(',')}
    os.makedirs(out, exist_ok=True)
    files = sorted(f for f in os.listdir(src) if f.endswith('.bin'))
    keep = [f for f in files if f.split('-', 1)[1][:-4] not in skip]
    for i, f in enumerate(keep):
        raw = open(os.path.join(src, f), 'rb').read().hex()
        dec = subprocess.run([oracle(), 'object', '--tag', TAG, '--decode', raw], capture_output=True, text=True).stdout
        obj = bytearray(184)
        for m in re.finditer(r'\+0x([0-9a-f]{4})\s+0x([0-9a-f]{16})', dec):
            struct.pack_into('<Q', obj, int(m.group(1), 16), int(m.group(2), 16))
        struct.pack_into('<Q', obj, 0, BASE + i)
        enc = subprocess.run([oracle(), 'object', '--tag', TAG, '--obj', obj.hex()], capture_output=True, text=True).stdout
        payload = max(re.findall(r'[0-9a-f]{100,}', enc), key=len)
        name = f'{i:02d}-' + f.split('-', 1)[1]
        open(os.path.join(out, name), 'wb').write(bytes.fromhex(payload))
        print(f'{f} -> {name}  soid {BASE + i:#x}')


if __name__ == '__main__':
    main()
