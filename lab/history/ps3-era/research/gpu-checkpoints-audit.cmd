@echo off
call F:\VS\BuildTools\VC\Auxiliary\Build\vcvars64.bat >nul
cl /nologo /std:c++20 /EHsc F:\d1\<pc>-import\d1xenia\local\RESEARCH\gpu-checkpoints-final-audit\gpu-checkpoint-audit.cpp /Fe:F:\d1\<pc>-import\d1xenia\local\RESEARCH\gpu-checkpoints-final-audit\audit.exe /Fo:F:\d1\<pc>-import\d1xenia\local\RESEARCH\gpu-checkpoints-final-audit\audit.obj
if errorlevel 1 exit /b %errorlevel%
F:\d1\<pc>-import\d1xenia\local\RESEARCH\gpu-checkpoints-final-audit\audit.exe
exit /b %errorlevel%
