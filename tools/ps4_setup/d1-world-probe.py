#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
"""d1-world-probe - read-only views of the live 56578 client's world state, from guest memory.

    d1-world-probe.py player
    d1-world-probe.py navpoints     [--near M] [--json]
    d1-world-probe.py interactables [--near M] [--ref 0xHASH/0xKIND/INDEX] [--json]
    d1-world-probe.py triggers      [--near M] [--ref 0xHASH/0xKIND/INDEX] [--json]
    d1-world-probe.py row --ref 0xHASH/0xKIND/INDEX [--json]
    d1-world-probe.py flags         [--json]

Needs one running shadPS4 on Windows. It only reads: it never writes guest memory, suspends the
process or sends keys. Every read is guarded and prints "unreadable" instead of crashing. With no
shadPS4 it exits 1 with a message. All addresses belong to build 56578 (eboot-relative; the load base
comes from d1-winguest.open_client). Positions are world units; --near keeps items within M of the
local Guardian (navpoints compare in the ground plane, the rest in 3D).

  player        the local Guardian's unit handle, address and position (d1_unit_tracking).
  navpoints     content rows of kind 0x2c (objective nav points) and the position each points at.
  interactables the client's "interactable component list": radius and position per node.
  triggers      the "trigger volume components" list: row ref, and the world box from the shape record.
  row           the replica entry of one bundle row (bundle table record, entry fields, value buffer).
  flags         the local player's flag block (count and hashes).
"""
import argparse
import contextlib
import importlib.util
import json
import math
import struct
import subprocess
import sys
from pathlib import Path

M32, M64 = 0xFFFFFFFF, (1 << 64) - 1
HERE = Path(__file__).resolve().parent

# Build 56578 addresses, eboot-relative. Each was found by reading the client's own code in Ghidra
# (decompile of the function named) and checked against the live client on 13-14 Nov.
# Handle registry: global holding a pointer to the registry; slot records are 0x40 bytes
# (data +0x10, stride +0x30, mask +0x34). Found by the client's handle-to-object walk; verified live
# in the restoration lab runs (rst16-rst26) on 13-14 Nov.
REGISTRY = 0x1A238A0
# Content table (nav points): d1-winguest's LT_COUNT/LT_TABLE walk, kind-0x2c row index at +0x2BD1C,
# dx table at +0x2A51C, 0x38-byte rows at +0x510. Found from an earlier namespace dump of the kind-0x2c
# rows; verified live in rst16 and rst19 on 13-14 Nov.
CONTENT_KIND_INDEX, CONTENT_DX_TABLE, CONTENT_ROWS, ROW_STRIDE = 0x2BD1C, 0x2A51C, 0x510, 0x38
NAV_KIND, NAV_POS_OFF = 0x2C, 0x20
# Interactable component list cell. Found from the list allocator/add/head functions
# (FUN_0064add0/0064afa0/0064afe0) and the node walk FUN_003be450; verified live in rst20 on 13-14 Nov.
# The list node sits at component+0x240; radius f32 at node+0x50, position f32x3 at node+0x60.
INTERACTABLE_LIST_CELL = 0x26BEBC0
INTERACTABLE_NODE_IN_COMP, INTERACTABLE_REF_IN_COMP = 0x240, 0x2EC
# Trigger volume list cell. Found from c_trigger_volume_game_system's init slot (FUN_001147C0) and the
# component add (FUN_00114F40: node = component+0x68); verified live in the dark-within lab run dw9 on
# 13-14 Nov. Component: own handle +0x24, geometry handle +0x50, shape index +0x58, row ref +0x5C.
# Shape table = G + i32[G+0x1C] + i32[G + 0x54 + i32[G+0x18]*0x30] with G = resolve(geometry handle)
# (FUN_00243EE0). World box in the shape record: half-extents f32x3 at +0x40, centre f32x3 at +0x50
# (w = 1.0 at +0x5C). The entity-transform formula read zeros in dw9, so the shape box is the position.
TRIGGER_LIST_CELL, TRIGGER_NODE_IN_COMP = 0x25D7160, 0x68
SHAPE_BOX_OFF = 0x40
# Replica entries: bundle pool index word, bundle table (bias at +0, stride at +0x10, record at +0x30
# of the pool slot), per-bundle entries (0x14 bytes at +0x14, base row index at +0x10) and row records
# (0x38 bytes from +0x520). Entry E: key +0, owner +0x60, pending +0x15, flag +0x16, dirty +0x66,
# value descriptors +0x8/+0xC, value buffer from FUN_002cfcd0. Found from FUN_001071d0 and FUN_002cfcd0;
# verified live in rst23 and rst24 on 13-14 Nov.
BUNDLE_POOL_INDEX, BUNDLE_TABLE, BUNDLE_STRIDE, BUNDLE_BUFFERS = 0x25D6FF8, 0x1AD5000, 0x1AD5010, 0x1AD5030
ROW_RECORDS_OFF, ENTRY_HANDLE_OFF = 0x520, 0x1A8
# Player flag block: player id array, player table, and the entity's flag image (count +0x264, hashes
# from +0x268). Found from the flag-set functions; verified live in rst23 and rst24 on 13-14 Nov.
PLAYER_IDS_PTR, PLAYER_TABLE = 0x2689600, 0x2689580
FLAG_COUNT_OFF, FLAG_HASHES_OFF, FLAG_HASHES_MAX = 0x264, 0x268, 8

