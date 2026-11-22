@echo off
call <VS_BUILDTOOLS>\VC\Auxiliary\Build\vcvars64.bat >nul
cd /d <D1_ROOT>\<pc>-import\d1xenia\local\RESEARCH\descriptor-cache-audit
cl /nologo /std:c++20 /EHsc /W4 descriptor-cache-audit.cpp /Fe:descriptor-cache-audit.exe
if errorlevel 1 exit /b %errorlevel%
descriptor-cache-audit.exe
