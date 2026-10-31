import sys
from pathlib import Path
D = Path('F:/d1/public-work/wt-paths/tools/ps4_setup')


def load(f):
    t = (D / f).read_bytes().decode('utf-8')
    return t.replace('\r\n', '\n'), '\r\n' in t


def save(f, t, crlf):
    (D / f).write_bytes((t.replace('\n', '\r\n') if crlf else t).encode('utf-8'))


def sub1(t, old, new, count=1):
    n = t.count(old)
    if n != count:
        sys.exit(f'expected {count} x {old!r}, found {n}')
    return t.replace(old, new)

IMPORT = ("sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))\n"
          "import d1paths  # sibling: local paths from D1_ROOT or paths.json\n")


def edit(f, pairs):
    t, crlf = load(f)
    for old, new in pairs:
        t = sub1(t, old, new)
    if 'F:\\d1' in t or 'F:/d1' in t:
        sys.exit(f'{f}: literal left')
    save(f, t, crlf)


WG_OPEN = "src = open(r'F:\\d1\\tools\\d1-winguest.py', encoding='utf-8')"
WG_NEW = "src = open(d1paths.tools() / 'd1-winguest.py', encoding='utf-8')"

edit('d1-auth40.py', [('import struct, sys, time\n', 'import os, struct, sys, time\n' + IMPORT),
                      (WG_OPEN, WG_NEW)])
edit('d1-engstart.py', [('"""\nimport sys\n', '"""\nimport os, sys\n' + IMPORT), (WG_OPEN, WG_NEW)])
edit('d1-gsauth.py', [('"""\nimport sys\n', '"""\nimport os, sys\n' + IMPORT), (WG_OPEN, WG_NEW)])
edit('d1-bundle-poke.py', [('import sys, struct, time\n', 'import os, sys, struct, time\n' + IMPORT),
                           (WG_OPEN, WG_NEW)])
edit('d1-bubbles.py', [('import struct\n', 'import os, struct, sys\n' + IMPORT),
                       ("src=open(r'F:\\d1\\tools\\d1-winguest.py').read()",
                        "src=open(d1paths.tools() / 'd1-winguest.py').read()")])

edit('d1-inventory-renumber.py', [
    ("import os, re, struct, subprocess, sys\n\n"
     "ORACLE = r'F:\\d1\\d1preservation-main\\d1-server\\target\\release\\d1-oracle.exe'\n",
     "import os, re, struct, subprocess, sys\n\n" + IMPORT + "\n\n"
     "def oracle():\n"
     "    return str(d1paths.workspace() / 'd1-server' / 'target' / 'release' / 'd1-oracle.exe')\n\n\n"),
    ("subprocess.run([ORACLE, 'object', '--tag', TAG, '--decode'", "subprocess.run([oracle(), 'object', '--tag', TAG, '--decode'"),
    ("subprocess.run([ORACLE, 'object', '--tag', TAG, '--obj'", "subprocess.run([oracle(), 'object', '--tag', TAG, '--obj'"),
])

edit('d1-scenarios.py', [
    ("import argparse, os, re, struct, subprocess, sys, tempfile\n\n"
     "ORACLE = r'F:\\d1\\d1preservation-main\\d1-server\\target\\release\\d1-oracle.exe'\n"
     "PKGDIR = r'F:\\d1\\d1preservation-main\\dumps\\later-56578-CUSA00219\\CUSA00219\\packages'\n",
     "import argparse, os, re, struct, subprocess, sys, tempfile\n\n" + IMPORT + "\n\n"
     "def oracle(tool='d1-oracle'):\n"
     "    return str(d1paths.workspace() / 'd1-server' / 'target' / 'release' / f'{tool}.exe')\n\n\n"
     "def pkgdir():\n"
     "    return str(d1paths.workspace() / 'dumps' / 'later-56578-CUSA00219' / 'CUSA00219' / 'packages')\n\n\n"),
    ("subprocess.run([ORACLE, 'oodle'", "subprocess.run([oracle(), 'oodle'"),
    ("[ORACLE.replace('d1-oracle', 'd1-pkg'), 'info', pkg]", "[oracle('d1-pkg'), 'info', pkg]"),
    ("os.path.join(PKGDIR, pkg + '.pkg')", "os.path.join(pkgdir(), pkg + '.pkg')"),
    ("[ORACLE.replace('d1-oracle', 'd1-pkg'), 'entries', a.pkg]", "[oracle('d1-pkg'), 'entries', a.pkg]"),
])

