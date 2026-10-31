@echo off
call F:\VS\BuildTools\VC\Auxiliary\Build\vcvars64.bat >nul
cl /nologo /std:c++20 /EHsc /W4 descriptor-retirement-audit.cpp /Fe:descriptor-retirement-audit.exe