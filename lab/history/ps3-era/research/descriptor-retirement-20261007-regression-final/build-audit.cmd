@echo off
call F:\VS\BuildTools\VC\Auxiliary\Build\vcvars64.bat >nul
cd /d F:\d1\<pc>-import\d1xenia\local\RESEARCH\descriptor-retirement-20261007-regression-final
cl /nologo /std:c++20 /EHsc /W4 descriptor-retirement-audit.cpp /Fe:descriptor-retirement-audit.exe >build.log 2>&1
if errorlevel 1 exit /b 2
descriptor-retirement-audit.exe >result.log
exit /b %errorlevel%