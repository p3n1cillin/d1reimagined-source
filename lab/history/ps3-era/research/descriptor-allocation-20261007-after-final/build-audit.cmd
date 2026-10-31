@echo off
call F:\VS\BuildTools\VC\Auxiliary\Build\vcvars64.bat >nul
cd /d F:\d1\<pc>-import\d1xenia\local\RESEARCH\descriptor-allocation-20261007-after-final
cl /nologo /std:c++20 /EHsc /W4 descriptor-allocation-audit.cpp /Fe:descriptor-allocation-audit.exe >build.log 2>&1
if errorlevel 1 exit /b 2
descriptor-allocation-audit.exe >result.log
exit /b %errorlevel%