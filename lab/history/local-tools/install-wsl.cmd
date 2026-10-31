@echo off
rem Installs WSL2 with Ubuntu 24.04. Right-click this file > "Run as administrator".
rem Needs CPU virtualization (SVM Mode) enabled in the BIOS first.
net session >nul 2>&1 || (echo Please right-click this file and choose "Run as administrator". & pause & exit /b 1)
wsl --install -d Ubuntu-24.04
echo.
echo When it finishes, restart the PC. After the restart Ubuntu opens once and asks for a
echo username and password: enter d1 / d1test. Then tell Claude "WSL is ready".
pause
