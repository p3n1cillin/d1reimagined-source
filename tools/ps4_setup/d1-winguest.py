#!/usr/bin/env python3
"""d1-winguest - read and (on request) write the live 56578 client's memory under shadPS4 on Windows.

The Windows counterpart of capture-rig/d1guest.py (which used Linux /proc/<pid>/mem). shadPS4 runs
the guest natively in its own process, so guest pointers ARE host addresses. Only eboot-relative
addresses need the load base, which the emulator logs as "Loading module eboot.bin to 0x...".

Every write is SYNTHETIC (synthetic_ancestor) and is printed before it is made; `launch` refuses
unless every control passes, and does nothing without --confirm.

    d1-winguest.py base
    d1-winguest.py session                 # the session-0 parameter block, as 0x775770 computes it
    d1-winguest.py launch --activity-id 407 [--confirm]

Addresses (build 56578, ELF sha 672f0341...; re-derived 2026-10-13, see notes/ps4-retarget):
  0x775770  session accessor = 01.29 0x773FC0, byte-for-byte: strides 0x131C8/0x98E0, state +0x8C58,
            params +0x6B38, valid mask 0x7F0; manager pointer global 0x26CA4E0 (01.29 0x26B1850)
  params+0x9B8   desired-activity parameter; +0xEA its u16 activity id (0xB8E1C0 reads it, cmp -1)
  params+0x0     goal parameter; +0xE4 its u32 value (read at 0xB8FFA2)
  0x19474B0 c_world_controller_state_setup_orbit_handler vtable (RTTI); handler +0x38 = debounce
            stamp (0x7AC9FA writes it, 0x7ACA13 compares with -1), same offset as 01.29
"""
import ctypes, ctypes.wintypes as wt, os, re, struct, subprocess, sys

ELF_SHA = '672f03411c0503dfbcda804cbb9fb27cdd9a8d73a8bcb71bf0c4f688073b0833'


def shad_log():
    """The emulator log, below D1_SHADPS4 (see d1paths.py). Other scripts exec this source without
    __file__; d1paths.py then comes from beside the script that did."""
    here = os.path.dirname(os.path.abspath(globals().get('__file__') or sys.argv[0]))
    if here not in sys.path:
        sys.path.insert(0, here)
    import d1paths
    return str(d1paths.shadps4() / 'user' / 'log' / 'shad_log.txt')


