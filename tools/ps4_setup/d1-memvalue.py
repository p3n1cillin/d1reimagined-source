"""Find a counter in the live 56578 client by elimination: d1-memvalue.py find|keep|show. Read-only.

    d1-memvalue.py find 25 --out mag.npy [--width 4]    every aligned integer equal to 25
    d1-memvalue.py keep 24 --cand mag.npy               of those, the ones that now read 24
    d1-memvalue.py show --cand mag.npy                  the survivors and their values

For a value the game shows (the magazine count in the HUD): find it, change it in game (fire a
shot), keep the new value, and repeat until a handful of addresses remain. Scans the committed,
writable memory of the guest range (0x100000000..0x1000000000), as d1-walk.py --scan does.
"""
import argparse
import ctypes
import ctypes.wintypes as wt
import importlib.util
import os

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location('d1_winguest', os.path.join(HERE, 'd1-winguest.py'))
winguest = importlib.util.module_from_spec(spec)
spec.loader.exec_module(winguest)

DTYPES = {1: '<u1', 2: '<u2', 4: '<u4'}


class MBI(ctypes.Structure):
    _fields_ = [('BaseAddress', ctypes.c_void_p), ('AllocationBase', ctypes.c_void_p),
                ('AllocationProtect', wt.DWORD), ('PartitionId', wt.WORD), ('RegionSize', ctypes.c_size_t),
                ('State', wt.DWORD), ('Protect', wt.DWORD), ('Type', wt.DWORD)]


def find(p, value, width):
    k32 = ctypes.WinDLL('kernel32', use_last_error=True)
    k32.VirtualQueryEx.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.POINTER(MBI), ctypes.c_size_t]
    out, addr, mbi = [], 0x100000000, MBI()
    while addr < 0x1000000000 and k32.VirtualQueryEx(p.h, ctypes.c_void_p(addr), ctypes.byref(mbi), ctypes.sizeof(mbi)):
        b0, sz = mbi.BaseAddress or 0, mbi.RegionSize
        if mbi.State == 0x1000 and mbi.Protect in (0x04, 0x40) and sz < (1 << 31):
            for c in range(0, sz, 1 << 24):
                try:
                    buf = p.read(b0 + c, min(1 << 24, sz - c))
                except OSError:
                    continue
                a = np.frombuffer(buf[:len(buf) // width * width], dtype=DTYPES[width])
                hits = np.nonzero(a == value)[0]
                out.append(b0 + c + width * hits.astype(np.uint64))
        addr = b0 + sz
    return np.concatenate(out) if out else np.zeros(0, np.uint64)


def read_all(p, cand, width):
    vals = np.full(len(cand), -1, np.int64)
    for i, a in enumerate(cand):
        try:
            vals[i] = int.from_bytes(p.read(int(a), width), 'little')
        except OSError:
            pass
    return vals


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('mode', choices=('find', 'keep', 'show'))
    ap.add_argument('value', type=int, nargs='?')
    ap.add_argument('--out')
    ap.add_argument('--cand')
    ap.add_argument('--width', type=int, choices=(1, 2, 4), default=4)
    a = ap.parse_args()
    p, _ = winguest.open_client()
    if a.mode == 'find':
        cand = find(p, a.value, a.width)
        np.save(a.out, cand)
        print(f'{len(cand)} addresses hold {a.value} ({a.width}-byte) -> {a.out}')
        return
    cand = np.load(a.cand)
    if a.mode == 'keep':
        # Many candidates: one scan and an intersection is faster than reading each.
        if len(cand) > 20_000:
            cand = np.intersect1d(cand, find(p, a.value, a.width))
        else:
            cand = cand[read_all(p, cand, a.width) == a.value]
        np.save(a.cand, cand)
        print(f'{len(cand)} now hold {a.value}')
    if len(cand) <= 40:
        for addr, v in zip(cand, read_all(p, cand, a.width)):
            print(f'  {int(addr):#x} {v}')


if __name__ == '__main__':
    main()
