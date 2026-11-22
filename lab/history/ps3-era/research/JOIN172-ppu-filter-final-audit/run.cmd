@echo off
call <VS_BUILDTOOLS>\VC\Auxiliary\Build\vcvars64.bat >nul
cd /d <D1_ROOT>\<pc>-import\d1xenia\local\RESEARCH\JOIN172-ppu-filter-final-audit
cl /nologo /std:c++20 /EHsc /W4 /O2 ppu-watch-filter-audit.cpp /Fe:ppu-watch-filter-audit.exe
if errorlevel 1 exit /b 1
ppu-watch-filter-audit.exe

