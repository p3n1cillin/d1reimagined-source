"""Lab vendor agent: turns the client's purchases (investment 901) into the account and profile changes a
server would make, and pushes them through the responder's D1_ACCOUNT_PUSH_FILE / D1_PROFILE_PUSH_FILE.

  vendor-agent.py --log <responder.log> --staging <cfw-staging> --profile-obj <decoded profile C object>
                  [--account SLOT:CONTENT:QTY ...] [--state state.json]

Per purchase: decode the 901 request {u8 vendor, i16 sale, u64} with the client's codec, look the sale up in
vendor-sales.tsv (d1-oracle vendor-sales write), deduct each cost from the slot of the cost item's own bucket
(item definition +0x88, the same rule as the client's price check 0x26D6E0), and put the item in a free slot of
its bucket (an instance with a new SOID when definition +0x8B is set). Then push the account, then the profile
with the new item as its .item/.soid sidecar.
"""
import argparse, json, os, re, struct, subprocess, sys, time

ORACLE = r'F:\d1\public-work\target-claude\release\d1-oracle.exe'
ROOT = r'F:\d1\d1preservation-main'
ENV = dict(os.environ, D1_BUILDS=ROOT + r'\capture-rig\builds.json', D1_BUILD='later-56578',
           D1_PKGDIR=ROOT + r'\dumps\later-56578-CUSA00219\CUSA00219\packages')
BUILD = ROOT + r'\capture-rig\builds\later-56578'
HERE = os.path.dirname(os.path.abspath(__file__))
ACCT, CHR, PROF = 0xD100000000000001, 0xD100000000000101, 0xD100000000000201
# The client's bucket table (investment_globals->vfunc[0x98], read live in sense69): ordinal -> (first, n, scope)
BUCKETS = {0x05: (0, 20, 0), 0x01: (20, 20, 0), 0x06: (40, 3, 0), 0x07: (43, 10, 0), 0x08: (53, 10, 0), 0x09: (63, 10, 0),
           0x0a: (73, 10, 0), 0x0b: (83, 10, 0), 0x02: (93, 10, 0), 0x03: (103, 10, 0), 0x04: (113, 10, 0), 0x0c: (123, 10, 0),
           0x0f: (133, 10, 0), 0x0d: (143, 10, 0), 0x0e: (153, 10, 0), 0x11: (163, 5, 0), 0x12: (168, 1, 0), 0x10: (169, 20, 0),
           0x14: (189, 20, 0), 0x15: (0, 20, 1), 0x16: (20, 108, 1), 0x17: (128, 72, 1), 0x18: (200, 108, 1), 0x13: (209, 1, 0),
           0x19: (210, 10, 0), 0x1b: (220, 10, 0), 0x1c: (230, 10, 0), 0x1e: (240, 10, 0), 0x1d: (250, 12, 0), 0x1a: (262, 16, 0),
           0x1f: (308, 1, 1), 0x20: (278, 1, 0), 0x21: (279, 1, 0), 0x22: (309, 1, 1), 0x23: (310, 1, 1), 0x24: (280, 10, 0),
           0x25: (290, 32, 0), 0x26: (311, 1, 1), 0x27: (312, 6, 1)}


def run(*a):
    r = subprocess.run([ORACLE, *a], env=ENV, capture_output=True, text=True, timeout=600)
    if r.returncode:
        raise RuntimeError(f'oracle {a[:4]}: {r.stdout[-400:]} {r.stderr[-400:]}')
    return r.stdout


def load_tags():
    tags = {}
    for line in open(BUILD + r'\item-content-index.tsv', encoding='utf8'):
        if line[0] in '#i':
            continue
        h, ci, tag = line.strip().split('\t')
        tags[int(ci)] = int(tag, 16)
    return tags