WALK_LIMIT = 4096


# ---- pure helpers (unit-tested with synthetic bytes) ----------------------------------------------

def parse_ref(text):
    """'0xHASH/0xKIND/INDEX' -> (hash, kind, index). Raises ValueError on anything else."""
    parts = text.split('/')
    if len(parts) != 3:
        raise ValueError(f'ref must be HASH/KIND/INDEX, got {text!r}')
    h, k, i = (int(x, 0) for x in parts)
    if not (0 <= h <= M32 and 0 <= k <= 0xFF and -0x8000 <= i <= 0x7FFF):
        raise ValueError(f'ref out of range: {text!r}')
    return h, k, i


def decode_ref(raw):
    """8 bytes {u32 hash @0, u8 kind @4, i16 index @6} -> (hash, kind, index)."""
    h, k, i = struct.unpack('<IBxh', raw[:8])
    return h, k, i


def fmt_ref(ref):
    return f'{ref[0]:#010x}/{ref[1]:#x}/{ref[2]}'


def handle_slot(h):
    """Registry slot of a handle: the client's (h >> 13) arithmetic shift, folded with 0x3FF."""
    esi = struct.unpack('<i', struct.pack('<I', h & M32))[0] >> 13
    return (((esi & M32) >> 0x12) | 0x3FF) & esi & M32


def handle_index(h):
    return h & 0x1FFF


def object_address(entry, mask, entry_q8):
    """Object address from a registry entry: entry - (mask & u64 at entry+8), wrapped to 64 bits."""
    return (entry - (mask & entry_q8)) & M64


def decode_shape_box(raw):
    """0x20 bytes read at shape+0x40 -> (half_extents, centre, w): f32x3 at +0, f32x3 at +0x10, f32 at +0x1C."""
    if len(raw) < 0x20:
        raise ValueError('need 0x20 bytes')
    half = struct.unpack_from('<3f', raw, 0)
    centre = struct.unpack_from('<3f', raw, 0x10)
    w = struct.unpack_from('<f', raw, 0x1C)[0]
    return half, centre, w


def decode_nav_row(row):
    """A 0x38-byte kind-0x2c content row -> (source tag u32, source offset u64) from +0x28 / +0x30."""
    st, so = struct.unpack_from('<QQ', row, 0x28)
    return st & M32, so


def decode_nav_position(rec):
    """Bytes read at the nav record's +0x20 (12 bytes) -> f32x3."""
    return struct.unpack('<3f', rec[:12])


def finite(v):
    return all(math.isfinite(x) for x in v)


def distance(a, b, planar=False):
    if a is None or b is None or not finite(a) or not finite(b):
        return None
    return math.dist(a[:2], b[:2]) if planar else math.dist(a, b)


def words(b, n):
    """Hex dump of the first n u32 words of b, or 'unreadable'."""
    if b is None or len(b) < 4 * n:
        return 'unreadable'
    return ' '.join(f'{w:08x}' for w in struct.unpack_from(f'<{n}I', b))


