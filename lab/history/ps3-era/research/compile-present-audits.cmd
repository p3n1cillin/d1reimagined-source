@echo off
call <VS_BUILDTOOLS>\VC\Auxiliary\Build\vcvars64.bat >nul
cl /nologo /std:c++20 /EHsc /W4 /Od <D1_ROOT>\<pc>-import\d1xenia\local\RESEARCH\present-lifetime-audit\present-lifetime-audit.cpp /Fe:<D1_ROOT>\<pc>-import\d1xenia\local\RESEARCH\present-lifetime-audit\present-lifetime-audit.exe /Fo:<D1_ROOT>\<pc>-import\d1xenia\local\RESEARCH\present-lifetime-audit\present-lifetime-audit.obj
if errorlevel 1 exit /b 1
cl /nologo /std:c++20 /EHsc /W4 /Od <D1_ROOT>\<pc>-import\d1xenia\local\RESEARCH\present-lifetime-baseline-audit\present-lifetime-audit.cpp /Fe:<D1_ROOT>\<pc>-import\d1xenia\local\RESEARCH\present-lifetime-baseline-audit\present-lifetime-audit.exe /Fo:<D1_ROOT>\<pc>-import\d1xenia\local\RESEARCH\present-lifetime-baseline-audit\present-lifetime-audit.obj
exit /b %errorlevel%