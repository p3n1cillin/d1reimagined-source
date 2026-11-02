"""Build the served profiles with a new Guardian's completion record instead of a later save's.

    d1-new-guardian-profiles.py [--fixtures DIR] [--out DIR]

Every profile a run serves carries the completion record (field [11]) of a later save: 94 subs at
boot, among them Restoration's own (sub 42), most of the Earth story and later content. So the
Director lights up the Earth story, and orbit shows "Destination revealed: Venus" after A Guardian
Rises (run sense125). A new Guardian has none of them. This writes, through d1-profile-completions.py
(the client's codec):
- characters/profile-tutorial-t.bin    boot                   --clear
- grants/profile-tutorial-th.bin       the Breach rifle grant --clear --set 615=2 (the Khvostov, flag 2503)
- postgame/profile-active164-xp0       mid-mission pushes     --clear --set 615=2
- postgame/profile-active164-xp900
- postgame/arcadia-profile-xp900-owned after the outro        --clear --set 40=2 --set 615=2 --set 673=1
  (A Guardian Rises complete, and the CONTINUE gate's post-game flag 2838 at 1)
and copies the rest of the postgame folder (the Arcadia item, the scoreboard) beside them.

Serve it with ending-run.ps1 -PostgameDir OUT/postgame, and with -ExtraEnv setting D1_PROFILE_FILE
to OUT/characters/profile-tutorial-t.bin and D1_LOOT_GRANT to the grant with OUT's
grants/profile-tutorial-th.bin (later values win). Untested in a run: the 94 subs may hold
something the route relies on, so the first run with it is the test.
"""
import argparse
import os
import shutil
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import d1paths  # sibling: local paths from D1_ROOT or paths.json

HERE = os.path.dirname(os.path.abspath(__file__))
PROFILES = [
    ('characters/profile-tutorial-t.bin', ['--clear']),
    ('characters/grants/profile-tutorial-th.bin', ['--clear', '--set', '615=2']),
    ('postgame/profile-active164-xp0.draft.bin', ['--clear', '--set', '615=2']),
    ('postgame/profile-active164-xp900.draft.bin', ['--clear', '--set', '615=2']),
    ('postgame/arcadia-profile-xp900-owned.draft.bin', ['--clear', '--set', '40=2', '--set', '615=2', '--set', '673=1']),
]


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('--fixtures', default=str(d1paths.fixtures()))
    ap.add_argument('--out', default=os.path.join(str(d1paths.staging()), 'new-guardian'))
    a = ap.parse_args()
    os.makedirs(os.path.join(a.out, 'postgame'), exist_ok=True)
    for name in os.listdir(os.path.join(a.fixtures, 'postgame')):
        src = os.path.join(a.fixtures, 'postgame', name)
        if os.path.isfile(src):
            shutil.copyfile(src, os.path.join(a.out, 'postgame', name))
    for rel, args in PROFILES:
        out = os.path.join(a.out, rel.replace('characters/grants/', 'grants/'))
        os.makedirs(os.path.dirname(out), exist_ok=True)
        r = subprocess.run([sys.executable, '-I', os.path.join(HERE, 'd1-profile-completions.py'),
                            os.path.join(a.fixtures, rel), out, *args], capture_output=True, text=True)
        if r.returncode:
            sys.exit(f'{rel}: {r.stdout[-400:]} {r.stderr[-400:]}')
        print(f'{rel} -> {out}: {r.stdout.strip().splitlines()[-1] if r.stdout.strip() else "written"}')


if __name__ == '__main__':
    main()
