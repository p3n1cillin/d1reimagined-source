# SPDX-License-Identifier: GPL-2.0-or-later
"""d1paths - where the local D1 lab files live, without hardcoding anyone's machine.

Every location resolves lazily, so importing this module never fails. Sources, first match wins:

  1. an environment variable (D1_ROOT, D1_TOOLS, ...);
  2. the same name, lowercase without the D1_ prefix ("root", "tools", ...), in the per-user
     config file: %APPDATA%\\d1reimagined\\paths.json on Windows,
     ${XDG_CONFIG_HOME:-~/.config}/d1reimagined/paths.json elsewhere (D1_PATHS_CONFIG overrides);
  3. the default, relative to another location (below).

  root         D1_ROOT         (required unless every location you use is set directly)
  tools        D1_TOOLS        root/tools
  shadps4      D1_SHADPS4      tools/shadPS4/0.19.0
  liborbispkg  D1_LIBORBISPKG  tools/liborbispkg/0.2.245
  workspace    D1_WORKSPACE    root/d1preservation-main
  staging      D1_STAGING      workspace/cfw-staging
  pkg_dir      D1_PKG_DIR      root/01.29/d1shad

Use from a sibling script (works under `python -I`, which drops the script directory):

    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import d1paths
    log = d1paths.shadps4() / 'user' / 'log' / 'shad_log.txt'

`python d1paths.py` prints every location and where it came from. D1Paths.ps1 is the
PowerShell twin and must resolve the same way.
"""
import json, os, sys
from pathlib import Path

# name -> (parent location or None, default path below the parent)
LOCATIONS = {
    'root': (None, None),
    'tools': ('root', 'tools'),
    'shadps4': ('tools', 'shadPS4/0.19.0'),
    'liborbispkg': ('tools', 'liborbispkg/0.2.245'),
    'workspace': ('root', 'd1preservation-main'),
    'staging': ('workspace', 'cfw-staging'),
    'pkg_dir': ('root', '01.29/d1shad'),
}


class D1PathError(SystemExit):
    """A location could not be resolved. Exits with the message when uncaught."""


def env_name(name):
    return 'D1_' + name.upper()


def config_file():
    explicit = os.environ.get('D1_PATHS_CONFIG')
    if explicit:
        return Path(explicit)
    if os.name == 'nt':
        base = os.environ.get('APPDATA') or os.path.join(os.path.expanduser('~'), 'AppData', 'Roaming')
    else:
        base = os.environ.get('XDG_CONFIG_HOME') or os.path.join(os.path.expanduser('~'), '.config')
    return Path(base) / 'd1reimagined' / 'paths.json'


def _config():
    path = config_file()
    try:
        with open(path, encoding='utf-8-sig') as f:
            data = json.load(f)
    except FileNotFoundError:
        return {}
    except (OSError, ValueError) as e:
        raise D1PathError(f'd1paths: cannot read {path}: {e}')
    if not isinstance(data, dict):
        raise D1PathError(f'd1paths: {path} must hold a JSON object, e.g. {{"root": "D:/d1"}}')
    return data


def _clean(value):
    return Path(os.path.abspath(os.path.expanduser(os.path.expandvars(str(value)))))


def resolve(name):
    """Return (Path, source) for a location; source is 'env', 'config' or 'default'."""
    if name not in LOCATIONS:
        raise KeyError(f'unknown D1 location {name!r} (known: {", ".join(LOCATIONS)})')
    value = os.environ.get(env_name(name))
    if value:
        return _clean(value), 'env'
    value = _config().get(name)
    if value:
        return _clean(value), 'config'
    parent, below = LOCATIONS[name]
    if parent is None:
        raise D1PathError(
            'd1paths: the D1 root folder is not set. Either set the environment variable D1_ROOT\n'
            '(e.g. D1_ROOT=D:/d1), or create ' + str(config_file()) + ' containing\n'
            '{"root": "D:/d1"} (see tools/ps4_setup/paths.example.json). Individual locations\n'
            'can be set the same way: ' + ', '.join(env_name(n) for n in LOCATIONS if n != 'root') + '.')
    return resolve(parent)[0].joinpath(*below.split('/')), 'default'


def get(name):
    return resolve(name)[0]


def root(): return get('root')
def tools(): return get('tools')
def shadps4(): return get('shadps4')
def liborbispkg(): return get('liborbispkg')
def workspace(): return get('workspace')
def staging(): return get('staging')
def pkg_dir(): return get('pkg_dir')


def main():
    print(f'config file: {config_file()}' + ('' if config_file().is_file() else ' (absent)'))
    failed = False
    for name in LOCATIONS:
        try:
            path, source = resolve(name)
        except D1PathError as e:
            print(f'{name:12} {env_name(name):15} unresolved')
            failed = e
            continue
        print(f'{name:12} {env_name(name):15} {source:8} {path}')
    if failed:
        print(failed.code, file=sys.stderr)
        return 2
    return 0


if __name__ == '__main__':
    sys.exit(main())
