@echo off
call F:\VS\BuildTools\VC\Auxiliary\Build\vcvars64.bat >nul
cd /d F:\d1\<pc>-import\d1xenia\local\RESEARCH\JOIN173-ppu-filter-audit
cl /nologo /std:c++20 /EHsc /W4 /O2 ppu-watch-filter-audit.cpp /Fe:ppu-watch-filter-audit.exe
if errorlevel 1 exit /b 1
ppu-watch-filter-audit.exe


