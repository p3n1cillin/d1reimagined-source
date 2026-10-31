@echo off
call F:\VS\BuildTools\VC\Auxiliary\Build\vcvars64.bat >nul
cl /nologo /std:c++20 /EHsc /W4 /Od F:\d1\<pc>-import\d1xenia\local\RESEARCH\present-lifetime-audit\present-lifetime-audit.cpp /Fe:F:\d1\<pc>-import\d1xenia\local\RESEARCH\present-lifetime-audit\present-lifetime-audit.exe /Fo:F:\d1\<pc>-import\d1xenia\local\RESEARCH\present-lifetime-audit\present-lifetime-audit.obj
if errorlevel 1 exit /b 1
cl /nologo /std:c++20 /EHsc /W4 /Od F:\d1\<pc>-import\d1xenia\local\RESEARCH\present-lifetime-baseline-audit\present-lifetime-audit.cpp /Fe:F:\d1\<pc>-import\d1xenia\local\RESEARCH\present-lifetime-baseline-audit\present-lifetime-audit.exe /Fo:F:\d1\<pc>-import\d1xenia\local\RESEARCH\present-lifetime-baseline-audit\present-lifetime-audit.obj
exit /b %errorlevel%