# SPDX-License-Identifier: GPL-2.0-or-later
"""Where the Tower route finds the repository's tools and fixtures and the lab's own folders.

The route's scripts live in lab/tower-route; tools come from tools/ps4_setup, served fixtures from
ps4/fixtures/56578 (D1_FIXTURES), and run output goes below the lab staging folder (D1_STAGING, see
tools/ps4_setup/d1paths.py). Probe output goes to STAGING/probes.
"""
import os
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
SETUP = REPO / 'tools' / 'ps4_setup'
ROUTES = REPO / 'missions' / 'routes'
if str(SETUP) not in sys.path:
    sys.path.insert(0, str(SETUP))
import d1paths  # noqa: E402

PAD = SETUP / 'd1-pad.ps1'
SHOT = SETUP / 'd1-shot.ps1'
WINGUEST = SETUP / 'd1-winguest.py'
WALK = SETUP / 'd1-walk.py'


def fixtures():
    return d1paths.fixtures()


def staging():
    return d1paths.staging()


def probes():
    """Probe output (JSON snapshots, monitors); created on first use."""
    out = staging() / 'probes'
    out.mkdir(parents=True, exist_ok=True)
    return out


def shad_log():
    return d1paths.shadps4() / 'user' / 'log' / 'shad_log.txt'


def eboot():
    """The owned 56578 executable for static reads: D1_EBOOT, else the workspace dump."""
    explicit = os.environ.get('D1_EBOOT')
    if explicit:
        return Path(explicit)
    return d1paths.workspace() / 'dumps' / 'later-56578-CUSA00219' / 'eboot.elf'