MANAGER_PTR = 0x26CA4E0
SESSION_STRIDE, INNER_STRIDE, STATE_OFF, PARAMS_OFF, VALID_MASK = 0x131C8, 0x98E0, 0x8C58, 0x6B38, 0x7F0
ACTIVITY_PARAM, ACTIVITY_ID, GOAL_VALUE, HAS_VALUE = 0x9B8, 0xEA, 0xE4, 0xE0
ORBIT_VTABLE, DEBOUNCE = 0x19474B0, 0x38
# rung 34 c_world_controller_state_activity_initial_slice_set_loading_handler (RTTI). Its update
# 0x7969A0 requests rung 0x23 when [this+0x1A8] (done) == [this+0x198] (requested), as on 01.29.
SLICE_VTABLE, REQUESTED, STARTED, DONE = 0x1945A50, 0x198, 0x1A0, 0x1A8
# Task 7 (rung 34, 0x7962D0) returns 2 at once when 0x766750() holds: byte 0 of the activity-
# selection record == 1 (or 0x78C710()). The record pointer is the qword at 0x26BC068 (0x574EF0 =
# lea rax,[0x26BC060]; mov rax,[rax+8]) - 01.29's [0x26A33E8]. Does not survive a teardown.
SELECTION_RECORD_PTR = 0x26BC068
# The activity lifetime phase the spawn gate reads, exactly as 0x10C500 computes it (same address
# as 01.29): 0x1072C0(0x10, 0) -> {tag, offset} via the count word 0x25D6FF8 and the self-relative
# table at 0x1AD5000; then the registry walk (global 0x1A238A0) resolves tag -> object, and the
# phase is the signed byte at object + offset + 0x1AC (-1 = unset). udt 5 sensor_auth_update writes it.
LT_COUNT, LT_TABLE, REGISTRY = 0x25D6FF8, 0x1AD5000, 0x1A238A0
# The world pool the udt-5 decoder resolves into (0x2A00A0: [0x26556D8] + (i & 0x1FFF) * [0x26556E0]).
# The decoder (0x2A1C40) drops the frame unless world+0x1C != 0 and frame gen == world+0x1D (0x2A1E88).
WORLD_POOL, WORLD_STRIDE = 0x26556D8, 0x26556E0
# 01.29's forced spawn (design/GUARDIAN-IN-WORLD.md §4-5), re-derived for 56578. All SYNTHETIC.
#  * 0x10C500 lifetime-state getter (same address and bytes as 01.29) -> mov [rdi],3; mov eax,1; ret
#  * 0x412DD6 per-player reject `je +0x371` after the spawn gate 0x415700 (01.29 0x411E16) -> NOP x6
#  * player pool: r13 = [[0x273EA80]+8] (01.29 0x2725700), player i at r13 + i*0x2AF0, spawn word +0x0C
GETTER, GETTER_ORIG, GETTER_HOOK = 0x10C500, '554889e54156534883ec2049', 'c70703000000b801000000c3'
REJECT_JE, REJECT_ORIG, REJECT_NOP = 0x412DD6, '0f8471030000', '909090909090'
PLAYER_POOL, PLAYER_STRIDE, SPAWN_WORD = 0x273EA80, 0x2AF0, 0x0C
#  * screen-fade block: [0x26AD7F0] (01.29 [0x2694B90]) -> +0x00 count, +0x14 mode, +0x18 alpha
#    (1.0f = black). Parked at 1.0 after a forced spawn; zeroing all three lifts the picture.
FADE_PTR = 0x26AD7F0
DATA_LO, DATA_HI = 0x18D8000, 0x2F6C000   # eboot RW segment .. end of image (BSS included)
RUNG_IN_WORLD = 0x24

k32 = ctypes.WinDLL('kernel32', use_last_error=True)
k32.OpenProcess.restype = wt.HANDLE
k32.ReadProcessMemory.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_size_t, ctypes.POINTER(ctypes.c_size_t)]
k32.WriteProcessMemory.argtypes = k32.ReadProcessMemory.argtypes


class Proc:
    def __init__(self, pid):
        self.h = k32.OpenProcess(0x0010 | 0x0020 | 0x0008 | 0x0400 | 0x0800, False, pid)
        if not self.h:
            sys.exit(f'OpenProcess({pid}) failed: {ctypes.get_last_error()}')

    def read(self, addr, n):
        buf = ctypes.create_string_buffer(n); got = ctypes.c_size_t()
        if not k32.ReadProcessMemory(self.h, ctypes.c_void_p(addr), buf, n, ctypes.byref(got)) or got.value != n:
            raise OSError(f'read {addr:#x}+{n:#x} failed ({ctypes.get_last_error()})')
        return buf.raw

    def write(self, addr, data):
        got = ctypes.c_size_t()
        if not k32.WriteProcessMemory(self.h, ctypes.c_void_p(addr), data, len(data), ctypes.byref(got)) or got.value != len(data):
            raise OSError(f'write {addr:#x} failed ({ctypes.get_last_error()})')

    def u(self, addr, fmt):
        return struct.unpack('<' + fmt, self.read(addr, struct.calcsize(fmt)))[0]