edit('d1-winguest.py', [
    ("import ctypes, ctypes.wintypes as wt, re, struct, subprocess, sys\n",
     "import ctypes, ctypes.wintypes as wt, os, re, struct, subprocess, sys\n"),
    ("LOG = r'F:\\d1\\tools\\shadPS4\\0.19.0\\user\\log\\shad_log.txt'\n",
     "\n\ndef shad_log():\n"
     "    \"\"\"The emulator log, below D1_SHADPS4 (see d1paths.py). Other scripts exec this source without\n"
     "    __file__; d1paths.py then comes from beside the script that did.\"\"\"\n"
     "    here = os.path.dirname(os.path.abspath(globals().get('__file__') or sys.argv[0]))\n"
     "    if here not in sys.path:\n"
     "        sys.path.insert(0, here)\n"
     "    import d1paths\n"
     "    return str(d1paths.shadps4() / 'user' / 'log' / 'shad_log.txt')\n\n\n"),
    ("log = open(LOG, 'rb')", "log = open(shad_log(), 'rb')"),
])

edit('d1-walk.py', [
    ("PAD = os.environ.get('D1_PAD', r'F:\\d1\\tools\\d1-pad.ps1')\n",
     "PAD = os.environ.get('D1_PAD')   # else D1_TOOLS/d1-pad.ps1 (d1paths.py), resolved on first use\n"),
    ("    subprocess.run(['powershell', '-NoProfile', '-File', PAD, '-Keys', ','.join(keys), '-GapMilliseconds', str(gap)],",
     "    if PAD is None:\n"
     "        sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))\n"
     "        import d1paths\n"
     "        globals()['PAD'] = str(d1paths.tools() / 'd1-pad.ps1')\n"
     "    subprocess.run(['powershell', '-NoProfile', '-File', PAD, '-Keys', ','.join(keys), '-GapMilliseconds', str(gap)],"),
])

edit('d1-waypoint.py', [
    ("SHOT = os.environ.get('D1_SHOT', r'F:\\d1\\tools\\d1-shot.ps1')\n"
     "PAD = os.environ.get('D1_PAD', r'F:\\d1\\tools\\d1-pad.ps1')\n",
     "sys.path.insert(0, HERE)\n"
     "import d1paths  # sibling: local paths from D1_ROOT or paths.json\n"
     "SHOT = os.environ.get('D1_SHOT')   # else D1_TOOLS/d1-shot.ps1\n"
     "PAD = os.environ.get('D1_PAD')     # else D1_TOOLS/d1-pad.ps1\n"),
    ("subprocess.run(['powershell', '-NoProfile', '-File', PAD, '-Keys', keys]",
     "subprocess.run(['powershell', '-NoProfile', '-File', PAD or str(d1paths.tools() / 'd1-pad.ps1'), '-Keys', keys]"),
    ("subprocess.run(['powershell', '-NoProfile', '-File', SHOT, '-Out', path]",
     "subprocess.run(['powershell', '-NoProfile', '-File', SHOT or str(d1paths.tools() / 'd1-shot.ps1'), '-Out', path]"),
])

