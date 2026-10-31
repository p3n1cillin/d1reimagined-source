import json, os, re, subprocess
W = 'F:/d1/public-work/wt-paths'
files = subprocess.run(['git', '-C', W, 'diff', '--name-only', '--', '*.ps1'], capture_output=True, text=True).stdout.split()
CALL = re.compile(r"""(?:&\s*"\$PSScriptRoot\\D1Paths\.ps1"|Get-D1Path)\s+(\w+)(?:\s+'([^']*)'|\s+(\$\w+))?""")
old_lits, calls = set(), []
for f in files:
    old = subprocess.run(['git', '-C', W, 'show', 'HEAD:' + f], capture_output=True, text=True, encoding='utf-8').stdout
    old = old.replace('F:\\d1\tools', 'F:\\d1\\tools')
    for m in re.finditer(r"F:\\d1\\[^'\"\s,)]*", old):
        old_lits.add(m.group(0).rstrip('\\').lower())
    new = open(os.path.join(W, f), encoding='utf-8').read()
    for m in CALL.finditer(new):
        if m.group(1) in ('Name',):
            continue
        calls.append((f, m.group(1), m.group(2) or '', m.group(3)))
names = sorted({(n, c) for _, n, c, _ in calls})
script = '. "F:/d1/public-work/wt-paths/tools/ps4_setup/D1Paths.ps1"\n' + ''.join(
    f"Get-D1Path {n} {(chr(39) + c + chr(39)) if c else ''}\n" for n, c in names)
env = dict(os.environ, D1_ROOT='F:/d1')
for k in list(env):
    if k.startswith('D1_') and k != 'D1_ROOT':
        del env[k]
out = subprocess.run(['pwsh', '-NoProfile', '-Command', script], capture_output=True, text=True, env=env).stdout.splitlines()
assert len(out) == len(names), (out, names)
res = dict(zip(names, out))
bad = 0
for f, n, c, var in calls:
    p = res[(n, c)]
    ok = p.lower() in old_lits
    print(('OK  ' if ok else 'BAD ') + f'{os.path.basename(f):32} {n} {c or var or ""} -> {p}')
    bad += not ok
print('mismatches:', bad)