def load_sales():
    """{(vendor, sale): (content, [(content, qty)])}; also fills DEFS from the bucket columns when present."""
    sales = {}
    for line in open(BUILD + r'\vendor-sales.tsv', encoding='utf8'):
        if line[0] == '#' or line.startswith('vendor'):
            continue
        f = line.rstrip('\n').split('\t')
        if len(f) == 7:                                   # vendor sale hash content bucket instanced costs
            v, s, _, ci, bucket, inst, costs = f
            if bucket != '-':
                DEFS[int(ci)] = (int(bucket, 16), inst == '1')
        else:
            v, s, _, ci, costs = f
        cs = []
        for c in ([] if costs == '-' else costs.split(',')):
            p = c.split(':')
            cs.append((int(p[0]), int(p[1])))
            if len(p) == 3 and p[2] != '-' and int(p[0]) not in DEFS:
                DEFS[int(p[0])] = (int(p[2], 16), False)
        sales[(int(v), int(s))] = (int(ci), cs)
    return sales


TAGS, DEFS = None, {}


def definition(ci):
    """(bucket ordinal, instanced) from the item definition's +0x88 / +0x8B."""
    if ci not in DEFS:
        out = run('oodle', 'entry', '--tag', hex(TAGS[ci]), '--hex', '200')
        b = bytes.fromhex(next(l for l in out.splitlines() if l.startswith('head:')).split()[1])
        DEFS[ci] = (b[0x88], b[0x8b] != 0)
    return DEFS[ci]


def decode_request(hexs):
    out = run('object', '--tag', '0x80801777', '--decode', hexs)
    words = {int(m[1], 16): int(m[2], 16) for m in re.finditer(r'\+0x([0-9a-f]+)\s+0x([0-9a-f]+)', out)}
    w0 = words.get(0, 0)
    return w0 & 0xff, (w0 >> 16) & 0xffff, words.get(8, 0)


