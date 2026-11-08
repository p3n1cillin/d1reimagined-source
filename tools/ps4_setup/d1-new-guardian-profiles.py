# SPDX-License-Identifier: GPL-2.0-or-later
"""Build served profiles with the owned 56578 creation defaults instead of a later save's.

    d1-new-guardian-profiles.py [--fixtures DIR] [--out DIR] [--keep SUB,SUB,...]

Every profile a run serves carries the completion record (field [11]) of a later save: 94 subs at
boot, among them Restoration's own (sub 42), most of the Earth story and later content.
The broad served unlock flags separately reveal destinations such as Venus (run sense125).
Creation is NOT an empty record: the content seeds flags of its own.
The default applies new-guardian-defaults.json before the mission's changes below.
--empty-record retains the old empty-record diagnostic. This writes, through d1-profile-completions.py
(the client's codec):
- characters/profile-tutorial-t.bin    boot                   --clear
- grants/profile-tutorial-th.bin       the Breach rifle grant --clear --set 615=2 (the Khvostov, flag 2503)
- postgame/profile-active164-xp0       mid-mission pushes     --clear --set 615=2
- postgame/profile-active164-xp900
- postgame/arcadia-profile-xp900-owned after the outro        --clear --set 40=2 --set 615=2 --set 673=1
  (A Guardian Rises complete, and the CONTINUE gate's post-game flag 2838 at 1)
and copies the rest of the postgame folder (the Arcadia item, the scoreboard) beside them.

--keep is a lab stand-in that keeps the listed subs completed (2) in all five, before each
profile's own settings. The empty
record shows seven announcements at the first orbit and, forced into the mission, ended in orbit
(notes/ps4-56578-new-guardian-2026-11-03.md); keeping the markers 673-2188 instead started A Guardian
Rises' opening by itself in an orbit boot.

Serve it with ending-run.ps1 -PostgameDir OUT/postgame, and with -ExtraEnv setting D1_PROFILE_FILE
to OUT/characters/profile-tutorial-t.bin and D1_LOOT_GRANT to the grant with OUT's
grants/profile-tutorial-th.bin (later values win). With the empty record the route stays in orbit
(run newguardian-run1); with --keep 673,731,...,2188 (the markers r2 kept; the notes list them) run
newguardian-keep1 played A Guardian Rises from the opening to the Tower, unattended.

Apply OUT/new-guardian.env as well: it replaces the later Guardian's Director/level grants with
the existing lab access core plus the selected identity's content-authored account flags.
It is lab permission policy, not a claim about retail new-account ownership. Use
d1-level1-fixtures.py separately for level-1 gear/cards; this tool edits only completion state.
"""
import argparse
import os
import shutil
import subprocess
import sys
import json
from pathlib import Path
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import d1paths  # sibling: local paths from D1_ROOT or paths.json
from d1_new_guardian import select_defaults

HERE = os.path.dirname(os.path.abspath(__file__))
PROFILES = [
    ('characters/profile-tutorial-t.bin', ['--clear']),
    ('characters/grants/profile-tutorial-th.bin', ['--clear', '--set', '615=2']),
    ('postgame/profile-active164-xp0.draft.bin', ['--clear', '--set', '615=2']),
    ('postgame/profile-active164-xp900.draft.bin', ['--clear', '--set', '615=2']),
    ('postgame/arcadia-profile-xp900-owned.draft.bin', ['--clear', '--set', '40=2', '--set', '615=2', '--set', '673=1']),
]

# Existing lab permission/ownership core, NOT retail new-account entitlements. These are
# explicitly inherited lab policy (d1-lab-served.ps1), separate from authored character state.
# The extra Director/level grants in full-game runs must not leak into a new-Guardian setup.
LAB_ACCESS_FLAGS = (98, 419, 440, 834, 2668, 5021, 5998, 6029)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('--fixtures', default=str(d1paths.fixtures()))
    ap.add_argument('--out', default=os.path.join(str(d1paths.staging()), 'new-guardian'))
    ap.add_argument('--keep', default='', help='comma-separated subs kept completed in every profile')
    ap.add_argument('--defaults', type=Path, default=d1paths.fixtures() / 'characters/new-guardian-defaults.json')
    ap.add_argument('--empty-record', action='store_true', help='diagnostic stand-in: omit the authored creation flags')
    a = ap.parse_args()
    if Path(a.out).resolve().is_relative_to(Path(a.fixtures).resolve()):
        ap.error('--out must be outside the source fixtures')
    creation_args = [] if a.empty_record else ['--creation-defaults', str(a.defaults)]
    keep = [f'{int(x)}=2' for x in a.keep.split(',') if x.strip()]
    os.makedirs(os.path.join(a.out, 'postgame'), exist_ok=True)
    for name in os.listdir(os.path.join(a.fixtures, 'postgame')):
        src = os.path.join(a.fixtures, 'postgame', name)
        if os.path.isfile(src):
            shutil.copyfile(src, os.path.join(a.out, 'postgame', name))
    for rel, args in PROFILES:
        out = os.path.join(a.out, rel.replace('characters/grants/', 'grants/'))
        os.makedirs(os.path.dirname(out), exist_ok=True)
        r = subprocess.run([sys.executable, '-I', os.path.join(HERE, 'd1-profile-completions.py'),
                            os.path.join(a.fixtures, rel), out, args[0], *creation_args,
                            *[x for k in keep for x in ('--set', k)], *args[1:]],
                           capture_output=True, text=True)
        if r.returncode:
            sys.exit(f'{rel}: {r.stdout[-400:]} {r.stderr[-400:]}')
        print(f'{rel} -> {out}: {r.stdout.strip().splitlines()[-1] if r.stdout.strip() else "written"}')
    if not a.empty_record:
        # Read the identity with the existing codec; never assume the fixture's race/class.
        oracle = os.environ.get('D1_ORACLE') or str(d1paths.workspace() / 'd1-server/target/release/d1-oracle.exe')
        with tempfile.TemporaryDirectory() as tmp:
            obj = Path(tmp) / 'profile.obj'
            subprocess.run([oracle, 'object', '--tag', '0x80801743', '--decode-file',
                            str(Path(a.fixtures) / PROFILES[0][0]), '--out-obj', str(obj)],
                           capture_output=True, text=True, check=True)
            profile = obj.read_bytes()
        flags = select_defaults(json.loads(a.defaults.read_text(encoding='utf-8')), profile[0xA], profile[8])
        # Profile kind-3 flags are carried by each profile, not by repeated 205/503 overrides:
        # the latter could put the creation value back over the outro's 673=1.
        account = [f['index'] for f in flags if f['kind'] in (1, 2) and f['value'] == 2]
        if any(f['kind'] in (1, 2) and f['value'] != 2 for f in flags):
            sys.exit('the current D1_UNLOCK_FLAGS interface cannot express this account default')
        indices = list(dict.fromkeys((*LAB_ACCESS_FLAGS, *account)))
        (Path(a.out) / 'new-guardian.env').write_text(
            '# Lab access permissions plus content-authored account creation flags.\n'
            '# Apply alongside ALL five generated profiles; this replaces the broad Director grants.\n'
            'D1_UNLOCK_FLAGS=' + ','.join(map(str, indices)) + '\n', encoding='utf-8')


if __name__ == '__main__':
    main()
