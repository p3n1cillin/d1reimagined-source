@echo off
call <VS_BUILDTOOLS>\VC\Auxiliary\Build\vcvars64.bat >nul
cl /nologo /std:c++20 /EHsc /W4 <D1_ROOT>\<pc>-import\d1xenia\local\RESEARCH\fragment-depth-audit\fragment-depth-audit.cpp /Fe:<D1_ROOT>\<pc>-import\d1xenia\local\RESEARCH\fragment-depth-audit\fragment-depth-audit.exe /Fo:<D1_ROOT>\<pc>-import\d1xenia\local\RESEARCH\fragment-depth-audit\fragment-depth-audit.obj
if errorlevel 1 exit /b %errorlevel%
<D1_ROOT>\<pc>-import\d1xenia\local\RESEARCH\fragment-depth-audit\fragment-depth-audit.exe
