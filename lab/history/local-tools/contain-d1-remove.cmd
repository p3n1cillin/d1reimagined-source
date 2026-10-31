@echo off
rem Right-click > Run as administrator. Removes the D1 hosts redirects and firewall rule.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0contain-d1.ps1" -Mode remove
