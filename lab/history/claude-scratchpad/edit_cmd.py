from pathlib import Path
D = Path('F:/d1/public-work/wt-paths/tools/ps4_setup')


def edit(f, pairs):
    p = D / f
    t = p.read_bytes().decode('utf-8')
    crlf = '\r\n' in t
    t = t.replace('\r\n', '\n')
    for old, new in pairs:
        assert t.count(old) == 1, (f, old)
        t = t.replace(old, new)
    p.write_bytes((t.replace('\n', '\r\n') if crlf else t).encode('utf-8'))


edit('Build-D1ShadPS4.cmd', [
    ('rem Usage: Build-D1ShadPS4.cmd [source-dir] [build-dir]\nsetlocal\n',
     'rem Usage: Build-D1ShadPS4.cmd [source-dir] [build-dir]\n'
     'rem Defaults live under %D1_TOOLS% (else %D1_ROOT%\\tools; see ps4/README.md "Local paths").\n'
     'rem VCVARS names vcvars64.bat; unset, vswhere finds the newest Visual Studio install.\n'
     'setlocal\n'
     'if not defined D1_TOOLS (\n'
     '  if not defined D1_ROOT (\n'
     '    echo Build-D1ShadPS4: set D1_ROOT to the D1 root folder, or D1_TOOLS to its tools folder. 1>&2\n'
     '    echo See ps4/README.md, "Local paths". 1>&2\n'
     '    exit /b 4\n'
     '  )\n'
     '  set "D1_TOOLS=%D1_ROOT%\\tools"\n'
     ')\n'),
    ('if "%SRC%"=="" set "SRC=F:\\d1\\tools\\shadPS4-src\\v0.19.0-d1"',
     'if "%SRC%"=="" set "SRC=%D1_TOOLS%\\shadPS4-src\\v0.19.0-d1"'),
    ('if "%OUT%"=="" set "OUT=F:\\d1\\tools\\shadPS4-src\\build-d1"',
     'if "%OUT%"=="" set "OUT=%D1_TOOLS%\\shadPS4-src\\build-d1"'),
    ('set "LLVM=F:\\d1\\tools\\llvm-23.1.3\\bin"', 'set "LLVM=%D1_TOOLS%\\llvm-23.1.3\\bin"'),
    ('call "F:\\VS\\BuildTools\\VC\\Auxiliary\\Build\\vcvars64.bat" >nul || exit /b 1',
     'if not defined VCVARS for /f "usebackq delims=" %%i in (`"%ProgramFiles(x86)%\\Microsoft Visual Studio\\Installer\\vswhere.exe" -latest -products * -property installationPath`) do set "VCVARS=%%i\\VC\\Auxiliary\\Build\\vcvars64.bat"\n'
     'call "%VCVARS%" >nul || exit /b 1'),
])

NOREPLY = 'From: p3n1cillin <p3n1cillin@users.noreply.github.com>'
for f in ['d1-56578-codec-ah-profile.patch', 'd1-56578-unlock-ordinals.patch']:
    edit(f, [('From: p3n1cillin <<old-email>>', NOREPLY)])
print('ok')
