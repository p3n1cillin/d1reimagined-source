@echo off
call <VS_BUILDTOOLS>\VC\Auxiliary\Build\vcvars64.bat >nul
cl /nologo /std:c++20 /EHsc /W4 /Od <D1_ROOT>\<pc>-import\d1xenia\local\RESEARCH\texture-usage-audit\texture-usage-audit.cpp /Fe:<D1_ROOT>\<pc>-import\d1xenia\local\RESEARCH\texture-usage-audit\texture-usage-audit.exe /Fo:<D1_ROOT>\<pc>-import\d1xenia\local\RESEARCH\texture-usage-audit\texture-usage-audit.obj
if errorlevel 1 exit /b 1
cl /nologo /std:c++20 /EHsc /W4 /Od <D1_ROOT>\<pc>-import\d1xenia\local\RESEARCH\texture-usage-baseline-audit\texture-usage-audit.cpp /Fe:<D1_ROOT>\<pc>-import\d1xenia\local\RESEARCH\texture-usage-baseline-audit\texture-usage-audit.exe /Fo:<D1_ROOT>\<pc>-import\d1xenia\local\RESEARCH\texture-usage-baseline-audit\texture-usage-audit.obj
exit /b %errorlevel%