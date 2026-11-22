@echo off
call <VS_BUILDTOOLS>\VC\Auxiliary\Build\vcvars64.bat >nul
cl /nologo /std:c++20 /EHsc <D1_ROOT>\<pc>-import\d1xenia\local\RESEARCH\gpu-checkpoints-final-audit\gpu-checkpoint-audit.cpp /Fe:<D1_ROOT>\<pc>-import\d1xenia\local\RESEARCH\gpu-checkpoints-final-audit\audit.exe /Fo:<D1_ROOT>\<pc>-import\d1xenia\local\RESEARCH\gpu-checkpoints-final-audit\audit.obj
if errorlevel 1 exit /b %errorlevel%
<D1_ROOT>\<pc>-import\d1xenia\local\RESEARCH\gpu-checkpoints-final-audit\audit.exe
exit /b %errorlevel%
