@echo off
call "F:\VS\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul || exit /b 1
set "PATH=F:\d1\tools\llvm-23.1.3\bin;%LOCALAPPDATA%\Microsoft\WinGet\Packages\Ninja-build.Ninja_Microsoft.Winget.Source_8wekyb3d8bbwe;C:\Program Files\CMake\bin;%PATH%"
cmake --build "F:\d1\tools\shadPS4-src\build-d1" -j 1 --target CMakeFiles/shadps4.dir/src/core/d1_native_mouse.cpp.obj || exit /b 3
echo compiled
