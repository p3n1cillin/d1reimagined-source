@echo off
call F:\VS\BuildTools\VC\Auxiliary\Build\vcvars64.bat >nul
cd /d F:\d1\<pc>-import\d1xenia\local\RESEARCH\fragment-depth-JOIN158-audit
cl /nologo /std:c++20 /EHsc /W4 fragment-depth-audit.cpp /Fe:fragment-depth-audit.exe /Fo:fragment-depth-audit.obj >build.log 2>&1
if errorlevel 1 exit /b %errorlevel%
fragment-depth-audit.exe >result.log 2>&1
exit /b %errorlevel%