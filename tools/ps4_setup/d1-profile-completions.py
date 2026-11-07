# SPDX-License-Identifier: GPL-2.0-or-later
"""Set a served profile's completion record (field [11]) through the client's own codec.

    d1-profile-completions.py IN OUT [--clear] [--set SUB=VALUE ...] [--list]

The family-4/1 profile (`0x80801743`) holds one byte per completion flag in field [11], 2200 bytes
from `+0x53C0` (sub N = byte N; 2 = completed, 1 = in progress). The Director's Earth nodes read
them: Restoration's own completion is sub 42 (flag 132), A Guardian Rises' are subs 40 and 41
(flags 130, 131; activities 164 and 165). Writes a new file and never edits in place; the edited
object must round-trip through the client's encoder.

The post-game profile served after A Guardian Rises (postgame/arcadia-profile-xp900-owned) carried
the 92 completions of a later save (every Earth story mission, Restoration included) and not
A Guardian Rises' own, so the Director would offer what a new Guardian could not yet play.
`--clear --set 40=2 --set 615=2 --set 673=1` gives the record right after A Guardian Rises:
the mission, the Khvostov (flag 2503) and the post-game flag 2838 that CONTINUE's gate reads.
"""
import argparse
import os
import subprocess
import sys
import tempfile
import json
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import d1paths  # sibling: local paths from D1_ROOT or paths.json
from d1_new_guardian import select_defaults

TAG = '0x80801743'
FIELD11, FIELD11_BYTES = 0x53C0, 2200


def oracle():
    return os.environ.get('D1_ORACLE') or str(d1paths.workspace() / 'd1-server' / 'target' / 'release' / 'd1-oracle.exe')


def run(*args):
    r = subprocess.run([oracle(), *args], capture_output=True, text=True)
    if r.returncode:
        sys.exit(f'd1-oracle {args[0]} failed: {r.stdout[-400:]} {r.stderr[-400:]}')
    return r.stdout


def subs(obj):
    return {i: obj[FIELD11 + i] for i in range(FIELD11_BYTES) if obj[FIELD11 + i]}


def apply_creation_defaults(obj, document):
    """Clear the old save, then seed the selected race/class's authored profile flags.

    Account flags are separate: copying their subindices into this field would corrupt
    unrelated character completions. The generator emits them in its environment snippet.
    """
    flags = select_defaults(document, obj[0xA], obj[8])
    edits = {}
    for flag in flags:
        if flag['kind'] != 3:
            continue
        sub, value = flag['sub'], flag['value']
        if not 0 <= sub < FIELD11_BYTES or not 0 <= value <= 2 or sub in edits:
            raise ValueError('invalid creation completion')
        edits[sub] = value
    obj[FIELD11:FIELD11 + FIELD11_BYTES] = bytes(FIELD11_BYTES)
    for sub, value in edits.items():
        obj[FIELD11 + sub] = value


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument('input')
    ap.add_argument('output', nargs='?')
    ap.add_argument('--clear', action='store_true', help='clear every completion first')
    ap.add_argument('--set', action='append', default=[], metavar='SUB=VALUE')
    ap.add_argument('--list', action='store_true', help='print the record and stop')
    ap.add_argument('--creation-defaults', type=Path,
                    help='clear the later save and apply the owned 56578 creation flags for this profile identity')
    a = ap.parse_args()
    if a.output and Path(a.input).resolve() == Path(a.output).resolve():
        ap.error('input and output must differ')
    with tempfile.TemporaryDirectory() as d:
        obj_path = os.path.join(d, 'in.obj')
        run('object', '--tag', TAG, '--decode-file', a.input, '--out-obj', obj_path)
        obj = bytearray(open(obj_path, 'rb').read())
        if a.list or not a.output:
            print(len(subs(obj)), 'completions:', subs(obj))
            return
        if a.clear:
            obj[FIELD11:FIELD11 + FIELD11_BYTES] = bytes(FIELD11_BYTES)
        if a.creation_defaults:
            apply_creation_defaults(obj, json.loads(a.creation_defaults.read_text(encoding='utf-8')))
        for item in a.set:
            sub, value = (int(x, 0) for x in item.split('='))
            if not 0 <= sub < FIELD11_BYTES or not 0 <= value < 256:
                sys.exit(f'bad --set {item}')
            obj[FIELD11 + sub] = value
        new = os.path.join(d, 'new.obj')
        open(new, 'wb').write(obj)
        out = run('object', '--tag', TAG, '--obj-file', new, '--out', a.output)
        if 'IDENTICAL' not in out:
            os.remove(a.output) if os.path.exists(a.output) else None
            sys.exit(f'the client codec did not round-trip the edited object: {out[-400:]}')
        run('object', '--tag', TAG, '--decode-file', a.output, '--out-obj', obj_path)
        if Path(obj_path).read_bytes() != obj:
            os.remove(a.output)
            sys.exit('re-decoded output differs from the edited profile')
    print(f'{a.output}: {len(subs(obj))} completions: {subs(obj)}; round trip identical')


if __name__ == '__main__':
    main()
