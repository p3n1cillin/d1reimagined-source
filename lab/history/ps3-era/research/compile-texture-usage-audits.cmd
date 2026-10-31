@echo off
call F:\VS\BuildTools\VC\Auxiliary\Build\vcvars64.bat >nul
cl /nologo /std:c++20 /EHsc /W4 /Od F:\d1\<pc>-import\d1xenia\local\RESEARCH\texture-usage-audit\texture-usage-audit.cpp /Fe:F:\d1\<pc>-import\d1xenia\local\RESEARCH\texture-usage-audit\texture-usage-audit.exe /Fo:F:\d1\<pc>-import\d1xenia\local\RESEARCH\texture-usage-audit\texture-usage-audit.obj
if errorlevel 1 exit /b 1
cl /nologo /std:c++20 /EHsc /W4 /Od F:\d1\<pc>-import\d1xenia\local\RESEARCH\texture-usage-baseline-audit\texture-usage-audit.cpp /Fe:F:\d1\<pc>-import\d1xenia\local\RESEARCH\texture-usage-baseline-audit\texture-usage-audit.exe /Fo:F:\d1\<pc>-import\d1xenia\local\RESEARCH\texture-usage-baseline-audit\texture-usage-audit.obj
exit /b %errorlevel%