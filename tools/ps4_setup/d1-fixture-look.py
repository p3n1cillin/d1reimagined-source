# SPDX-License-Identifier: GPL-2.0-or-later
"""Give served peer cards and profiles a coherent Guardian look (identity and customization), 56578.

    python -I d1-fixture-look.py [--check] FILE.bin [...]

The peer view (0x808017FD, the f0t1-*.bin roster card) and the profile's character record
(0x80801714 at +0 of 0x80801743) hold the identity 0x808017FC {race, gender, class} at +0x08 and the
customization 0x8080181A at +0x0C (ps4/fixtures/56578/rtti/fields.tsv). The served fixtures had
race 0, gender 0, class 1 (a Hunter) on a Titan with Titan gear and a Striker, and an all-zero
customization. In the lab that drew NO Guardian on the character screen (runs agr51-agr53); the same
set with a coherent Titan identity and a real look drew one (agr54). Created characters get the
look the player chose (player_store::apply_created_look); this fixes the fixtures served without a
creation.

The look written is a STAND-IN: the client's own default for an Exo male Titan, as its character
creation sent it in run new1 (501 request: race 2, gender 0, class 0; customization f0-f9 below).
The fixture character's original look is unknown. f10 (in the profile, the last-orbited
destination) and f11 (the wear-helmet field, +0x2C) are kept.

Each file goes through the client's codec (d1-oracle object): decode -> write -> encode (IDENTICAL)
-> decode again, which must equal the written object and differ only inside +0x08..+0x28. The file
is rewritten in place. --check reports and writes nothing (exit 1 if any file differs from the look).
Needs D1_ORACLE and D1_BUILDS/D1_BUILD, as d1-level1-fixtures.py.
"""
import argparse
import importlib.util
import struct
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('level1', HERE / 'd1-level1-fixtures.py')
level1 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(level1)

LOOK_AT, LOOK_BYTES = 0x08, 0x20
# Run new1's 501 request (the client's default look for an Exo male Titan): identity, then
# customization f0..f9 with the block's pads at 0 (d1_bap CreateCharacterRequest::look_image).
RACE, GENDER, CLASS = 2, 0, 0
F0, F1, F2, F3 = 570, 0, 0, 0
F4, F5, F6 = 2079, 2115, 2007
F7 = (2124, -1, -1)
F8 = (-1, -1, -1)
F9 = 2052


def look_image():
    b = bytearray(LOOK_BYTES)
    b[0:3] = bytes([RACE, GENDER, CLASS])
    c = bytearray(0x1C)
    struct.pack_into('<h', c, 0x00, F0)
    c[0x02], c[0x03], c[0x04] = F1 & 0xFF, F2 & 0xFF, F3 & 0xFF
    struct.pack_into('<hhh', c, 0x06, F4, F5, F6)
    struct.pack_into('<hhh', c, 0x0C, *F7)
    struct.pack_into('<hhh', c, 0x12, *F8)
    struct.pack_into('<h', c, 0x18, F9)
    b[4:] = c
    return bytes(b)


def tag_of(path):
    return level1.PEER if path.name.startswith('f0t1-') else level1.PROFILE


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--check', action='store_true', help='report only; exit 1 if any file differs')
    ap.add_argument('files', nargs='+', type=Path)
    a = ap.parse_args()
    look = look_image()
    differ = 0
    for path in a.files:
        path = path.resolve()  # d1-oracle runs in the workspace folder
        tag = tag_of(path)
        before = level1.decode(path, tag)
        current = before[LOOK_AT:LOOK_AT + LOOK_BYTES]
        print(f'{path.name}: identity {current[0]}/{current[1]}/{current[2]}, customization f0 {struct.unpack_from("<h", current, 4)[0]}')
        if current == look:
            continue
        differ += 1
        if a.check:
            continue
        b = bytearray(before)
        b[LOOK_AT:LOOK_AT + LOOK_BYTES] = look
        with tempfile.TemporaryDirectory() as t:
            out = Path(t) / path.name
            level1.encode(bytes(b), tag, out)
            after = level1.decode(out, tag)
            if after != bytes(b):
                sys.exit(f'{path}: the re-decoded object differs from the written one; not written')
            diff = [i for i, (x, y) in enumerate(zip(before, after)) if x != y]
            if diff and not (LOOK_AT <= min(diff) and max(diff) < LOOK_AT + LOOK_BYTES):
                sys.exit(f'{path}: bytes outside the look changed ({hex(min(diff))}..{hex(max(diff))}); not written')
            path.write_bytes(out.read_bytes())
        print(f'  written: race {RACE}, gender {GENDER}, class {CLASS} and the default customization ({len(diff)} bytes)')
    if a.check and differ:
        sys.exit(1)


if __name__ == '__main__':
    main()