def hx(v):
    return 'unreadable' if v is None else hex(v)


def rnd(v, n=2):
    return None if v is None else [round(x, n) for x in v]


# ---- guarded memory --------------------------------------------------------------------------------

class Mem:
    """Wraps d1-winguest's Proc: u()/read() return None instead of raising."""

    def __init__(self, p, base):
        self.p, self.base = p, base

    def u(self, addr, fmt):
        try:
            return self.p.u(addr, fmt)
        except Exception:
            return None

    def read(self, addr, n):
        try:
            return self.p.read(addr, n)
        except Exception:
            return None

    def f3(self, addr):
        b = self.read(addr, 12)
        return struct.unpack('<3f', b) if b else None

    def resolve(self, h):
        """Registry handle -> object address, or None."""
        if h is None or h == M32:
            return None
        cell = self.u(self.base + REGISTRY, 'Q')
        reg = self.u(cell, 'Q') if cell else None
        if not reg:
            return None
        rec = reg + handle_slot(h) * 0x40
        data, stride, mask = self.u(rec + 0x10, 'Q'), self.u(rec + 0x30, 'i'), self.u(rec + 0x34, 'i')
        if not data or stride is None or not 0 < stride <= 0x100000 or mask is None:
            return None
        e = data + handle_index(h) * stride
        q8 = self.u(e + 8, 'Q')
        if q8 is None:
            return None
        return object_address(e, mask & M64, q8)

    def ref_at(self, addr):
        b = self.read(addr, 8)
        return decode_ref(b) if b else None


def walk_list(m, cell):
    """Yield (handle, node address) for a client component list (head {u32 handle, u64 node offset})."""
    lst = m.u(m.base + cell, 'Q')
    if not lst:
        return None
    h, off = m.u(lst, 'I'), m.u(lst + 8, 'Q')
    out, seen = [], set()
    while h is not None and off is not None and h != M32 and len(out) < WALK_LIMIT and h not in seen:
        seen.add(h)
        obj = m.resolve(h)
        if obj is None:
            out.append((h, None))
            break
        node = obj + off
        out.append((h, node))
        h, off = m.u(node, 'I'), m.u(node + 8, 'Q')
    return out


def player_position(m, ut):
    try:
        return ut.read_unit_position(m.p, m.base)
    except Exception:
        return None


# ---- subcommands: each returns (data, text lines) ---------------------------------------------------

def cmd_player(m, ut, args):
    me = player_position(m, ut)
    if me is None:
        return {'player': None}, ['player: unreadable (no unit yet, or a transition)']
    d = {'handle': me.handle, 'address': me.address, 'position': rnd(me.position)}
    return {'player': d}, [f'player handle={me.handle:#x} unit={me.address:#x} position={tuple(d["position"])}']


def cmd_navpoints(m, ut, args):
    me = player_position(m, ut)
    mp = me.position if me else None
    lines = [f'player {tuple(rnd(mp)) if mp else "unreadable"}']
    base = m.base
    tbl = base + BUNDLE_TABLE
    idx, bias, stride = m.u(base + BUNDLE_POOL_INDEX, 'H'), m.u(tbl, 'q'), m.u(tbl + 0x10, 'Q')
    content = None
    if None not in (idx, bias, stride):
        content = m.u(tbl + bias + idx * stride + 0x30, 'Q')
    if not content:
        return {'navpoints': None}, lines + ['navpoints: content table unreadable']
    ks = m.read(content + CONTENT_KIND_INDEX + 4 * NAV_KIND, 4)
    if ks is None:
        return {'navpoints': None}, lines + ['navpoints: kind index unreadable']
    start, count = struct.unpack('<hh', ks)
    items = []
    for i in range(max(0, min(count, WALK_LIMIT))):
        dx = m.u(content + CONTENT_DX_TABLE + 2 * (start + i), 'H')
        row = m.read(content + CONTENT_ROWS + (dx or 0) * ROW_STRIDE, ROW_STRIDE) if dx is not None else None
        if row is None:
            items.append({'i': i, 'dx': dx, 'position': None})
            lines.append(f'{i} dx={dx} unreadable')
            continue
        tag, off = decode_nav_row(row)
        obj = m.resolve(tag)
        raw = m.read(obj + off + NAV_POS_OFF, 12) if obj else None
        pos = decode_nav_position(raw) if raw else None
        d = distance(pos, mp, planar=True)
        if args.near is not None and (d is None or d > args.near):
            continue
        items.append({'i': i, 'dx': dx, 'src_tag': tag, 'src_off': off, 'position': rnd(pos), 'd2d': None if d is None else round(d, 1)})
        lines.append(f'{i} dx={dx} src_tag={tag:#x} off={off:#x} ' +
                     (f'pos={tuple(rnd(pos))} d2d={round(d, 1)}' if pos and d is not None else
                      f'pos={tuple(rnd(pos)) if pos else "unreadable"}'))
    return {'navpoints': items}, lines


