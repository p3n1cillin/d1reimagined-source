import re, sys
from pathlib import Path

D = Path('F:/d1/public-work/wt-paths/tools/ps4_setup')

# literal prefix (after F:\d1\) -> location name, longest first
PREFIXES = [
    ('d1preservation-main\\cfw-staging', 'Staging'),
    ('d1preservation-main', 'Workspace'),
    ('tools\\shadPS4\\0.19.0', 'ShadPS4'),
    ('tools\\liborbispkg\\0.2.245', 'LibOrbisPkg'),
    ('tools', 'Tools'),
    ('01.29\\d1shad', 'PkgDir'),
    ('', 'Root'),
]


def split(rest):
    """rest = path after 'F:\\d1\\' -> (Name, child or '')."""
    for pre, name in PREFIXES:
        if pre == '' or rest == pre or rest.startswith(pre + '\\'):
            child = rest[len(pre):].lstrip('\\')
            return name, child
    raise ValueError(rest)


def call(rest, invoke=False):
    name, child = split(rest)
    args = name + (f" '{child}'" if child else '')
    return f'(& "$PSScriptRoot\\D1Paths.ps1" {args})' if invoke else f'(Get-D1Path {args})'


def load(f):
    b = (D / f).read_bytes()
    t = b.decode('utf-8')
    crlf = '\r\n' in t
    return t.replace('\r\n', '\n'), crlf


def save(f, t, crlf):
    if crlf:
        t = t.replace('\n', '\r\n')
    (D / f).write_bytes(t.encode('utf-8'))


def sub1(t, old, new, count=1):
    n = t.count(old)
    if n != count:
        sys.exit(f'expected {count} x {old!r}, found {n}')
    return t.replace(old, new)


PARAM_RE = re.compile(r"^(\s*(?:\[[^\]]+\])+\$\w+ = )'F:\\d1\\([^']*)'", re.M)
SQ_RE = re.compile(r"'F:\\d1\\([^']*)'")
BARE_RE = re.compile(r"(?<=\s)F:\\d1\\([^\s'\"]+)")


def ps(f, manual=(), dot=None):
    t, crlf = load(f)
    t = t.replace('F:\\d1\tools', 'F:\\d1\\tools')     # d1-world-capture: a literal tab once ate '\t'
    for old, new in manual:
        t = sub1(t, old, new)
    t = PARAM_RE.sub(lambda m: m.group(1) + call(m.group(2), invoke=True), t)
    body_changed = [False]

    def body(m):
        body_changed[0] = True
        return call(m.group(1))
    t = SQ_RE.sub(body, t)
    t = BARE_RE.sub(body, t)
    if 'Get-D1Path' in t.replace('(& "$PSScriptRoot\\D1Paths.ps1"', ''):
        if not dot:
            sys.exit(f'{f}: body uses Get-D1Path but no dot-source anchor given')
        t = sub1(t, dot, dot + '. "$PSScriptRoot\\D1Paths.ps1"\n')
    if 'F:\\d1' in t:
        sys.exit(f'{f}: literal left: ' + t[t.index('F:\\d1') - 60:t.index('F:\\d1') + 60])
    save(f, t, crlf)


EAP_STOP = "$ErrorActionPreference = 'Stop'\n"

for f in ['Apply-D1PS4Preparation.ps1', 'Apply-D1PkgOwnershipFix.ps1', 'Build-D1PfsReader.ps1',
          'Export-D1RepackContent.ps1', 'Export-D1RepackFull.ps1', 'Export-D1RepackHeaders.ps1',
          'Export-D1RepackSceSys.ps1', 'Export-D1RepackSlack.ps1', 'Join-D1Update.ps1',
          'Prepare-D1PS4Launch.ps1', 'Start-D1ContainedBoot.ps1', 'Test-D1BuildContent.ps1',
          'Test-D1PS4Readiness.ps1', 'Test-D1RecoveredContent.ps1', 'Test-D1RepackInodes.ps1']:
    ps(f)

