@echo off
call F:\VS\BuildTools\VC\Auxiliary\Build\vcvars64.bat >nul
cl /nologo /std:c++20 /EHsc /W4 F:\d1\<pc>-import\d1xenia\local\RESEARCH\fragment-depth-audit\fragment-depth-audit.cpp /Fe:F:\d1\<pc>-import\d1xenia\local\RESEARCH\fragment-depth-audit\fragment-depth-audit.exe /Fo:F:\d1\<pc>-import\d1xenia\local\RESEARCH\fragment-depth-audit\fragment-depth-audit.obj
if errorlevel 1 exit /b %errorlevel%
F:\d1\<pc>-import\d1xenia\local\RESEARCH\fragment-depth-audit\fragment-depth-audit.exe
