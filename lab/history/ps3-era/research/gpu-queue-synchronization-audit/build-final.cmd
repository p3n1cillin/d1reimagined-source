@echo off
call F:\VS\BuildTools\VC\Auxiliary\Build\vcvars64.bat >nul
cd /d F:\d1\<pc>-import\d1xenia\local\RESEARCH\gpu-queue-synchronization-audit
cl /nologo /std:c++20 /EHsc /W4 before-final\queue-synchronization-audit.cpp /Febefore-final\audit.exe /Fobefore-final\audit.obj >before-final\build.log 2>&1
if errorlevel 1 exit /b %errorlevel%
before-final\audit.exe >before-final\result.log 2>&1
set baseline_result=%errorlevel%
cl /nologo /std:c++20 /EHsc /W4 after-final\queue-synchronization-audit.cpp /Feafter-final\audit.exe /Foafter-final\audit.obj >after-final\build.log 2>&1
if errorlevel 1 exit /b %errorlevel%
after-final\audit.exe >after-final\result.log 2>&1
set fixed_result=%errorlevel%
echo baseline_exit=%baseline_result%, fixed_exit=%fixed_result%
exit /b %fixed_result%
