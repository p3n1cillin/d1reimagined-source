"""Write the Breach's Fallen as the mission script places them: one sensor list per player trigger.

    python -I d1-breach-waves.py BASE_LIST OUT_DIR [--gen G] [--count N] [--timeout S] [--overlay LIST]

BASE_LIST is the list served before the waves, normally `agr-b-graveyard.txt`. OUT_DIR gets:
- `agr-bw-armed.txt`: every wave trigger armed (the script places no squad before its first trigger:
  wall l a, floor a and floor b only get `set_objective`, `enc_breach_m0.es:165-167`);
- `agr-bw-<trigger>.txt`, one per trigger;
- `agr-bw-steps.txt`, schedule lines to insert after the `agr-b-graveyard.txt` step of
  `agr-steps-divide.txt`.

The waves are Blam Network's Fukugen Daihon reconstruction of `enc_breach_m0.es`
(https://github.com/Blam-Network/tiger); see notes/ps4-56578-breach-sequences-2026-10-31.md.
Each `sleep_until` on a trigger places squads, and the trigger reports reach the host as incident
3403 (`^G/0x1D/I` steps).

`--overlay LIST` carries every row in which LIST differs from BASE_LIST into all the lists. The
unattended route uses it for the garage door (`breach-prop3-only.txt` opens prop 3), which it would
otherwise publish as a control list over the waves.

Each list is the whole state, so later lists carry the earlier waves:
- A squad that has had its wave keeps its generation, with one entry of count 0 (an explicit zero:
  with no entry the client places its own default). Its members stay, but a member killed is not
  replaced. With a count of 1 the squad refills, which is how run sense101 met the first wave
  again behind it.
- A trigger that has fired goes back to unarmed.
- The host keeps every trigger report until a step uses it, so a trigger crossed before its step
  is due still counts.

Entrances: the script's `enter(member, action, audio)` after a `place()` is the member's command
script (`__cs.scripted_sequence(action)`, `member.execute(__cs)`; the audio sequence is left out).
Each entered member gets that state (d1-member-states.py ACTION) and its squad an explicit zero
entry, so the squad adds no anonymous body beside the named actor. That squad keeps the same image
in later lists. `--no-entrances` writes the squads alone, as before.
"""
import argparse
import importlib.util
from pathlib import Path

BREACH = '0xfc0e45a3'
ARMED, UNARMED = '0100000001000000', '0000000000000000'
# Placed before any trigger: none (26/28/30 only get an objective in the script).
OPENING = ()
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

# Squads whose script gives @member_counts: their bodies, summed over the cells (variant 0 here).
MEMBER_COUNTS = {
    21: 1,   # first vandal {vandal: 1}
    55: 2,   # h {dreg: 1, dreg_1: 1}
    76: 3,   # garage exit {dregs: 1, dregs_1: 1, vandal: 1}
}
# enter(member, action, ...) per squad, as each wave's script calls it (enc_breach_m0.es:305-378).
ENTRANCES = {
    24: ((25, 'act_fallen_intro_vandal_drop'),),       # bouncedown c: vandal_blade drops from the ceiling
    38: ((39, 'act_fallen_vault_right'),),             # wall l c: the dreg vaults the pipes
    42: ((43, 'fallen_drop'),),                        # dropdown e
    56: ((57, 'fallen_drop'),),                        # 1c
    58: ((59, 'fallen_drop'), (60, 'fallen_drop')),    # 2: dreg_a, dreg_b (dreg_c is not entered)
    44: ((45, 'fallen_drop'), (46, 'fallen_drop')),    # dropdown f: dreg, dreg1
    47: ((48, 'act_fallen_intro_grate_climb'),),       # floor f
    49: ((50, 'act_fallen_vault_left'),),              # wall r f
    51: ((52, 'fallen_drop'),),                        # dropdown g
    53: ((54, 'act_fallen_vault_right'),),             # wall l g
    72: ((73, 'act_fallen_intro_vandal_drop'),),       # garage vandals 2
    74: ((75, 'act_fallen_intro_vandal_drop'),),       # garage vandals 3
    68: ((69, 'fallen_drop'), (70, 'fallen_drop'), (71, 'fallen_drop')),  # garage melee dreg1-3
}


