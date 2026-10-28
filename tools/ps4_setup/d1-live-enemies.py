"""One read-only active-squad snapshot (56578); --group accepts a namespace hash."""
import argparse
import contextlib
import importlib.util
import json
from pathlib import Path
import sys

from d1_squad_tracking import SquadReader, Unavailable


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--group', type=lambda s: int(s, 0))
    args = parser.parse_args()
    spec = importlib.util.spec_from_file_location('winguest', Path(__file__).with_name('d1-winguest.py'))
    guest = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(guest)
    with contextlib.redirect_stdout(sys.stderr):
        reader, base = guest.open_client()
    try:
        print(json.dumps(SquadReader(reader, base).snapshot(args.group), allow_nan=False))
    except (OSError, Unavailable) as exc:
        print(json.dumps({'unavailable': str(exc)}))
        return 2
    return 0


if __name__ == '__main__':
    sys.exit(main())
