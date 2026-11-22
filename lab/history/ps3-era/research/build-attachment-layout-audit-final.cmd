@echo off
call <VS_BUILDTOOLS>\VC\Auxiliary\Build\vcvars64.bat >nul
cd /d <D1_ROOT>\<pc>-import\d1xenia\local\RESEARCH\gpu-attachment-layout-audit-before-final
cl /nologo /std:c++20 /EHsc /W4 attachment-layout-audit.cpp /Fe:attachment-layout-audit.exe > build.log 2>&1
if errorlevel 1 exit /b 2
attachment-layout-audit.exe > result.log 2>&1
cd /d <D1_ROOT>\<pc>-import\d1xenia\local\RESEARCH\gpu-attachment-layout-audit-final
cl /nologo /std:c++20 /EHsc /W4 attachment-layout-audit.cpp /Fe:attachment-layout-audit.exe > build.log 2>&1
if errorlevel 1 exit /b 3
attachment-layout-audit.exe > result.log 2>&1
exit /b %errorlevel%