ps('d1-agr-run.ps1', manual=[
    ('$D = "F:\\d1\\d1preservation-main\\cfw-staging\\$Name"', '$D = Get-D1Path Staging $Name'),
    ("$pad = 'F:\\d1\\tools\\d1-pad.ps1'\n",
     "$pad = Get-D1Path Tools 'd1-pad.ps1'\n"
     "$hostExe = Get-D1Path Workspace 'd1-server\\target\\release\\d1-activity-host.exe'\n"),
    ('",D1_AH_CONTROL_FILE=F:\\d1\\d1preservation-main\\cfw-staging\\ah-control.txt"',
     '",D1_AH_CONTROL_FILE=$(Get-D1Path Staging \'ah-control.txt\')"'),
    ("$server.Path -eq 'F:\\d1\\d1preservation-main\\d1-server\\target\\release\\d1-activity-host.exe'",
     '$server.Path -eq $hostExe'),
    ("$B = 'F:\\d1\\d1preservation-main\\capture-rig\\builds\\later-56578'",
     "$B = Get-D1Path Workspace 'capture-rig\\builds\\later-56578'"),
    ("$L = 'F:\\d1\\tools\\shadPS4\\0.19.0\\user\\log\\shad_log.txt'",
     "$L = Get-D1Path ShadPS4 'user\\log\\shad_log.txt'"),
], dot="$ErrorActionPreference = 'Continue'\n")

ps('d1-lab-served.ps1', manual=[
    ("$B = 'F:\\d1\\d1preservation-main\\capture-rig\\builds\\later-56578'",
     "$B = Get-D1Path Workspace 'capture-rig\\builds\\later-56578'"),
    ("$S = 'F:\\d1\\d1preservation-main\\cfw-staging'", '$S = Get-D1Path Staging'),
], dot="if ($Extra.Count -eq 1 -and $Extra[0] -match ',') { $Extra = $Extra[0] -split ',(?=[A-Z0-9_]+=)' }\n")

shad_old = "$shad = 'F:\\d1\\tools\\shadPS4\\0.19.0\\user\\log\\shad_log.txt'"
shad_new = "$shad = Get-D1Path ShadPS4 'user\\log\\shad_log.txt'"
for f in ['d1-drive-in-world.ps1', 'd1-drive-to-orbit.ps1', 'd1-orbit-count.ps1']:
    ps(f, manual=[(shad_old, shad_new)], dot=EAP_STOP)
ps('d1-relaunch.ps1', manual=[("$L = 'F:\\d1\\tools\\shadPS4\\0.19.0\\user\\log\\shad_log.txt'",
                               "$L = Get-D1Path ShadPS4 'user\\log\\shad_log.txt'")], dot=EAP_STOP)
ps('d1-world-capture.ps1', manual=[("$S = 'F:\\d1\\d1preservation-main\\cfw-staging'", '$S = Get-D1Path Staging')],
   dot=EAP_STOP)
ps('d1-lab-net.ps1', manual=[
    ("$exe     = 'F:\\d1\\tools\\shadPS4\\0.19.0\\shadPS4.exe'", "$exe     = Get-D1Path ShadPS4 'shadPS4.exe'"),
    # The marker must stay byte-identical to the one contain-d1.ps1 writes from the same tools folder.
    ("$begin   = '# >>> D1 local redirects (F:\\d1\\tools\\contain-d1.ps1)'",
     "$begin   = \"# >>> D1 local redirects ($(Get-D1Path Tools 'contain-d1.ps1'))\""),
    ("Get-Content -LiteralPath 'F:\\d1\\tools\\shadPS4\\d1-hosts.txt'",
     "Get-Content -LiteralPath (Get-D1Path Tools 'shadPS4\\d1-hosts.txt')"),
], dot=EAP_STOP)
print('ps ok')