def sub_bits(tag, obj):
    src, out = os.path.join(HERE, f'agent-{tag}.obj'), os.path.join(HERE, f'agent-{tag}.bin')
    open(src, 'wb').write(obj)
    if os.path.exists(out):
        os.remove(out)
    o = run('object', '--tag', tag, '--obj-file', src, '--out', out)
    if 'IDENTICAL' not in o:
        raise RuntimeError(f'{tag} round trip failed')
    n = int(re.search(r'/ (\d+) bits', o).group(1))
    data = open(out, 'rb').read()
    return [(data[i // 8] >> (7 - i % 8)) & 1 for i in range(n)]


def encode_account(slots):
    """Fields [0] [1] [3] as d1_bap::account_rsat, plus [6]; everything else ABSENT (see build-account2.py)."""
    bits = []
    put = lambda v, n: bits.extend((v >> i) & 1 for i in range(n - 1, -1, -1))
    put(1, 1); put(ACCT, 64); put(1, 1); put(1, 4); put(CHR, 64); put(0, 1); put(1, 1); put(PROF, 64); put(0, 1); put(0, 1)
    inv = bytearray(10440)
    for s in range(326):
        inv[8 + 32 * s:8 + 32 * s + 2] = b'\xff\xff'
    for s, (ci, q) in slots.items():
        e = 8 + 32 * s
        inv[e:e + 2] = ci.to_bytes(2, 'little'); inv[e + 0x10:e + 0x14] = q.to_bytes(4, 'little')
    put(1, 1); bits.extend(sub_bits('0x80800fe6', bytes(inv)))
    bits.extend([0] * (25 - 7))
    while len(bits) % 8:
        bits.append(0)
    return bytes(int(''.join(map(str, bits[i:i + 8])), 2) for i in range(0, len(bits), 8))


def encode(tag, obj, name):
    src, out = os.path.join(HERE, name + '.obj'), os.path.join(HERE, name + '.bin')
    open(src, 'wb').write(obj)
    if os.path.exists(out):
        os.remove(out)
    if 'IDENTICAL' not in run('object', '--tag', tag, '--obj-file', src, '--out', out):
        raise RuntimeError(f'{name}: round trip failed')
    return open(out, 'rb').read()


def wait_sent(path, limit=60):
    t = time.time()
    while time.time() - t < limit:
        if not os.path.exists(path):
            return True
        time.sleep(0.5)
    return False


def main():
    global TAGS
    ap = argparse.ArgumentParser()
    ap.add_argument('--log', required=True); ap.add_argument('--staging', required=True)
    ap.add_argument('--profile-obj', required=True); ap.add_argument('--item-template', default=os.path.join(HERE, 'khv.obj'))
    ap.add_argument('--account', nargs='*', default=[]); ap.add_argument('--state', default=os.path.join(HERE, 'agent-state.json'))
    ap.add_argument('--from-start', action='store_true')
    a = ap.parse_args()
    TAGS, sales = load_tags(), load_sales()
    if os.path.exists(a.state):
        st = json.load(open(a.state))
        account = {int(k): tuple(v) for k, v in st['account'].items()}; next_soid = st['next_soid']
    else:
        account = {int(s): (int(c), int(q)) for s, c, q in (x.split(':') for x in a.account)}; next_soid = 0xD100000000000601
    profile = bytearray(open(st['profile_obj'] if os.path.exists(a.state) else a.profile_obj, 'rb').read())
    template = open(a.item_template, 'rb').read()
    log = open(a.log, encoding='utf8', errors='replace')
    if not a.from_start:
        log.seek(0, 2)
    print(f'agent: {len(sales)} sales, account {account}', flush=True)
    while True:
        line = log.readline()
        if not line:
            time.sleep(0.3); continue
        m = re.search(r'VENDOR BUY \(901\) \(\d+B request\): ([0-9a-f]+)', line)
        if not m:
            continue
        vendor, sale, extra = decode_request(m.group(1))
        item, costs = sales.get((vendor, sale), (None, None))
        print(f'{time.strftime("%H:%M:%S")} buy vendor {vendor} sale {sale} (u64 {extra:#x}) -> item {item} costs {costs}', flush=True)
        if item is None:
            continue
        ok = True
        for ci, q in costs:
            b, _ = definition(ci); first, n, scope = BUCKETS[b]
            slot = next((s for s in range(first, first + n) if scope == 1 and account.get(s, (0xffff, 0))[0] == ci), None)
            if slot is None or account[slot][1] < q:
                print(f'  cannot pay {q} of {ci} (bucket {b:#x}, scope {scope})', flush=True); ok = False; break
            account[slot] = (ci, account[slot][1] - q)
        if not ok:
            continue
        b, instanced = definition(item); first, n, scope = BUCKETS[b]
        sidecar = None
        if scope == 0:
            free = next((s for s in range(first, first + n) if struct.unpack_from('<H', profile, 0x38 + 32 * s)[0] == 0xffff), None)
            if free is None:
                print(f'  no free slot in bucket {b:#x}', flush=True); continue
            e = 0x38 + 32 * free; soid = 0
            if instanced:
                soid = next_soid; next_soid += 1
                it = bytearray(template); struct.pack_into('<Q', it, 0, soid); struct.pack_into('<H', it, 0x24, item)
                sidecar = (soid, encode('0x80801732', bytes(it), 'agent-item'))
            struct.pack_into('<H', profile, e, item); struct.pack_into('<Q', profile, e + 8, soid); struct.pack_into('<I', profile, e + 0x10, 1)
        else:
            free = next((s for s in range(first, first + n) if s not in account), None)
            account[free] = (item, account.get(free, (item, 0))[1] + 1)
        acct = encode_account(account)
        prof = encode('0x80801743', bytes(profile), 'agent-profile')
        pa, pp = os.path.join(a.staging, 'account-push.bin'), os.path.join(a.staging, 'profile-push.bin')
        open(pa, 'wb').write(acct); wait_sent(pa)
        if sidecar:
            open(os.path.join(a.staging, 'profile-push.item'), 'wb').write(sidecar[1])
            open(os.path.join(a.staging, 'profile-push.soid'), 'w').write(hex(sidecar[0]))
        open(pp, 'wb').write(prof); wait_sent(pp)
        open(os.path.join(HERE, 'agent-profile-current.obj'), 'wb').write(profile)
        json.dump({'account': {str(k): v for k, v in account.items()}, 'next_soid': next_soid,
                   'profile_obj': os.path.join(HERE, 'agent-profile-current.obj')}, open(a.state, 'w'))
        print(f'  applied: account {account}; item {item} in bucket {b:#x}' + (f' as {sidecar[0]:#x}' if sidecar else ''), flush=True)


if __name__ == '__main__':
    main()
