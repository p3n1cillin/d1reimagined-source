"""Write the Breach's Fallen as the mission script places them: one sensor list per player trigger.

    python -I d1-breach-waves.py BASE_LIST OUT_DIR [--gen G] [--count N] [--timeout S]

BASE_LIST is the list served before the waves, normally `agr-b-graveyard.txt`. OUT_DIR gets:
- `agr-bw-armed.txt`: every wave trigger armed, and the opening squads live;
- `agr-bw-<trigger>.txt`, one per trigger;
- `agr-bw-steps.txt`, schedule lines to insert after the `agr-b-graveyard.txt` step of
  `agr-steps-divide.txt`.

The waves are Blam Network's Fukugen Daihon reconstruction of `enc_breach_m0.es`
(https://github.com/Blam-Network/tiger); see notes/ps4-56578-breach-sequences-2026-10-31.md.
Each `sleep_until` on a trigger places squads, and the trigger reports reach the host as incident
3403 (`^G/0x1D/I` steps).

Each list is the whole state, so later lists carry the earlier waves:
- A squad that has had its wave keeps its generation, with a desired count of 0. Its members stay,
  but a member killed is not replaced. With a count of 1 the squad refills, which is how run
  sense101 met the first wave again behind it.
- A trigger that has fired goes back to unarmed.
- The host keeps every trigger report until a step uses it, so a trigger crossed before its step
  is due still counts.
"""
import argparse
import importlib.util
from pathlib import Path

BREACH = '0xfc0e45a3'
ARMED, UNARMED = '0100000001000000', '0000000000000000'
# Placed when the encounter starts: wall l a, floor a, floor b.
OPENING = (26, 28, 30)
# (trigger, squads) in the order the route meets them.
WAVES = (
    (108, (21,)),          # pt_fallen_1b_spawn: first vandal
    (110, (24, 38)),       # pt_fallen_c_spawn: bouncedown c, wall l c
    (111, (42,)),          # pt_fallen_melee: dropdown e
    (112, (56, 58)),       # pt_fallen_arena_spawn: 1c, 2 (the bridge rush)
    (114, (44,)),          # pt_corr_spawn_b: dropdown f
    (115, (47, 49)),       # pt_corr_spawn_b_melee: floor f, wall r f
    (116, (51, 53, 55)),   # pt_corr_spawn_b_rear: dropdown g, wall l g, h
    (106, (72, 74, 68)),   # pt_fallen_vandal_spawn: the garage vandals and melee
    (107, (76,)),          # pt_fallen_3_spawn: garage exit
)

_spec = importlib.util.spec_from_file_location('squad_states', Path(__file__).with_name('d1-squad-states.py'))
squad_states = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(squad_states)


def render(base_lines, squads, triggers):
    """BASE with `squads` {index: state} and `triggers` {index: hex} applied to the Breach rows."""
    out, hit = [], set()
    for line in base_lines:
        p = line.split()
        if len(p) >= 5 and p[0].lower() == BREACH:
            kind, index = p[1].lower(), int(p[2])
            if kind == '0x1' and index in squads:
                line = ' '.join(p[:5] + ['0x8080071E', squads[index].hex()]) + '\n'
                hit.add(('squad', index))
            elif kind == '0x1d' and index in triggers:
                line = ' '.join(p[:5] + ['0x80800463', triggers[index]]) + '\n'
                hit.add(('trigger', index))
        out.append(line)
    missing = {('squad', i) for i in squads} | {('trigger', i) for i in triggers}
    missing -= hit
    if missing:
        raise SystemExit(f'not in the base list: {sorted(missing)}')
    return ''.join(out)


def build(base_lines, gen=2, count=1):
    """[(name, text)]: the armed list, then one per wave."""
    live = lambda: squad_states.squad(gen, count)
    spent = lambda: squad_states.squad(gen, 0)
    squads = {i: live() for i in OPENING}
    triggers = {t: ARMED for t, _ in WAVES}
    lists = [('agr-bw-armed.txt', render(base_lines, squads, triggers))]
    for trigger, placed in WAVES:
        squads = {i: spent() for i in squads}
        squads.update({i: live() for i in placed})
        triggers[trigger] = UNARMED
        lists.append((f'agr-bw-{trigger}.txt', render(base_lines, squads, triggers)))
    return lists


def steps(timeout, activity=164, hash_='0x4786c0e0'):
    lines = [f'{activity} +2 {hash_} agr-bw-armed.txt']
    lines += [f'{activity} ^{BREACH}/0x1d/{t}|{timeout} {hash_} agr-bw-{t}.txt' for t, _ in WAVES]
    return '\n'.join(lines) + '\n'


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument('base')
    ap.add_argument('out_dir')
    ap.add_argument('--gen', type=int, default=2, help='generation for every Breach squad (default 2)')
    ap.add_argument('--count', type=int, default=1, help='desired members per squad (default 1)')
    ap.add_argument('--timeout', type=int, default=300, help='seconds each wave waits for its trigger')
    a = ap.parse_args()
    out = Path(a.out_dir)
    out.mkdir(parents=True, exist_ok=True)
    base = Path(a.base).read_text().splitlines(keepends=True)
    for name, text in build(base, a.gen, a.count):
        (out / name).write_text(text, newline='\n')
    (out / 'agr-bw-steps.txt').write_text(steps(a.timeout), newline='\n')
    print(f'{out}: {len(WAVES) + 1} lists and agr-bw-steps.txt')


if __name__ == '__main__':
    main()