def cmd_interactables(m, ut, args):
    me = player_position(m, ut)
    mp = me.position if me else None
    nodes = walk_list(m, INTERACTABLE_LIST_CELL)
    lines = [f'player {tuple(rnd(mp)) if mp else "unreadable"}']
    if nodes is None:
        return {'interactables': None}, lines + ['interactable list not allocated (client not in a world yet)']
    items = []
    for h, node in nodes:
        if node is None:
            items.append({'handle': h, 'node': None})
            lines.append(f'h={h:#x} unreadable')
            continue
        radius, pos = m.u(node + 0x50, 'f'), m.f3(node + 0x60)
        ref = m.ref_at(node - INTERACTABLE_NODE_IN_COMP + INTERACTABLE_REF_IN_COMP)
        if args.ref and ref != args.ref:
            continue
        d = distance(pos, mp)
        if args.near is not None and (d is None or d > args.near):
            continue
        items.append({'handle': h, 'node': node, 'radius': radius, 'position': rnd(pos),
                      'distance': None if d is None else round(d, 1), 'ref': ref})
        lines.append(f'h={h:#x} radius={"unreadable" if radius is None else round(radius, 2)} '
                     f'pos={tuple(rnd(pos)) if pos else "unreadable"} '
                     f'd={"?" if d is None else round(d, 1)} ref={fmt_ref(ref) if ref else "unreadable"}')
    return {'interactables': items}, lines


def trigger_shape(m, comp):
    """The shape record address of a trigger component, or None."""
    gh, gidx = m.u(comp + 0x50, 'I'), m.u(comp + 0x58, 'I')
    G = m.resolve(gh)
    if G is None or gidx is None:
        return None
    o1c, c18 = m.u(G + 0x1C, 'i'), m.u(G + 0x18, 'i')
    if o1c is None or c18 is None:
        return None
    o54 = m.u(G + 0x54 + c18 * 0x30, 'i')
    if o54 is None:
        return None
    return m.u(G + o1c + o54 + gidx * 8, 'Q')


def cmd_triggers(m, ut, args):
    me = player_position(m, ut)
    mp = me.position if me else None
    nodes = walk_list(m, TRIGGER_LIST_CELL)
    lines = [f'player {tuple(rnd(mp)) if mp else "unreadable"}']
    if nodes is None:
        return {'triggers': None}, lines + ['trigger volume list not allocated (client not in a world yet)']
    items = []
    for h, node in nodes:
        if node is None:
            items.append({'handle': h, 'component': None})
            lines.append(f'h={h:#x} unreadable')
            continue
        comp = node - TRIGGER_NODE_IN_COMP
        ref = m.ref_at(comp + 0x5C)
        if args.ref and ref != args.ref:
            continue
        shape = trigger_shape(m, comp)
        raw = m.read(shape + SHAPE_BOX_OFF, 0x20) if shape else None
        half = centre = w = None
        if raw:
            half, centre, w = decode_shape_box(raw)
        d = distance(centre, mp)
        if args.near is not None and (d is None or d > args.near):
            continue
        items.append({'handle': h, 'component': comp, 'self_ok': m.u(comp + 0x24, 'I') == h, 'ref': ref,
                      'shape': shape, 'centre': rnd(centre), 'half_extents': rnd(half), 'w': w,
                      'distance': None if d is None else round(d, 1)})
        lines.append(f'h={h:#x} ref={fmt_ref(ref) if ref else "unreadable"}')
        lines.append('    centre=' + (str(tuple(rnd(centre, 1))) if centre else 'unreadable') +
                     '  half=' + (str(tuple(rnd(half, 1))) if half else 'unreadable') +
                     f'  d={"?" if d is None else round(d, 1)}')
    return {'triggers': items}, lines


