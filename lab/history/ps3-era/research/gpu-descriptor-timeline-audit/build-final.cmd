@echo off
call F:\VS\BuildTools\VC\Auxiliary\Build\vcvars64.bat >nul
cd /d F:\d1\<pc>-import\d1xenia\local\RESEARCH\gpu-descriptor-timeline-audit
cl /nologo /std:c++20 /EHsc /W4 trace-final\descriptor-timeline-audit.cpp /Fetrace-final\audit.exe /Fotrace-final\audit.obj >trace-final\build.log 2>&1
if errorlevel 1 exit /b %errorlevel%
trace-final\audit.exe >trace-final\result.log 2>&1
if errorlevel 1 exit /b %errorlevel%
cl /nologo /std:c++20 /EHsc /W4 fence-final\fence-completion-audit.cpp /Fefence-final\audit.exe /Fofence-final\audit.obj >fence-final\build.log 2>&1
if errorlevel 1 exit /b %errorlevel%
fence-final\audit.exe >fence-final\result.log 2>&1
exit /b %errorlevel%