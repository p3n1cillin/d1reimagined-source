@echo off
call F:\VS\BuildTools\VC\Auxiliary\Build\vcvars64.bat >nul
cd /d F:\d1\<pc>-import\d1xenia\local\RESEARCH\gpu-descriptor-timeline-audit
cl /nologo /std:c++20 /EHsc /W4 trace\descriptor-timeline-audit.cpp /Fetrace\audit.exe /Fotrace\audit.obj >trace\build.log 2>&1
if errorlevel 1 exit /b %errorlevel%
trace\audit.exe >trace\result.log 2>&1
if errorlevel 1 exit /b %errorlevel%
cl /nologo /std:c++20 /EHsc /W4 queue\queue-synchronization-audit.cpp /Fequeue\audit.exe /Foqueue\audit.obj >queue\build.log 2>&1
if errorlevel 1 exit /b %errorlevel%
queue\audit.exe >queue\result.log 2>&1
if errorlevel 1 exit /b %errorlevel%
cl /nologo /std:c++20 /EHsc /W4 gc\gc-scope-audit.cpp /Fegc\audit.exe /Fogc\audit.obj >gc\build.log 2>&1
if errorlevel 1 exit /b %errorlevel%
gc\audit.exe >gc\result.log 2>&1
if errorlevel 1 exit /b %errorlevel%
cl /nologo /std:c++20 /EHsc /W4 fence\fence-completion-audit.cpp /Fefence\audit.exe /Fofence\audit.obj >fence\build.log 2>&1
if errorlevel 1 exit /b %errorlevel%
fence\audit.exe >fence\result.log 2>&1
exit /b %errorlevel%