def cmd_row(m, ut, args):
    if not args.ref:
        sys.exit('row needs --ref 0xHASH/0xKIND/INDEX')
    hash_, kind, index = args.ref
    base = m.base
    out = {'ref': args.ref}
    lines = [f'-- {fmt_ref(args.ref)}']
    pool_idx = m.u(base + BUNDLE_POOL_INDEX, 'H')
    bias, stride = m.u(base + BUNDLE_TABLE, 'Q'), m.u(base + BUNDLE_STRIDE, 'Q')
    if not pool_idx or bias is None or stride is None:
        return out, lines + ['    bundle table unreadable']
    rec = m.u(base + BUNDLE_TABLE + bias + pool_idx * stride + 0x30, 'Q')
    n = m.u(rec + 8, 'i') if rec else None
    if n is None:
        return out, lines + ['    bundle table unreadable']
    bi = None
    for k in range(max(0, min(n, WALK_LIMIT))):
        if m.u(rec + 0x14 + 0x14 * k, 'I') == hash_:
            bi = m.u(rec + 0x10 + 0x14 * k, 'i')
            break
    else:
        return out, lines + [f'    bundle {hash_:#x} not in table']
    if bi is None:
        return out, lines + ['    base index unreadable']
    ra = rec + (bi + index) * 0x38 + ROW_RECORDS_OFF
    h, off = m.u(ra, 'I'), m.u(ra + 8, 'Q')
    if h is None or off is None:
        return out, lines + ['    record unreadable']
    if h == M32:
        return out, lines + [f'    record {ra:#x} handle -1 (row not instantiated)']
    obj = m.resolve(h)
    O = None if obj is None else (obj + off) & M64
    eh = m.u(O + ENTRY_HANDLE_OFF, 'I') if O else None
    lines.append(f'    record handle {h:#x} offset {hx(off)} -> object {hx(O)}  entry handle {hx(eh)}')
    out.update({'record_handle': h, 'object': O, 'entry_handle': eh})
    if eh is None or eh == M32:
        return out, lines + ['    no entry handle: this object has no replica entry']
    E = m.resolve(eh)
    if E is None:
        return out, lines + ['    entry unresolvable']
    kh, kk, ki = m.u(E, 'I'), m.u(E + 4, 'B'), m.u(E + 6, 'h')
    f = {n_: m.u(E + o, t) for n_, o, t in (('owner', 0x60, 'I'), ('pending', 0x15, 'B'), ('flag', 0x16, 'B'),
                                          ('dirty', 0x66, 'B'), ('a_desc', 0xC, 'I'), ('b_desc', 8, 'I'))}
    lines.append(f'    E {E:#x} key {hx(kh)}/{hx(kk)}/{ki} (key ok: {(kh, kk, ki) == args.ref})')
    lines.append('    ' + '  '.join(f'{k_}={hx(v)}' for k_, v in f.items()))
    # value buffer: [BUNDLE_BUFFERS + u16(E+0x40) * stride + bias] + u64(E+0x38)
    bidx, boff = m.u(E + 0x40, 'H'), m.u(E + 0x38, 'Q')
    b = m.u(base + BUNDLE_BUFFERS + bidx * stride + bias, 'Q') if bidx is not None and None not in (stride, bias) else None
    buf = None if b is None or boff is None else (b + boff) & M64
    data = m.read(buf, 0x60) if buf else None
    lines.append(f'    value buffer {hx(buf)}: {words(data, 24)}')
    out.update({'entry': E, 'key': [kh, kk, ki], 'fields': f, 'buffer': buf, 'buffer_words': words(data, 24)})
    return out, lines