edit('d1-navfollow.py', [
    ("import argparse, importlib.util, json, math, os, subprocess, time\n",
     "import argparse, importlib.util, json, math, os, subprocess, sys, time\n"),
    ("SHOT = r'F:\\d1\\tools\\d1-shot.ps1'\n", "sys.path.insert(0, HERE)\nimport d1paths  # sibling: local paths from D1_ROOT or paths.json\n"),
    ("subprocess.run(['powershell', '-NoProfile', '-File', SHOT, '-Out', path]",
     "subprocess.run(['powershell', '-NoProfile', '-File', str(d1paths.tools() / 'd1-shot.ps1'), '-Out', path]"),
])

edit('d1-step.py', [
    ("import argparse, importlib.util, json, math, os, subprocess\n\nHERE = os.path.dirname(os.path.abspath(__file__))\n",
     "import argparse, importlib.util, json, math, os, subprocess, sys\n\nHERE = os.path.dirname(os.path.abspath(__file__))\n"
     "sys.path.insert(0, HERE)\nimport d1paths  # sibling: local paths from D1_ROOT or paths.json\n"),
    ("'-File', r'F:\\d1\\tools\\d1-shot.ps1', '-Out', a.shot]", "'-File', str(d1paths.tools() / 'd1-shot.ps1'), '-Out', a.shot]"),
])

edit('d1-agr-replay.py', [
    ("HERE = Path(__file__).resolve().parent\n",
     "HERE = Path(__file__).resolve().parent\nsys.path.insert(0, str(HERE))\n"
     "import d1paths  # sibling: local paths from D1_ROOT or paths.json\n"),
    ("    ap.add_argument('--pad', type=Path, default=Path(r'F:\\d1\\tools\\d1-pad.ps1'))\n",
     "    ap.add_argument('--pad', type=Path, help='pad driver (default: D1_TOOLS/d1-pad.ps1)')\n"),
    ("    a = ap.parse_args()\n", "    a = ap.parse_args()\n    if a.pad is None:\n        a.pad = d1paths.tools() / 'd1-pad.ps1'\n"),
])

edit('d1-record.py', [
    ("import argparse, glob, json, os, re, shutil, subprocess, sys, time\n",
     "import argparse, glob, json, os, re, shutil, subprocess, sys, time\n\n" + IMPORT),
    ("    ap.add_argument('--tap', default=os.environ.get('D1_AUDIO_TAP', r'F:\\d1\\tools\\shadPS4\\audio-tap'))\n",
     "    ap.add_argument('--tap', default=os.environ.get('D1_AUDIO_TAP'),\n"
     "                    help='audio tap folder (default: D1_AUDIO_TAP, else D1_TOOLS/shadPS4/audio-tap)')\n"),
    ("    a = ap.parse_args()\n", "    a = ap.parse_args()\n    if not a.tap:\n        a.tap = str(d1paths.tools() / 'shadPS4' / 'audio-tap')\n"),
])

edit('d1-scene.py', [
    ("    d1-scene.py OUT.png [--dump-dir F:/d1/tools/shadPS4/dump-live] [--wait 4]\n",
     "    d1-scene.py OUT.png [--dump-dir D1_TOOLS/shadPS4/dump-live] [--wait 4]\n"),
    ("import argparse, os, sys, time\n", "import argparse, os, sys, time\n"),
    ("TONEMAP = '_27d596fb07cd3321_'\n",
     "sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))\n"
     "import d1paths  # sibling: local paths from D1_ROOT or paths.json\n\n"
     "TONEMAP = '_27d596fb07cd3321_'\n"),
    ("    ap.add_argument('--dump-dir', default=r'F:\\d1\\tools\\shadPS4\\dump-live')\n",
     "    ap.add_argument('--dump-dir', help='frame dump folder (default: D1_TOOLS/shadPS4/dump-live)')\n"),
    ("    a = ap.parse_args()\n", "    a = ap.parse_args()\n    if not a.dump_dir:\n        a.dump_dir = str(d1paths.tools() / 'shadPS4' / 'dump-live')\n"),
])
print('py ok')