def open_client():
    out = subprocess.run(['tasklist', '/FI', 'IMAGENAME eq shadPS4.exe', '/FO', 'CSV', '/NH'], capture_output=True, text=True).stdout
    pids = [int(l.split('","')[1]) for l in out.splitlines() if l.startswith('"shadPS4')]
    if len(pids) != 1:
        sys.exit(f'expected one shadPS4 process, found {pids}')
    log = open(shad_log(), 'rb').read(2_000_000).decode('utf-8', 'replace')
    m = re.findall(r'Loading module eboot\.bin to (0x[0-9a-f]+)', log)
    if len(m) != 1:
        sys.exit(f'expected one eboot load line in the emulator log, found {m}')
    base = int(m[0], 16)
    p = Proc(pids[0])
    # Control: the orbit handler vtable's first slot must be a relocated pointer into the image's
    # code segment, and a shifted read must differ (an all-zero page cannot pass by being degenerate).
    slot0 = p.u(base + ORBIT_VTABLE, 'Q'); shifted = p.u(base + ORBIT_VTABLE + 0x1000, 'Q')
    ok = base <= slot0 < base + 0x18D8000 and slot0 != shifted
    print(f'shadPS4 pid {pids[0]}  eboot base {base:#x}  control vtable[0]={slot0:#x} {"PASS" if ok else "FAIL"}')
    if not ok:
        sys.exit(2)
    return p, base


ntdll = ctypes.WinDLL('ntdll')


def patch_code(p, addr, data):
    # Freeze-write-thaw: a torn multi-byte patch on a hot function (the lifetime getter runs every
    # frame) crashed a 01.29 run (GUARDIAN-IN-WORLD.md, LESSONS.md). Suspend the whole emulator
    # process around the write.
    if ntdll.NtSuspendProcess(p.h) != 0:
        raise OSError('NtSuspendProcess failed')
    try:
        _patch_code(p, addr, data)
    finally:
        ntdll.NtResumeProcess(p.h)


def _patch_code(p, addr, data):
    old = wt.DWORD()
    k32.VirtualProtectEx.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.c_size_t, wt.DWORD, ctypes.POINTER(wt.DWORD)]
    if not k32.VirtualProtectEx(p.h, ctypes.c_void_p(addr), len(data), 0x40, ctypes.byref(old)):
        raise OSError(f'VirtualProtectEx {addr:#x} failed ({ctypes.get_last_error()})')
    p.write(addr, data)
    k32.VirtualProtectEx(p.h, ctypes.c_void_p(addr), len(data), old.value, ctypes.byref(wt.DWORD()))
    k32.FlushInstructionCache.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.c_size_t]
    k32.FlushInstructionCache(p.h, ctypes.c_void_p(addr), len(data))


def session(p, base, index=0):
    mgr = p.u(base + MANAGER_PTR, 'Q')
    if not mgr or p.read(mgr, 1)[0] == 0:
        sys.exit(f'session manager [{base + MANAGER_PTR:#x}] = {mgr:#x}: not valid yet')
    slot = mgr + index * 0x131C8                    # session index (0x775180's stride)
    inner = struct.unpack('<i', p.read(slot + 8, 4))[0] * INNER_STRIDE
    state = p.u(slot + inner + STATE_OFF, 'I')
    if state > 0xA or not (VALID_MASK >> state) & 1:
        sys.exit(f'session state {state} is outside 4..10; the client\'s accessor would return NULL')
    params = slot + inner + PARAMS_OFF
    return mgr, state, params


def orbit_handler(p, base, vtable=ORBIT_VTABLE):
    want = struct.pack('<Q', base + vtable)
    lo, hi = base + DATA_LO, base + DATA_HI
    hits, step = [], 0x10000
    for a in range(lo, hi, step):
        try:
            chunk = p.read(a, min(step + 8, hi - a))
        except OSError:
            continue
        i = chunk.find(want)
        while i >= 0:
            if i % 8 == 0 and i < step:
                hits.append(a + i)
            i = chunk.find(want, i + 1)
    return hits


