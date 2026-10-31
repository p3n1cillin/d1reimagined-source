@echo off
rem Right-click > Run as administrator. Applies D1 hosts redirects + shadPS4 outbound firewall block.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0contain-d1.ps1" -Mode apply
