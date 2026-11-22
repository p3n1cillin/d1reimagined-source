@echo off
call <VS_BUILDTOOLS>\VC\Auxiliary\Build\vcvars64.bat >nul
cd /d <D1_ROOT>\<pc>-import\d1xenia\local\RESEARCH\fragment-start-audit
cl /nologo /std:c++20 /EHsc /W4 fragment-start-audit.cpp /Fe:fragment-start-audit.exe /Fo:fragment-start-audit.obj
if errorlevel 1 exit /b 1
fragment-start-audit.exe