def cmd_flags(m, ut, args):
    base = m.base
    me = player_position(m, ut)
    unit = me.address if me else None
    ent = m.u(unit + 0x2C, 'I') if unit else None
    lines = [f'unit {hx(unit)} entity handle (+0x2c) {hx(ent)}']
    ids_ptr = m.u(base + PLAYER_IDS_PTR, 'Q')
    raw = m.read(ids_ptr + 8, 80) if ids_ptr else None
    ids = list(struct.unpack('<20I', raw)) if raw else None
    slot = ids.index(ent) if ids and ent in ids else None
    T0 = base + PLAYER_TABLE
    te, ts, t1c, t20, t34 = (m.u(T0 + 8, 'Q'), m.u(T0 + 0x10, 'I'), m.u(T0 + 0x1C, 'I'),
                             m.u(T0 + 0x20, 'Q'), m.u(T0 + 0x34, 'I'))
    sh = None
    if slot is not None and None not in (te, t1c, t20, t34):
        gen = m.u(te + slot * (t20 & M32) + t1c, 'I')
        if gen is not None:
            v = gen & (t20 >> 32)
            if t34 & 0x40000000:
                sh = ((v & 7) << 0x1B) | ((t34 & 0x3FFF) << 0xD) | slot | 0x80000000
            else:
                sh = ((v & 0xFF) << 0x17) | ((t34 & 0x3FF) << 0xD) | slot
    lines.append(f'slot {slot} slot handle {hx(sh)}')
    X = None
    if sh is not None and te and ts is not None:
        X = m.resolve(m.u(te + (sh & 0x1FFF) * ts + 0x50, 'I'))
    lines.append(f'entity {hx(X)}')
    out = {'slot': slot, 'entity': X, 'count': None, 'hashes': None}
    if X:
        cnt = m.u(X + FLAG_COUNT_OFF, 'i')
        hb = m.read(X + FLAG_HASHES_OFF, 4 * FLAG_HASHES_MAX)
        out['count'] = cnt
        out['hashes'] = list(struct.unpack(f'<{FLAG_HASHES_MAX}I', hb)) if hb else None
        lines.append(f'flag count {cnt}  hashes {words(hb, FLAG_HASHES_MAX)}')
    return {'flags': out}, lines


COMMANDS = {'player': cmd_player, 'navpoints': cmd_navpoints, 'interactables': cmd_interactables,
            'triggers': cmd_triggers, 'row': cmd_row, 'flags': cmd_flags}


# ---- entry ------------------------------------------------------------------------------------------

def shadps4_running():
    try:
        out = subprocess.run(['tasklist', '/FI', 'IMAGENAME eq shadPS4.exe', '/FO', 'CSV', '/NH'],
                             capture_output=True, text=True).stdout
    except OSError:
        return False
    return any(line.startswith('"shadPS4') for line in out.splitlines())


def load(name, fname):
    spec = importlib.util.spec_from_file_location(name, HERE / fname)
    mod = importlib.util.module_from_spec(spec)
    if str(HERE) not in sys.path:
        sys.path.insert(0, str(HERE))
    spec.loader.exec_module(mod)
    return mod


def ref_arg(text):
    try:
        return parse_ref(text)
    except ValueError as e:
        raise argparse.ArgumentTypeError(str(e))


def build_parser():
    ap = argparse.ArgumentParser(prog='d1-world-probe', description='Read-only world-state probes for the live 56578 client.')
    ap.add_argument('command', choices=sorted(COMMANDS))
    ap.add_argument('--near', type=float, metavar='M', help='only items within M of the local Guardian')
    ap.add_argument('--ref', type=ref_arg, metavar='0xHASH/0xKIND/INDEX', help='a bundle row reference')
    ap.add_argument('--json', action='store_true', help='print JSON instead of text')
    return ap


def main(argv=None):
    args = build_parser().parse_args(argv)
    if sys.platform != 'win32':
        print('d1-world-probe: Windows only (reads shadPS4 guest memory)')
        return 1
    if not shadps4_running():
        print('d1-world-probe: no shadPS4 process is running - nothing to probe')
        return 1
    g = load('winguest', 'd1-winguest.py')
    ut = load('d1_unit_tracking', 'd1_unit_tracking.py')
    with contextlib.redirect_stdout(sys.stderr):      # keep --json output clean of the control line
        p, base = g.open_client()
    data, lines = COMMANDS[args.command](Mem(p, base), ut, args)
    if args.json:
        print(json.dumps(data, default=str))
    else:
        print('\n'.join(lines))
    return 0


if __name__ == '__main__':
    sys.exit(main())
