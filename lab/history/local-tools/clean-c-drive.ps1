# C: drive cleanup. Every step asks first. Run elevated via clean-c-drive.cmd.
$ErrorActionPreference = 'Continue'
if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Host 'Run clean-c-drive.cmd with right-click > Run as administrator.' -ForegroundColor Red; Read-Host 'Enter to close'; exit 1
}
$user = '%USERPROFILE%'
function Free { [math]::Round((Get-Volume -DriveLetter C).SizeRemaining / 1GB, 2) }
function SizeOf($p) { if (Test-Path -LiteralPath $p) { [math]::Round(((Get-ChildItem -LiteralPath $p -Recurse -Force -File -ErrorAction SilentlyContinue | Measure-Object Length -Sum).Sum) / 1GB, 2) } else { 0 } }
function Ask($text) { (Read-Host "$text  [y/N]") -match '^(y|yes)$' }
$start = Free
Write-Host "C: free before: $start GB`n" -ForegroundColor Cyan

# 1. Orphaned LLVM (its registration was removed; a working copy now lives at F:\d1\tools\LLVM)
$llvm = 'C:\Program Files\LLVM'
if ((Test-Path $llvm) -and (Test-Path 'F:\d1\tools\LLVM\bin\libclang.dll')) {
    if (Ask "1. Delete orphaned C:\Program Files\LLVM ($(SizeOf $llvm) GB; the build now uses F:\d1\tools\LLVM)?") { Remove-Item -LiteralPath $llvm -Recurse -Force }
}

# 2. Hibernation file
$hib = 'C:\hiberfil.sys'
if (Test-Path $hib -ErrorAction SilentlyContinue) {
    $gb = [math]::Round((Get-Item $hib -Force).Length / 1GB, 1)
    if (Ask "2. Turn off hibernation and remove hiberfil.sys ($gb GB)? Sleep still works; Hibernate and Fast Startup stop. Undo later with: powercfg /h on") { powercfg /h off }
}

# 3. NVIDIA shader caches (rebuilt automatically; the first launch of each game may stutter briefly)
foreach ($c in "$user\AppData\Local\NVIDIA\DXCache", "$user\AppData\Local\NVIDIA\GLCache") {
    if (Test-Path $c) { if (Ask "3. Clear NVIDIA shader cache $(Split-Path $c -Leaf) ($(SizeOf $c) GB)?") { Get-ChildItem -LiteralPath $c -Force | Remove-Item -Recurse -Force -ErrorAction SilentlyContinue } }
}

# 4. Leftover Windows upgrade staging
$ws = 'C:\$Windows.~WS'
if (Test-Path -LiteralPath $ws) {
    if (Ask "4. Delete leftover Windows upgrade files C:\`$Windows.~WS ($(SizeOf $ws) GB)?") {
        takeown /F $ws /R /D Y | Out-Null; icacls $ws /grant Administrators:F /T /C /Q | Out-Null; Remove-Item -LiteralPath $ws -Recurse -Force -ErrorAction SilentlyContinue
    }
}

# 5. Application crash dumps
$cd = "$user\AppData\Local\CrashDumps"
if (Test-Path $cd) { if (Ask "5. Delete old application crash dumps ($(SizeOf $cd) GB)?") { Get-ChildItem -LiteralPath $cd -Force -File | Remove-Item -Force -ErrorAction SilentlyContinue } }

# 6. Temp files older than 7 days (in-use files are skipped)
if (Ask "6. Delete Temp files older than 7 days (user and Windows Temp)?") {
    foreach ($t in "$user\AppData\Local\Temp", 'C:\Windows\Temp') {
        Get-ChildItem -LiteralPath $t -Recurse -Force -File -ErrorAction SilentlyContinue | Where-Object { $_.LastWriteTime -lt (Get-Date).AddDays(-7) } | Remove-Item -Force -ErrorAction SilentlyContinue
    }
}

# 7. Windows component store (superseded update files; safe, takes a few minutes)
if (Ask '7. Run Windows component cleanup (DISM StartComponentCleanup)?') { Dism.exe /Online /Cleanup-Image /StartComponentCleanup }

$end = Free
Write-Host "`nC: free after: $end GB (freed $([math]::Round($end - $start, 2)) GB)" -ForegroundColor Green
Read-Host 'Enter to close'
