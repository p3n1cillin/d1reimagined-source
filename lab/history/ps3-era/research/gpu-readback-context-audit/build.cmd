@echo off
call F:\VS\BuildTools\VC\Auxiliary\Build\vcvars64.bat >nul
cd /d F:\d1\<pc>-import\d1xenia\local\RESEARCH\gpu-readback-context-audit
cl /nologo /std:c++20 /EHsc /W4 gpu-checkpoint-audit.cpp /Fe:audit.exe
if errorlevel 1 exit /b 1
audit.exe