def main():
    cmd = sys.argv[1] if len(sys.argv) > 1 else 'base'
    p, base = open_client()
    if cmd == 'base':
        return 0
    if cmd == 'players':
        r13 = p.u(p.u(base + PLAYER_POOL, 'Q') + 8, 'Q')
        print(f'player pool r13 {r13:#x} (stride {PLAYER_STRIDE:#x})')
        for i in range(4):
            a = r13 + i * PLAYER_STRIDE
            print(f'  player[{i}] {a:#x}: +0x0C = {p.u(a + SPAWN_WORD, "I"):#x}  +0x00..0x20 {p.read(a, 0x20).hex(" ")}')
        return 0
    if cmd in ('spawn-hooks', 'restore-hooks'):
        for va, orig, new in ((GETTER, GETTER_ORIG, GETTER_HOOK), (REJECT_JE, REJECT_ORIG, REJECT_NOP)):
            cur = p.read(base + va, len(orig) // 2).hex()
            want_from, want_to = (orig, new) if cmd == 'spawn-hooks' else (new, orig)
            print(f'  {va:#x}: now {cur}')
            if cur == want_to:
                print('    already in place'); continue
            if cur != want_from:
                sys.exit(f'REFUSING {va:#x}: bytes are neither the original nor the hook')
            if '--confirm' in sys.argv:
                patch_code(p, base + va, bytes.fromhex(want_to))
                print(f'    wrote {p.read(base + va, len(orig) // 2).hex()}  (SYNTHETIC)')
            else:
                print('    dry run (add --confirm)')
        return 0
    if cmd == 'replicas':
        # 0x2D1600 (walker terminator) walks the replica pool [world+0x28] and REJECTS the frame when a
        # pending replica (+0x15 != 0) has owner +0x60 == the local partition id or > 0x7F. Local id:
        # 0x345620 -> bits 3..8 of dword [[0x2693B60+8]+0x2114] (0x12D700), -1 if unset.
        pool, stride = p.u(base + WORLD_POOL, 'Q'), p.u(base + WORLD_STRIDE, 'I')
        w = pool + int(sys.argv[sys.argv.index('--world') + 1] if '--world' in sys.argv else 0) * stride
        raw = p.u(p.u(base + 0x2693B60 + 8, 'Q') + 0x2114, 'I')
        local = -1 if raw == 0xFFFFFFFF else (raw >> 3) & 0x3F
        P = p.u(w + 0x28, 'Q')
        cnt = p.u(P + 0x14, 'i')
        bm = p.u(p.u(p.u(P, 'Q') + 8, 'Q') + 0x10, 'Q')
        print(f'local partition id {local} (raw {raw:#x}); replica pool {P:#x} slots {cnt} bitmap {bm:#x}')
        data, base_off, esz, mask, flags = p.u(P + 8, 'Q'), p.u(P + 0x1C, 'I'), p.u(P + 0x20, 'I'), p.u(P + 0x24, 'I'), p.u(P + 0x34, 'I')
        reg = p.u(p.u(base + REGISTRY, 'Q'), 'Q')
        shown = 0
        for slot in range(min(cnt, 8192)):
            if not (p.u(bm + 4 * (slot // 32), 'I') >> (slot % 32)) & 1:
                continue
            hi = p.u(data + base_off + esz * slot, 'I') & mask
            if flags & 0x40000000:
                tag = 0x80000000 | ((hi & 7) << 27) | ((flags << 13) & 0x7FFE000) | (slot & 0x1FFF)
            else:
                tag = ((hi << 23) & 0x7F800000) | ((flags << 13) & 0x7FE000) | (slot & 0x1FFF)
            esi = struct.unpack('<i', struct.pack('<I', tag))[0] >> 13
            sl = (((esi & 0xFFFFFFFF) >> 0x12) | 0x3FF) & esi & 0xFFFFFFFF
            rec = reg + sl * 0x40
            d, es, mk = p.u(rec + 0x10, 'Q'), p.u(rec + 0x30, 'i'), p.u(rec + 0x34, 'i') & 0xFFFFFFFFFFFFFFFF
            e = d + (tag & 0x1FFF) * es
            try:
                obj = e - (mk & p.u(e + 8, 'Q')); pend = p.read(obj + 0x15, 1)[0]; own = p.u(obj + 0x60, 'I')
            except OSError:
                continue
            reject = pend and (own == local or own > 0x7F)
            if pend or shown < 12:
                print(f'  slot {slot:5d} tag {tag:#010x} obj {obj:#x} pending {pend} owner {own:#x}{"  <== REJECTS" if reject else ""}')
                shown += 1
        return 0
    if cmd == 'sensor-clients':
        # Every PENDING sensor entity client of world 0 that the local machine must receive (owner
        # +0x60 == local id or > 0x7F), as `hash i8 sub has_a has_b` lines for D1_AH_SENSOR_CLIENTS.
        # Client layout (pool "sensor entity clients 0"): +0 name {u32 hash, i8 @+4, i16 @+6},
        # +8 B type, +0xC A type, +0x15 pending, +0x60 owner. Names are per-load: read them after the
        # world is resident. Read-only.
        pool, stride = p.u(base + WORLD_POOL, 'Q'), p.u(base + WORLD_STRIDE, 'I')
        w = pool + int(sys.argv[sys.argv.index('--world') + 1] if '--world' in sys.argv else 0) * stride
        raw = p.u(p.u(base + 0x2693B60 + 8, 'Q') + 0x2114, 'I')
        local = -1 if raw == 0xFFFFFFFF else (raw >> 3) & 0x3F
        P = p.u(w + 0x28, 'Q'); cnt = p.u(P + 0x14, 'i')
        bm = p.u(p.u(p.u(P, 'Q') + 8, 'Q') + 0x10, 'Q')
        data, base_off, esz, mask, flags = p.u(P + 8, 'Q'), p.u(P + 0x1C, 'I'), p.u(P + 0x20, 'I'), p.u(P + 0x24, 'I'), p.u(P + 0x34, 'I')
        reg = p.u(p.u(base + REGISTRY, 'Q'), 'Q')
        out = [f'# world {w:#x} gen {p.read(w + 0x1D, 1)[0]} local id {local}: hash i8 sub has_a has_b']
        bundled = False
        for slot in range(min(cnt, 8192)):
            if not (p.u(bm + 4 * (slot // 32), 'I') >> (slot % 32)) & 1:
                continue
            hi = p.u(data + base_off + esz * slot, 'I') & mask
            if flags & 0x40000000:
                tag = 0x80000000 | ((hi & 7) << 27) | ((flags << 13) & 0x7FFE000) | (slot & 0x1FFF)
            else:
                tag = ((hi << 23) & 0x7F800000) | ((flags << 13) & 0x7FE000) | (slot & 0x1FFF)
            esi = struct.unpack('<i', struct.pack('<I', tag))[0] >> 13
            sl = (((esi & 0xFFFFFFFF) >> 0x12) | 0x3FF) & esi & 0xFFFFFFFF
            rec = reg + sl * 0x40
            d, es, mk = p.u(rec + 0x10, 'Q'), p.u(rec + 0x30, 'i'), p.u(rec + 0x34, 'i') & 0xFFFFFFFFFFFFFFFF
            e = d + (tag & 0x1FFF) * es
            obj = e - (mk & p.u(e + 8, 'Q'))
            h, i8, sub, tb, ta = struct.unpack('<IBxhII', p.read(obj, 0x10))
            pend, own = p.read(obj + 0x15, 1)[0], p.u(obj + 0x60, 'I')
            # --all also lists clients already delivered (pending clear), e.g. on a run that serves them.
            if (pend or '--all' in sys.argv) and (own == local or own > 0x7F):
                line = f'{h:#010x} {i8:#x} {sub} {int(ta != 0xFFFFFFFF)} {int(tb != 0xFFFFFFFF)}'
                if '--dump' in sys.argv and f'{h:#010x}/{i8:#x}/{sub}' == sys.argv[sys.argv.index('--dump') + 1]:
                    raw = p.read(obj, 0x100)
                    print(f'object {obj:#x}', file=sys.stderr)
                    for o in range(0, 0x100, 16): print(f'  +{o:03x} ' + raw[o:o+16].hex(' '), file=sys.stderr)
                if '--types' in sys.argv:
                    line += f'  # A {ta:#010x} B {tb:#010x} owner {own:#x} pending {pend}'
                # --bundle: give the first type-12 client bundle (i8 0x0C) the local player's value,
                # which is what links the player and lets the spawn gate pass (notes, 13 Oct):
                #   +0x08/+0x09 = 1, +0x38 = players[0]+0x738 (sign-extended), +0x40 = the player key
                #   (player-array entry +0x18), byte-reversed because the client stores that kind-0x23
                #   field reversed relative to the oracle image.
                if '--bundle' in sys.argv and i8 == 0x0C and not bundled and ta == 0x808022A5:
                    pa = p.u(base + 0x2689580 + 8, 'Q')
                    r13 = p.u(p.u(base + PLAYER_POOL, 'Q') + 8, 'Q')
                    img = bytearray(0x78)
                    img[0:8] = bytes.fromhex('ffffffffc59d1c81'); img[8] = 1; img[9] = 1
                    img[0x38:0x3C] = struct.pack('<i', struct.unpack('<b', p.read(r13 + 0x738, 1))[0])
                    img[0x40:0x48] = p.read(pa + 0x18, 8)[::-1]
                    line += f' {ta:#010X} {img.hex()}'.replace('0X', '0x')
                    bundled = True
                out.append(line)
        text = chr(10).join(out) + chr(10)
        if '--out' in sys.argv:
            open(sys.argv[sys.argv.index('--out') + 1], 'w').write(text)
        print(text, end='')
        print(f'{len(out) - 1} pending clients', file=sys.stderr)
        return 0
    if cmd == 'worldset':
        # The udt-5 post-pass sweep's container is world+0x38 (walker 0x2D134B..0x2D13DC); entries are
        # u16 slots at +0xC, groups at +0x5E8/+0x5EC, tags composed as 0x10304F0 does.
        pool, stride = p.u(base + WORLD_POOL, 'Q'), p.u(base + WORLD_STRIDE, 'I')
        w = pool + int(sys.argv[sys.argv.index('--world') + 1] if '--world' in sys.argv else 0) * stride
        c = w + 0x38
        P, n, ng = p.u(c, 'Q'), p.u(c + 8, 'i'), p.u(c + 0x5E8, 'i')
        print(f'world {w:#x} container {c:#x}: pool desc {P:#x} entries {n} groups {ng}')
        if not P or n <= 0:
            return 0
        starts = [p.u(c + 0x5EC + 4 * g, 'i') for g in range(min(ng, 64))]
        print(f'  group starts {starts}')
        data, base_off, esz, mask, flags = p.u(P + 8, 'Q'), p.u(P + 0x1C, 'I'), p.u(P + 0x20, 'I'), p.u(P + 0x24, 'I'), p.u(P + 0x34, 'I')
        reg = p.u(p.u(base + REGISTRY, 'Q'), 'Q')
        for i in range(min(n, 400)):
            slot = p.u(c + 0xC + 2 * i, 'H')
            if slot == 0xFFFF:
                continue
            hi = p.u(data + base_off + esz * slot, 'I') & mask
            if flags & 0x40000000:
                tag = 0x80000000 | ((hi & 7) << 27) | ((flags << 13) & 0x7FFE000) | (slot & 0x1FFF)
            else:
                tag = ((hi << 23) & 0x7F800000) | ((flags << 13) & 0x7FE000) | (slot & 0x1FFF)
            esi = struct.unpack('<i', struct.pack('<I', tag))[0] >> 13
            sl = (((esi & 0xFFFFFFFF) >> 0x12) | 0x3FF) & esi & 0xFFFFFFFF
            rec = reg + sl * 0x40
            d, es, mk = p.u(rec + 0x10, 'Q'), p.u(rec + 0x30, 'i'), p.u(rec + 0x34, 'i') & 0xFFFFFFFFFFFFFFFF
            e = d + (tag & 0x1FFF) * es
            try:
                obj = e - (mk & p.u(e + 8, 'Q'))
                cls = p.u(obj + 0x60, 'I')
            except OSError:
                obj, cls = 0, 0
            mark = '  <== lifetime record' if tag == 0x27FA2003 else ''
            print(f'  [{i:3d}] slot {slot:5d} tag {tag:#010x} obj {obj:#x} +0x60 {cls:#010x}{mark}')
        return 0
    if cmd == 'unfade':
        blk = p.u(base + FADE_PTR, 'Q')
        cnt, mode, alpha = p.u(blk, 'I'), p.u(blk + 0x14, 'I'), p.u(blk + 0x18, 'f')
        print(f'fade block {blk:#x}: count {cnt} mode {mode} alpha {alpha}')
        if '--confirm' in sys.argv:
            p.write(blk, bytes(4)); p.write(blk + 0x14, bytes(8))
            print(f'  zeroed (SYNTHETIC): count {p.u(blk, "I")} mode {p.u(blk + 0x14, "I")} alpha {p.u(blk + 0x18, "f")}')
        else:
            print('  dry run (add --confirm)')
        return 0
    if cmd == 'spawn':
        i = int(sys.argv[sys.argv.index('--player') + 1]) if '--player' in sys.argv else 0
        r13 = p.u(p.u(base + PLAYER_POOL, 'Q') + 8, 'Q')
        a = r13 + i * PLAYER_STRIDE + SPAWN_WORD
        print(f'player[{i}] spawn word @{a:#x} = {p.u(a, "I"):#x}')
        if '--confirm' in sys.argv:
            p.write(a, struct.pack('<I', 3)); print(f'  wrote 3 (SYNTHETIC spawn word); now {p.u(a, "I"):#x}')
        else:
            print('  dry run (add --confirm)')
        return 0
    if cmd == 'worlds':
        pool, stride = p.u(base + WORLD_POOL, 'Q'), p.u(base + WORLD_STRIDE, 'I')
        print(f'world pool {pool:#x} stride {stride:#x}')
        for i in range(4):
            w = pool + i * stride
            try:
                b = p.read(w + 0x10, 0x10)
            except OSError:
                break
            print(f'  world[{i}] {w:#x}: +0x1C enabled={b[0xC]} +0x1D gen={b[0xD]}  +0x10..0x20 {b.hex(" ")}')
        return 0
    if cmd == 'lifetime':
        cnt = p.u(base + LT_COUNT, 'H')
        t = base + LT_TABLE
        rcx = 0
        if cnt:
            rcx = p.u(cnt * p.u(t + 0x10, 'Q') + t + p.u(t, 'q') + 0x30, 'Q')
        if not rcx:
            sys.exit('globals table not resolved')
        idx = 0x10
        if p.u(rcx + idx * 4 + 0x2BD1E, 'h') <= 0:
            sys.exit('globals record 0x10 absent')
        r8 = p.u(rcx + p.u(rcx + idx * 4 + 0x2BD1C, 'h') * 2 + 0x2A51C, 'H')
        if r8 == 0xFFFF:
            sys.exit('globals record 0x10 unmapped')
        ent = rcx + r8 * 0x38 + 0x520
        tag, off = p.u(ent, 'I'), p.u(ent + 8, 'Q')
        if tag == 0xFFFFFFFF:
            sys.exit('lifetime record tag unset')
        reg = p.u(p.u(base + REGISTRY, 'Q'), 'Q')
        esi = struct.unpack('<i', struct.pack('<I', tag))[0] >> 13
        slot = (((esi & 0xFFFFFFFF) >> 0x12) | 0x3FF) & esi & 0xFFFFFFFF
        rec = reg + slot * 0x40
        data, esz, mask = p.u(rec + 0x10, 'Q'), p.u(rec + 0x30, 'i'), p.u(rec + 0x34, 'i') & 0xFFFFFFFFFFFFFFFF
        e = data + (tag & 0x1FFF) * esz
        obj = e - (mask & p.u(e + 8, 'Q'))
        phase_at = obj + off + 0x1AC
        phase = struct.unpack('<b', p.read(phase_at, 1))[0]
        print(f'lifetime record tag {tag:#010x} object {obj:#x} offset {off:#x}: phase byte @{phase_at:#x} = {phase}')
        return 0
    if cmd == 'suppress':
        rec = p.u(base + SELECTION_RECORD_PTR, 'Q')
        if not rec:
            sys.exit('activity-selection record pointer is NULL')
        print(f'activity-selection record {rec:#x}: byte0 = {p.read(rec, 1)[0]}')
        if '--confirm' in sys.argv:
            p.write(rec, b''); print(f'  wrote byte0 = {p.read(rec, 1)[0]}  (SYNTHETIC: task-7 suppress)')
        else:
            print('  dry run (add --confirm)')
        return 0
    if cmd == 'masks':
        hs = orbit_handler(p, base, SLICE_VTABLE)
        if len(hs) != 1:
            sys.exit(f'expected exactly one rung-34 handler, found {[hex(h) for h in hs]}')
        h = hs[0]
        r, st, d = (p.u(h + o, 'Q') for o in (REQUESTED, STARTED, DONE))
        print(f'rung-34 handler {h:#x}: requested {r:#x} started {st:#x} done {d:#x} missing {r & ~d:#x}')
        if '--force-done' in sys.argv:
            if '--confirm' not in sys.argv:
                print('  dry run (add --confirm)'); return 0
            p.write(h + DONE, struct.pack('<Q', r))
            print(f'  wrote done = {p.u(h + DONE, "Q"):#x}  (SYNTHETIC: skips the unfinished tasks, as 01.29 did)')
        return 0
    if cmd == 'goal':
        # goal --session 1 --value 0x1d [--confirm]: during setup:matchmaking 0x7AB5D0 keeps the
        # client matchmaking (team pick, ready-to-start) while session 1's goal is 0x1D; at 0x24 it
        # treats matchmaking as finished by a remote host.
        idx = int(sys.argv[sys.argv.index('--session') + 1]) if '--session' in sys.argv else 0
        mgr, state, params = session(p, base, idx)
        print(f'session {idx}: state {state} goal has={p.read(params + HAS_VALUE, 1)[0] & 1} '
              f'value={p.u(params + GOAL_VALUE, "I"):#x}')
        if '--value' in sys.argv and '--confirm' in sys.argv:
            v = int(sys.argv[sys.argv.index('--value') + 1], 0)
            p.write(params + GOAL_VALUE, struct.pack('<I', v))
            print(f'  wrote goal = {p.u(params + GOAL_VALUE, "I"):#x}  (SYNTHETIC)')
        return 0
    mgr, state, params = session(p, base)
    act = params + ACTIVITY_PARAM
    print(f'session 0: manager {mgr:#x} state {state} params {params:#x}')
    print(f'  desired_activity has={p.read(act + HAS_VALUE, 1)[0] & 1} id={p.u(act + ACTIVITY_ID, "H"):#06x}')
    print(f'  goal             has={p.read(params + HAS_VALUE, 1)[0] & 1} value={p.u(params + GOAL_VALUE, "I"):#x}')
    if cmd == 'session':
        return 0
    if cmd == 'launch':
        aid = int(sys.argv[sys.argv.index('--activity-id') + 1]) if '--activity-id' in sys.argv else 407
        hs = orbit_handler(p, base)
        if len(hs) != 1:
            sys.exit(f'REFUSING: expected exactly one orbit handler (vtable {base + ORBIT_VTABLE:#x}), found {[hex(h) for h in hs]}')
        h = hs[0]
        # --goal 0x1d asks for setup:matchmaking (rung 29) instead of in_world (rung 36), so the
        # client runs its own matchmaking and team pick (0x7AB270) rather than reading goal 0x24
        # as "the activity-session host already finished matchmaking" (0x7AB5D0).
        goal = int(sys.argv[sys.argv.index('--goal') + 1], 0) if '--goal' in sys.argv else RUNG_IN_WORLD
        writes = [(act + ACTIVITY_ID, struct.pack('<H', aid), f'activity id = {aid}'),
                  (params + GOAL_VALUE, struct.pack('<I', goal), f'goal = {goal:#x}'),
                  (h + DEBOUNCE, struct.pack('<Q', 0), 'orbit debounce stamp = 0')]
        print(f'  orbit handler {h:#x} (debounce now {p.u(h + DEBOUNCE, "Q"):#x})')
        for a, d, why in writes:
            print(f'  {a:#x} <- {d.hex()}   {why}   (was {p.read(a, len(d)).hex()})')
        if '--confirm' not in sys.argv:
            print('  dry run: nothing written (add --confirm). SYNTHETIC: record in the run notes.')
            return 0
        for a, d, why in writes:
            p.write(a, d)
            print(f'  wrote {a:#x}: now {p.read(a, len(d)).hex()}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
