"""Dismantle a spare item in Tower North and check the server removed it (the 2 November recipe).

    python -I dismantle-test.py RUN_DIR --profile PROFILE --item ITEM_ELEMENT --soid HEX

Read-only on the client apart from the pad presses below; the server side is the responder's
402 handler (vendor::on_item_action). Steps, each with a screenshot in RUN_DIR/dismantle:
1. push PROFILE with ITEM_ELEMENT under SOID (push_profile.py): the post-game profile plus an
   unequipped spare, e.g. `d1-character-fixup.py profile IN OUT --item 94:3598:0xd10000000000030b`;
2. open the character screen (Options) and move the cursor onto Primary Weapons, where the spare
   shows beside the equipped rifle with "Equip / Dismantle";
3. hold Square for 3.5 s (Dismantle), then read the responder log for the 402 and the
   "DISMANTLE APPLIED" line, and take the screen again: the spare should be gone.
The cursor moves are the ones measured in run sense118 (Tower North, default window size).
"""
import argparse
import os
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
TOOLS = os.path.join(HERE, '..', '..', 'tools', 'ps4_setup')
sys.path.insert(0, TOOLS)
import d1_keys  # noqa: E402


def shot(out, name):
    subprocess.run(['powershell', '-NoProfile', '-File', os.path.join(TOOLS, 'd1-shot.ps1'), '-Out', os.path.join(out, name)],
                   capture_output=True)


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument('run_dir'); ap.add_argument('--profile', required=True)
    ap.add_argument('--item', required=True); ap.add_argument('--soid', required=True)
    ap.add_argument('--responder-log', help='the run\'s responder.log, to report the 402 and its result')
    a = ap.parse_args()
    sys.stdout.reconfigure(encoding='utf-8', errors='replace')   # the responder log has emoji; a cp1252 console can't print them
    out = os.path.join(a.run_dir, 'dismantle'); os.makedirs(out, exist_ok=True)
    subprocess.run([sys.executable, '-I', os.path.join(HERE, 'push_profile.py'), a.profile, '--item', a.item, '--soid', a.soid], check=True)
    time.sleep(3)
    d1_keys.press('Return'); time.sleep(3); shot(out, '1-character.png')
    d1_keys.press('Hold:A:500', 'Hold:W:150', gap=200); time.sleep(1); shot(out, '2-primary.png')
    d1_keys.press('Hold:NumPad4:3500'); time.sleep(3); shot(out, '3-after.png')
    if a.responder_log:
        lines = open(a.responder_log, encoding='utf-8', errors='replace').read().splitlines()
        for l in [l for l in lines if 'ITEM ACTION (402)' in l or 'DISMANTLE' in l][-4:]:
            print(l.strip()[:200])
    print('screens in', out)


if __name__ == '__main__':
    main()
