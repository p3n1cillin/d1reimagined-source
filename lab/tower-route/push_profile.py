"""Push a character profile to the running client, optionally with one inventory item element.

    python -I push_profile.py PROFILE [--item ITEM_ELEMENT --soid HEX] [--timeout 60]

The responder watches the staging folder (d1paths staging): it sends `profile-push.bin` as a
family-4 profile delta and deletes it. When `profile-push.item` and `profile-push.soid` are there
too, it sends that inventory element (0x80801732) under that instance first. postgame5.sh pushes the
completed profile with the Arcadia this way; this script does the same outside it, for example to
give the Guardian an unequipped spare in the Tower (the dismantle test, 2 November).
"""
import argparse
import os
import shutil
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))


def staging():
    out = subprocess.run([sys.executable, os.path.join(HERE, '..', '..', 'tools', 'ps4_setup', 'd1paths.py'), 'staging'],
                         capture_output=True, text=True, check=True).stdout.strip()
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument('profile')
    ap.add_argument('--item', help='an inventory element (0x80801732) to send with the profile')
    ap.add_argument('--soid', help='the item instance, hex (e.g. d10000000000030b)')
    ap.add_argument('--timeout', type=float, default=60)
    a = ap.parse_args()
    if bool(a.item) != bool(a.soid):
        sys.exit('--item and --soid go together')
    c = staging()
    push = os.path.join(c, 'profile-push.bin')
    if os.path.exists(push):
        sys.exit(f'a push is already waiting: {push}')
    if a.item:
        shutil.copy(a.item, os.path.join(c, 'profile-push.item'))
        with open(os.path.join(c, 'profile-push.soid'), 'w') as f:
            f.write(a.soid.lower().removeprefix('0x'))
    shutil.copy(a.profile, push)
    end = time.time() + a.timeout
    while os.path.exists(push) and time.time() < end:
        time.sleep(0.5)
    if os.path.exists(push):
        sys.exit(f'not taken in {a.timeout:.0f} s: is the responder running with profile pushes on?')
    print('pushed', os.path.basename(a.profile), f'with item {a.soid}' if a.item else '')


if __name__ == '__main__':
    main()
