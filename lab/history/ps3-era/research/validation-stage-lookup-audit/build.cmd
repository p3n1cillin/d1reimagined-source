@echo off
call <VS_BUILDTOOLS>\VC\Auxiliary\Build\vcvars64.bat >nul
cl /nologo /std:c++20 /EHsc /W4 validation-stage-lookup-audit.cpp /Fe:validation-stage-lookup-audit.exe