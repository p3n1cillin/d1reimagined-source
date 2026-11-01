"""Run the checks CI runs, on your machine, before you open a pull request. No game files needed.

    python tools/check.py                       # everything that applies to your change
    python tools/check.py --python              # only the Python tests
    python tools/check.py --rust                # only the Rust workspace tests
    python tools/check.py --title "area: summary"   # also check the pull request title

It runs, from the repository root:
- the conventions check (`.github/scripts/check_pr.py`: no secrets, personal paths or broken
  relative links in what changed against `origin/main`);
- the Python unit tests CI runs (`server`, `scripts`, `tests`, `tools/content_capture`);
- `ruff` on the Python files you changed, if ruff is installed;
- the Rust workspace tests CI runs (`ps4/d1-server`), if cargo is installed.

Each step prints PASS, FAIL or SKIP (with the reason), and the exit code is non-zero if any failed.
Tests that need your own game inputs skip without them, as they do in CI.
"""
import argparse
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PY_DIRS = ('server', 'scripts', 'tests', 'tools/content_capture')
# The same exclusions as CI's rust-workspace job (.github/workflows/ci.yml).
CARGO_TEST = ['cargo', 'test', '--locked', '--no-fail-fast', '--workspace',
              '--exclude', 'd1-activity-host', '--exclude', 'd1-pkg', '--exclude', 'd1-live']
# The ruff rules CI's lint job reports.
RUFF_RULES = 'E9,F63,F7,F82,F401,F811,F841'


def run(label, args, cwd=ROOT):
    print(f'\n== {label}: {" ".join(args)}', flush=True)
    ok = subprocess.run(args, cwd=cwd).returncode == 0
    return label, 'PASS' if ok else 'FAIL'


def changed(base, pattern):
    out = subprocess.run(['git', 'diff', '--name-only', '--diff-filter=AM', f'{base}...HEAD', '--', pattern],
                         cwd=ROOT, capture_output=True, text=True)
    return out.stdout.split() if out.returncode == 0 else []


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument('--python', action='store_true', help='only the Python tests')
    ap.add_argument('--rust', action='store_true', help='only the Rust tests')
    ap.add_argument('--title', help='the pull request title, checked against `area: summary`')
    ap.add_argument('--base', default='origin/main', help='what your branch is compared with (default origin/main)')
    a = ap.parse_args()
    everything = not (a.python or a.rust)
    results = []

    if everything:
        args = [sys.executable, '.github/scripts/check_pr.py', '--base', a.base, '--head', 'HEAD']
        if a.title:
            args += ['--title', a.title]
        results.append(run('conventions', args))

    if everything or a.python:
        for d in PY_DIRS:
            results.append(run(f'python {d}', [sys.executable, '-m', 'unittest', 'discover', '-s', d, '-p', 'test_*.py']))
        files = changed(a.base, '*.py')
        if not files:
            results.append(('ruff', 'SKIP (no changed Python files)'))
        elif shutil.which('ruff'):
            results.append(run('ruff', ['ruff', 'check', '--no-cache', '--select', RUFF_RULES, *files]))
        else:
            results.append(('ruff', 'SKIP (install it with `pip install ruff`)'))

    if everything or a.rust:
        if shutil.which('cargo'):
            results.append(run('rust workspace', CARGO_TEST, ROOT / 'ps4' / 'd1-server'))
        else:
            results.append(('rust workspace', 'SKIP (no cargo; install Rust from https://rustup.rs)'))

    print('\n== summary')
    for label, verdict in results:
        print(f'  {verdict:<5} {label}')
    return 1 if any(v == 'FAIL' for _, v in results) else 0


if __name__ == '__main__':
    sys.exit(main())
