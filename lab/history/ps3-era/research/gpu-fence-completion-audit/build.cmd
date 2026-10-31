@echo off
call F:\VS\BuildTools\VC\Auxiliary\Build\vcvars64.bat >nul
cd /d F:\d1\<pc>-import\d1xenia\local\RESEARCH\gpu-fence-completion-audit
cl /nologo /std:c++20 /EHsc /W4 before\fence-completion-audit.cpp /Febefore\audit.exe /Fobefore\audit.obj >before\build.log 2>&1
if errorlevel 1 exit /b %errorlevel%
before\audit.exe >before\result.log 2>&1
set baseline_result=%errorlevel%
cl /nologo /std:c++20 /EHsc /W4 after\fence-completion-audit.cpp /Feafter\audit.exe /Foafter\audit.obj >after\build.log 2>&1
if errorlevel 1 exit /b %errorlevel%
after\audit.exe >after\result.log 2>&1
set fixed_result=%errorlevel%
echo baseline_exit=%baseline_result%, fixed_exit=%fixed_result%
exit /b %fixed_result%
