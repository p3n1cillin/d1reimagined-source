@echo off
call "<VS_BUILDTOOLS>\VC\Auxiliary\Build\vcvars64.bat" >nul || exit /b 1
set "PATH=<D1_ROOT>\tools\llvm-23.1.3\bin;%LOCALAPPDATA%\Microsoft\WinGet\Packages\Ninja-build.Ninja_Microsoft.Winget.Source_8wekyb3d8bbwe;C:\Program Files\CMake\bin;%PATH%"
cmake -S "<D1_ROOT>\tools\shadPS4-src\v0.19.0-d1" -B "<D1_ROOT>\tools\shadPS4-src\build-d1" >nul || exit /b 2
cmake --build "<D1_ROOT>\tools\shadPS4-src\build-d1" -j 2 || exit /b 3
echo built
