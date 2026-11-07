# SPDX-License-Identifier: GPL-2.0-or-later
"""Mark a served profile's unused stat-override entries absent (key 0xFF), build 56578.

    python -I d1-profile-override-tail.py [--check] PROFILE.bin [...]

The profile (0x80801743) carries a stat-override block: a present byte at +0x6FDC and seven
entries from +0x6FE0, stride 8 ({i8 key, i32 value at +4}). The client's bulk reader (0x281860)
applies every entry whose key is not 0xFF, so an unused entry written as (0, 0) sets stat 0 (the
effective rating) to 0, and the game rates the character at about level 1. That refused The Awoken
("lacking permissions"), showed "Requires Better Gear" on level 10-20 story nodes, and hid content
(notes/ps4-56578-content-gates-2026-11-07.md, notes/ps4-56578-awoken-2026-11-07.md). d1-oracle's
ProfileSpec writes 0xFF since #346; this fixes the fixtures captured or edited before that.

Only entries 1-6 that read exactly (0, 0) change. Each file goes through the client's codec
(d1-oracle object): decode -> edit -> encode (must report IDENTICAL) -> decode again, which must
equal the edited object and differ from the source only at the changed key bytes. The file is
rewritten in place. --check reports and writes nothing (exit 1 if any file needs the fix).

Needs D1_ORACLE (or the default d1-oracle.exe) and D1_BUILDS/D1_BUILD, as d1-level1-fixtures.py.
"""
import argparse
import importlib.util
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('level1', HERE / 'd1-level1-fixtures.py')
level1 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(level1)

PROFILE = level1.PROFILE
PRESENT, ENTRIES, STRIDE, COUNT, ABSENT = 0x6FDC, 0x6FE0, 8, 7, 0xFF


def entries(b):
    return [(b[ENTRIES + STRIDE * i], int.from_bytes(b[ENTRIES + STRIDE * i + 4:ENTRIES + STRIDE * i + 8], 'little', signed=True))
            for i in range(COUNT)]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--check', action='store_true', help='report only; exit 1 if any file needs the fix')
    ap.add_argument('profiles', nargs='+', type=Path)
    a = ap.parse_args()
    needed = 0
    for path in a.profiles:
        path = path.resolve()  # d1-oracle runs in the workspace folder
        before = level1.decode(path, PROFILE)
        b = bytearray(before)
        if not b[PRESENT]:
            print(f'{path}: no override block; unchanged')
            continue
        fix = [i for i in range(1, COUNT) if entries(b)[i] == (0, 0)]
        print(f'{path}: entries {entries(b)}; unused (0, 0) at {fix or "none"}')
        if not fix:
            continue
        needed += 1
        if a.check:
            continue
        for i in fix:
            b[ENTRIES + STRIDE * i] = ABSENT
        with tempfile.TemporaryDirectory() as t:
            out = Path(t) / path.name
            level1.encode(bytes(b), PROFILE, out)
            after = level1.decode(out, PROFILE)
            if after != bytes(b):
                sys.exit(f'{path}: the re-decoded object differs from the edited one; not written')
            diff = [i for i, (x, y) in enumerate(zip(before, after)) if x != y]
            want = [ENTRIES + STRIDE * i for i in fix]
            if diff != want:
                sys.exit(f'{path}: changed bytes {[hex(d) for d in diff]}, expected {[hex(w) for w in want]}; not written')
            path.write_bytes(out.read_bytes())
        print(f'  fixed: keys at {[hex(w) for w in want]} -> 0xFF (codec round trip identical)')
    if a.check and needed:
        sys.exit(1)


if __name__ == '__main__':
    main()