def _load(name, file):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(file))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


squad_states = _load('squad_states', 'd1-squad-states.py')
member_states = _load('member_states', 'd1-member-states.py')


def render(base_lines, squads, triggers, members=None):
    """BASE with `squads` {index: state}, `triggers` {index: hex} and `members` {index: state}
    applied to the Breach rows."""
    members = members or {}
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
            elif kind == '0x2' and index in members:
                line = ' '.join(p[:5] + [member_states.TAG, members[index].hex()]) + '\n'
                hit.add(('member', index))
        out.append(line)
    missing = {('squad', i) for i in squads} | {('trigger', i) for i in triggers}
    missing |= {('member', i) for i in members}
    missing -= hit
    if missing:
        raise SystemExit(f'not in the base list: {sorted(missing)}')
    return ''.join(out)


def overlay(base_lines, other_lines):
    """BASE with every row that differs in OTHER replaced by OTHER's (same rows, same order)."""
    if len(base_lines) != len(other_lines):
        raise SystemExit('the overlay list must have the same rows as the base list')
    out = []
    for a, b in zip(base_lines, other_lines):
        if a.split()[:3] != b.split()[:3]:
            raise SystemExit(f'the overlay list differs in its rows: {a.split()[:3]} against {b.split()[:3]}')
        out.append(b if a.split() != b.split() else a)
    return out


def build(base_lines, gen=2, count=1, entrances=True):
    """[(name, text)]: the armed list, then one per wave."""
    live = lambda: squad_states.squad(gen, count)
    # An explicit zero: with no entry (+0x2C = 0) the client places its own default again.
    spent = lambda: squad_states.squad(gen, 0, entry=True)
    named = spent
    entered = ENTRANCES if entrances else {}
    squads = {i: live() for i in OPENING}
    members = {}
    triggers = {t: ARMED for t, _ in WAVES}
    lists = [('agr-bw-armed.txt', render(base_lines, squads, triggers, members))]
    for trigger, placed in WAVES:
        squads = {i: spent() for i in squads}
        for i in placed:
            if i in entered:
                squads[i] = named()
                members.update({m: member_states.member(gen, action) for m, action in entered[i]})
            elif entrances and i in MEMBER_COUNTS:
                squads[i] = squad_states.squad(gen, MEMBER_COUNTS[i])
            else:
                squads[i] = live()
        triggers[trigger] = UNARMED
        lists.append((f'agr-bw-{trigger}.txt', render(base_lines, squads, triggers, members)))
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
    # 900: fighting each stretch, sense108 took over five minutes from one wave trigger to the next,
    # and 300 s served the corridor's waves on their timeouts before the Guardian got there.
    ap.add_argument('--timeout', type=int, default=900, help='seconds each wave waits for its trigger')
    ap.add_argument('--overlay', help='a list whose differences from BASE every list carries (e.g. the garage door)')
    ap.add_argument('--no-entrances', action='store_true', help='squads only: no enter() command scripts')
    a = ap.parse_args()
    out = Path(a.out_dir)
    out.mkdir(parents=True, exist_ok=True)
    base = Path(a.base).read_text().splitlines(keepends=True)
    if a.overlay:
        base = overlay(base, Path(a.overlay).read_text().splitlines(keepends=True))
    for name, text in build(base, a.gen, a.count, not a.no_entrances):
        (out / name).write_text(text, newline='\n')
    (out / 'agr-bw-steps.txt').write_text(steps(a.timeout), newline='\n')
    print(f'{out}: {len(WAVES) + 1} lists and agr-bw-steps.txt')


if __name__ == '__main__':
    main()
