@echo off
rem Build shadPS4 v.0.19.0 (c7e065d1) with shadps4-0.19.0-local-ip.patch applied, using the
rem official LLVM 23.1.3 clang-cl and the MSVC/Windows SDK environment. Discord RPC and the
rem updater are off: a contained lab binary must not reach the network on its own.
rem HAVE_ENDIAN_H=OFF: LLVM 23 ships an endian.h without the Windows byte-order helpers, which
rem would otherwise make the vendored LibreSSL skip its own Windows definitions.
rem Usage: Build-D1ShadPS4.cmd [source-dir] [build-dir]
rem Defaults live under %D1_TOOLS% (else %D1_ROOT%\tools; see ps4/README.md "Local paths").
rem VCVARS names vcvars64.bat; unset, vswhere finds the newest Visual Studio install.
setlocal
if not defined D1_TOOLS if defined D1_ROOT set "D1_TOOLS=%D1_ROOT%\tools"
if not defined D1_TOOLS goto :nopaths
set "SRC=%~1"
if "%SRC%"=="" set "SRC=%D1_TOOLS%\shadPS4-src\v0.19.0-d1"
set "OUT=%~2"
if "%OUT%"=="" set "OUT=%D1_TOOLS%\shadPS4-src\build-d1"
set "LLVM=%D1_TOOLS%\llvm-23.1.3\bin"
set "NINJA=%LOCALAPPDATA%\Microsoft\WinGet\Packages\Ninja-build.Ninja_Microsoft.Winget.Source_8wekyb3d8bbwe"
if not defined VCVARS for /f "usebackq delims=" %%i in (`"%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe" -latest -products * -property installationPath`) do set "VCVARS=%%i\VC\Auxiliary\Build\vcvars64.bat"
call "%VCVARS%" >nul || exit /b 1
set "PATH=%LLVM%;%NINJA%;C:\Program Files\CMake\bin;%PATH%"
cmake -S "%SRC%" -B "%OUT%" -G Ninja -DCMAKE_BUILD_TYPE=Release ^
  -DCMAKE_C_COMPILER=clang-cl -DCMAKE_CXX_COMPILER=clang-cl ^
  -DENABLE_DISCORD_RPC=OFF -DENABLE_UPDATER=OFF ^
  -DHAVE_ENDIAN_H=OFF || exit /b 2
cmake --build "%OUT%" --parallel || exit /b 3
echo built: %OUT%\shadPS4.exe
exit /b 0
:nopaths
echo Build-D1ShadPS4: set D1_ROOT to the D1 root folder, or D1_TOOLS to its tools folder. 1>&2
echo See ps4/README.md, "Local paths". 1>&2
exit /b 4
