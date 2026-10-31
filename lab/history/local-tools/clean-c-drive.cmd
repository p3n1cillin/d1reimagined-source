@echo off
rem Right-click > Run as administrator. Asks before every step.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0clean-c-drive.ps1"
