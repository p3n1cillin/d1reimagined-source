@echo off
call F:\VS\BuildTools\VC\Auxiliary\Build\vcvars64.bat >nul
cl /nologo /std:c++20 /EHsc F:\d1\<pc>-import\d1xenia\local\RESEARCH\fragmented-recording-current-audit\recording-header-audit.cpp /Fe:F:\d1\<pc>-import\d1xenia\local\RESEARCH\fragmented-recording-current-audit\audit.exe /Fo:F:\d1\<pc>-import\d1xenia\local\RESEARCH\fragmented-recording-current-audit\audit.obj
if errorlevel 1 exit /b %errorlevel%
F:\d1\<pc>-import\d1xenia\local\RESEARCH\fragmented-recording-current-audit\audit.exe
if errorlevel 1 exit /b %errorlevel%
cl /nologo /std:c++20 /EHsc F:\d1\<pc>-import\d1xenia\local\RESEARCH\fragmented-recording-before-audit\recording-header-audit.cpp /Fe:F:\d1\<pc>-import\d1xenia\local\RESEARCH\fragmented-recording-before-audit\audit.exe /Fo:F:\d1\<pc>-import\d1xenia\local\RESEARCH\fragmented-recording-before-audit\audit.obj
if errorlevel 1 exit /b %errorlevel%
F:\d1\<pc>-import\d1xenia\local\RESEARCH\fragmented-recording-before-audit\audit.exe
if errorlevel 1 exit /b 0
exit /b 1
