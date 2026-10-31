@echo off
setlocal
set "D1_ROOT=F:/d1"
if not defined D1_TOOLS (
  if not defined D1_ROOT (
    echo no 1>&2
    exit /b 4
  )
  set "D1_TOOLS=%D1_ROOT%\tools"
)
echo tools=%D1_TOOLS%
if not defined VCVARS for /f "usebackq delims=" %%i in (`"%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe" -latest -products * -property installationPath`) do set "VCVARS=%%i\VC\Auxiliary\Build\vcvars64.bat"
echo vcvars=%VCVARS%
