"""Name a story world's mission from the 56578 packages, offline.

For each top-level activity symbol in a package (e.g. `act_cosmo_m1_dome_a`, tag 0x80CB8003),
walk the tags it references and print:
  - its phase names (`cosmo_m1_phase00_dome_a` ...), i.e. the route through the bubbles;
  - its objective objects (`obj_m1_o1a` ...);
  - every localized string library (class 0x8080035A) it reaches, decoded to English.
The Ghost lines and objective text name the mission ("Skywatch's old array stations" = The Dark
Within). Join that with the activity table to get the activity index -> world row.

String library layout (56578, little-endian): index {u64 size, u64 count, u64 rel, 12 u32 language
table tags from +0x18 (English first), hash array after a 0x80800184 header}; language table
{array @+8 of 0x20-byte parts {u64, u64 rel-from-+8, u32, u16 bytes, u16 chars, u16 shift, ...},
array @+0x48 of 0x10-byte strings {i64 rel to first part, u64 part count}}; text bytes are
stored minus `shift`. Entries come from `d1-oracle oodle entry` (the client's own decoder).

  d1-scenarios.py --pkg ps4_cosmo_activities_activity_025c_0 [--strings]
"""
import argparse, os, re, struct, subprocess, sys, tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import d1paths  # sibling: local paths from D1_ROOT or paths.json


def oracle(tool='d1-oracle'):
    return str(d1paths.workspace() / 'd1-server' / 'target' / 'release' / f'{tool}.exe')


def pkgdir():
    return str(d1paths.workspace() / 'dumps' / 'later-56578-CUSA00219' / 'CUSA00219' / 'packages')


STRING_INDEX_CLASS = 0x8080035A
ARRAY = 0x80800184
os.environ.setdefault('D1_BUILD', 'later-56578')
CACHE = os.path.join(tempfile.gettempdir(), 'd1-scenarios-cache')
os.makedirs(CACHE, exist_ok=True)
_mem = {}


def entry(tag):
    if tag in _mem:
        return _mem[tag]
    p = os.path.join(CACHE, f'{tag:#010x}.bin')
    if not os.path.exists(p):
        subprocess.run([oracle(), 'oodle', 'entry', '--tag', hex(tag), '-o', p, '--hex', '0'], capture_output=True)
    _mem[tag] = open(p, 'rb').read() if os.path.exists(p) else b''
    return _mem[tag]


def symbols(pkg):
    # The symbol count and offset are what `d1-pkg info` prints; take them from it.
    info =subprocess.run([oracle('d1-pkg'), 'info', pkg], capture_output=True, text=True).stdout
    m = re.search(r'symbols\s+(\d+)\s+@\s+(\d+)', info)
    n, off = int(m.group(1)), int(m.group(2))
    with open(os.path.join(pkgdir(), pkg + '.pkg'), 'rb') as f:
        f.seek(off)
        raw = f.read(n * 0x44)
    for i in range(n):
        r = raw[i * 0x44:(i + 1) * 0x44]
        yield struct.unpack_from('<I', r, 0)[0], struct.unpack_from('<I', r, 4)[0], r[8:].split(b'\0')[0].decode()


def closure(root, depth=5):
    seen, front = {root}, [root]
    for _ in range(depth):
        nxt = []
        for t in front:
            d = entry(t)
            for o in range(0, len(d) - 3, 4):
                r = struct.unpack_from('<I', d, o)[0]
                if (r >> 16) == (root >> 16) and r not in seen:
                    seen.add(r)
                    nxt.append(r)
        front = nxt
    return seen


def arr(d, field):
    n = struct.unpack_from('<Q', d, field)[0]
    head = field + 8 + struct.unpack_from('<Q', d, field + 8)[0]
    if struct.unpack_from('<I', d, head - 4)[0] != ARRAY or struct.unpack_from('<Q', d, head)[0] != n:
        raise ValueError(f'no array at +{field:#x}')
    return n, head + 0x10


def library(tag):
    idx = entry(tag)
    count = struct.unpack_from('<Q', idx, 8)[0]
    if not count:
        return {}
    english = struct.unpack_from('<I', idx, 0x18)[0]
    hp = idx.find(struct.pack('<IQ', ARRAY, count))
    hashes = struct.unpack_from(f'<{count}I', idx, hp + 4 + 0x10)
    t = entry(english)
    n, off = arr(t, 8)
    parts = []
    for i in range(n):
        b = off + 0x20 * i
        rel = struct.unpack_from('<Q', t, b + 8)[0]
        blen, _, shift = struct.unpack_from('<HHH', t, b + 0x14)
        parts.append(bytes((c + shift) & 0xFF for c in t[b + 8 + rel:b + 8 + rel + blen]).decode('utf-8', 'replace'))
    m, mo = arr(t, 0x48)
    out = []
    for i in range(m):
        e = mo + 0x10 * i
        rel, ln = struct.unpack_from('<qQ', t, e)
        first = (e + rel - off) // 0x20
        out.append(''.join(parts[first:first + ln]) if ln else '')
    return dict(zip(hashes, out))


def is_library(tag):
    d = entry(tag)
    return len(d) > 0x60 and d.find(struct.pack('<I', 0x80800184)) > 0 and \
        struct.unpack_from('<Q', d, 8)[0] < 4096 and (struct.unpack_from('<I', d, 0x18)[0] >> 24) == 0x80


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--pkg', default='ps4_cosmo_activities_activity_025c_0')
    ap.add_argument('--strings', action='store_true', help='print every decoded string')
    a = ap.parse_args()
    sys.stdout.reconfigure(encoding='utf-8')
    libs = {int(l.split()[3], 16) for l in subprocess.run(
        [oracle('d1-pkg'), 'entries', a.pkg], capture_output=True, text=True).stdout.splitlines()
        if len(l.split()) > 5 and l.split()[5] == f'{STRING_INDEX_CLASS:08x}'}
    for tag, cls, name in symbols(a.pkg):
        if ':' in name or cls != 0x808008BA:
            continue
        reach = closure(tag)
        names = set()
        for t in reach:
            names.update(m.decode() for m in re.findall(rb'[a-z0-9_]{6,}', entry(t)))
        phases = sorted(n for n in names if 'phase' in n and not n.startswith('obj_'))
        objs = sorted(n for n in names if n.startswith('obj_m') or n.startswith('obj_s'))
        mine = sorted(libs & reach)
        print(f'== {name} ({tag:#010x}) - {len(reach)} tags')
        print('   phases:', ' '.join(phases) or '-')
        print('   objectives:', ' '.join(objs) or '-')
        for lt in mine:
            try:
                lib = library(lt)
            except (ValueError, struct.error) as e:
                print(f'   library {lt:#010x}: unreadable ({e})')
                continue
            print(f'   library {lt:#010x}: {len(lib)} strings')
            for h, s in lib.items():
                if a.strings:
                    print(f'     {h:#010x} {s[:150]}')


if __name__ == '__main__':
    main()
