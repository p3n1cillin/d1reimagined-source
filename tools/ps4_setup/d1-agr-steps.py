"""Build the D1_AH_MISSION_STEPS files for A Guardian Rises' opening (56578, activity index 164).

The order is the 01.24 mission script (`missions/a_guardian_rises.json`): the revival cinematic
`pf_104_res`, then the first-person `pf_104_res_fp`, both stopped, then the host teleport from the
intro slice set 113 to gameplay 112 with "Enter the Wall" active.

Inputs are sensor-client lists captured with `d1-winguest.py sensor-clients --all --bundle` while
the client waits in each region (113 and 112), plus a list holding the objective states (the
`obj3s` list: objective 3 state 5 with its waypoint, incoming sequence 4).

Cinematic state `0x8080254D` (212 bytes on 56578, handler 0x403140):
  +0x04 play counter (a change starts/stops), +0x08 playing, +0x0C optional object ref
  ({0x811C9DC5, 0xFF, 0xFFFF} = absent; all-zero is "present" and the failed resolve crashes at
  0x126CE5), +0x14 team filter (-1 = off), +0x9C cast count, +0xA8 cast[0] = the local player's
  identity (kind 0x23, the spawn bundle's +0x40 bytes), not a machine id.

  d1-agr-steps.py --r113 LIST --r112 LIST --objectives LIST --out-dir DIR
"""
import argparse, os, struct
from pathlib import Path

ABSENT = (0x811C9DC5, 0xFF, 0xFFFF)
C1, C2 = ('0x992bb471', '0x6', '1'), ('0x992bb471', '0x6', '2')
CINE_TYPE = '0x8080254D'
STORY_GROUP = '0x4786c0e0'


def identity(lines):
    for l in lines:
        p = l.split()
        if len(p) > 6 and p[5].lower() == '0x808022a5':
            return bytes.fromhex(p[6])[0x40:0x48]
    raise SystemExit('no spawn bundle (A 0x808022A5) in the list')


def cine(counter, playing, ident):
    img = bytearray(0xD4)
    struct.pack_into('<I', img, 0x04, counter)
    img[0x08] = 1 if playing else 0
    struct.pack_into('<IBxH', img, 0x0C, *ABSENT)
    img[0x14] = 0xFF
    if playing:
        struct.pack_into('<I', img, 0x9C, 1)
        img[0xA8:0xB0] = ident
    return img.hex()


def patched(lines, states):
    out, hit = [], set()
    for l in lines:
        p = l.split()
        key = tuple(p[:3])
        if key in states:
            t, h = states[key]
            l = ' '.join(p[:5] + [t, h])
            hit.add(key)
        out.append(l)
    missing = set(states) - hit
    if missing:
        raise SystemExit(f'objects not in the list: {sorted(missing)}')
    return '\n'.join(out) + '\n'


def promote(steps, divide, output):
    """Promote an already verified full schedule, keeping its earlier steps intact.

    The input list is opaque here: build its objective images with d1-oracle.
    This does not turn an untested list into native acceptance.
    """
    text = steps.read_text()
    lines = text.splitlines(keepends=True)
    rows = [(i, line.split()) for i, line in enumerate(lines)
            if line.strip() and not line.lstrip().startswith('#')]
    crossings = [(i, row) for i, row in rows if len(row) >= 2 and row[1].startswith('%96|')]
    if len(crossings) != 1 or crossings[0] != rows[-1]:
        raise ValueError('Expected one final slice-96 step in the verified full schedule')
    index, row = crossings[0]
    if len(row) != 4 or row[0] != '164' or row[2].lower() != STORY_GROUP:
        raise ValueError('Unexpected final Divide step; no files written')
    if Path(row[3]).resolve() != divide.resolve():
        raise ValueError('Final step must already refer to the verified Divide list')
    target = output / 'agr-divide-objective17.txt'
    # Replace only the path, preserving trigger/timing and every prior step.
    lines[index] = lines[index].replace(row[3], target.resolve().as_posix(), 1)
    files = {target: divide.read_text(), output/'agr-steps-divide.txt': ''.join(lines)}
    for path, value in files.items():
        if path.exists() and path.read_text() != value:
            raise ValueError(f'Refusing to replace different existing content: {path}')
    output.mkdir(parents=True, exist_ok=True)
    for path, value in files.items():
        if not path.exists():
            with path.open('x') as stream:
                stream.write(value)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--r113')
    ap.add_argument('--r112')
    ap.add_argument('--objectives')
    ap.add_argument('--promote-steps', type=Path,
                    help='Verified full schedule ending at the natural slice-96 crossing')
    ap.add_argument('--divide-list', type=Path,
                    help='The objective-17 list already referenced by that verified final step')
    ap.add_argument('--out-dir', required=True)
    a = ap.parse_args()
    if a.promote_steps or a.divide_list:
        if not (a.promote_steps and a.divide_list) or any((a.r113, a.r112, a.objectives)):
            ap.error('Promotion requires --promote-steps and --divide-list only')
        promote(a.promote_steps, a.divide_list, Path(a.out_dir))
        print('Promoted verified schedule to agr-steps-divide.txt; no live control published')
        return
    if not all((a.r113, a.r112, a.objectives)):
        ap.error('Opening generation requires --r113, --r112 and --objectives')
    r113 = open(a.r113).read().splitlines()
    r112 = open(a.r112).read().splitlines()
    ident = identity(r113)
    objectives = {tuple(p[:3]): (p[5], p[6]) for p in (l.split() for l in open(a.objectives))
                  if len(p) > 6 and p[0] == '0x992bb471'}
    files = {
        'agr-step0.txt': patched(r113, {C1: (CINE_TYPE, cine(1, True, ident))}),
        'agr-step1.txt': patched(r113, {C1: (CINE_TYPE, cine(2, False, ident)), C2: (CINE_TYPE, cine(1, True, ident))}),
        'agr-step2.txt': patched(r113, {C1: (CINE_TYPE, cine(2, False, ident)), C2: (CINE_TYPE, cine(2, False, ident))}),
        'agr-step3.txt': patched(r112, objectives),
    }
    os.makedirs(a.out_dir, exist_ok=True)
    for name, text in files.items():
        open(os.path.join(a.out_dir, name), 'w').write(text)
    d = os.path.abspath(a.out_dir).replace('\\', '/')
    # Each cinematic ends on the client's stop (891) or skip request (1726; the host decides), or
    # after its length if the client never reports.
    steps = ('# A Guardian Rises (164): revival, first person, teleport 113 -> 112 (01.24 script order)\n'
             f'164  0             {STORY_GROUP}  {d}/agr-step0.txt\n'
             f'164  @891,1726|70  {STORY_GROUP}  {d}/agr-step1.txt\n'
             f'164  @891,1726|45  {STORY_GROUP}  {d}/agr-step2.txt\n'
             f'164  +5            {STORY_GROUP}  {d}/agr-step3.txt  112\n')
    open(os.path.join(a.out_dir, 'agr-steps.txt'), 'w').write(steps)
    print(f'wrote {len(files) + 1} files to {d}; objectives {sorted(objectives)}')


if __name__ == '__main__':
    main()
