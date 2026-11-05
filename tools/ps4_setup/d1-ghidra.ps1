# SPDX-License-Identifier: GPL-2.0-or-later
# Ghidra (headless) on the owned 56578 eboot: import and analyse once, then decompile by address.
#
#   d1-ghidra.ps1 -Import                       # one-off, slow (CPU-heavy: never during a route run)
#   d1-ghidra.ps1 -Decompile 0x403140,0x403770  # C of the functions containing those vaddrs
#
# Needs D1_GHIDRA_HOME (the unpacked ghidra_<ver>_PUBLIC folder), D1_JAVA_HOME (a JDK 25: Ghidra 12 refuses 21) and
# D1_EBOOT. The project (it holds the game's code) goes to the staging folder, never the repo.
# Runs at Idle priority: heavy CPU and disk stall route runs (notes/ps4-56578-lab-stalls-2026-11-05.md).
param(
    [switch]$Import,
    [string[]]$Decompile = @(),
    [string]$Project = 'd1-56578'
)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\D1Paths.ps1"
foreach ($v in 'D1_GHIDRA_HOME', 'D1_JAVA_HOME', 'D1_EBOOT') {
    if (-not [Environment]::GetEnvironmentVariable($v)) { throw "$v is not set" }
}
$headless = Join-Path $env:D1_GHIDRA_HOME 'support\analyzeHeadless.bat'
$dir = Join-Path (Get-D1Path Staging) 'ghidra'
New-Item -ItemType Directory -Force $dir | Out-Null
$env:JAVA_HOME = $env:D1_JAVA_HOME
$eboot = (Resolve-Path $env:D1_EBOOT).Path
$name = Split-Path $eboot -Leaf
if ($Import) {
    $args = @($dir, $Project, '-import', $eboot, '-overwrite')
} elseif ($Decompile.Count) {
    $addrs = @($Decompile | ForEach-Object { $_ -split ',' } | Where-Object { $_ })
    $args = @($dir, $Project, '-process', $name, '-noanalysis', '-readOnly',
              '-scriptPath', (Join-Path $PSScriptRoot 'ghidra'), '-postScript', 'DecompileAt.java') + $addrs
} else {
    throw 'Give -Import or -Decompile ADDR[,ADDR...]'
}
$log = Join-Path $dir "$Project-$(Get-Date -Format yyyyMMdd-HHmmss).log"
$p = Start-Process -FilePath $headless -ArgumentList $args -NoNewWindow -PassThru -RedirectStandardOutput $log -RedirectStandardError "$log.err"
$p.PriorityClass = 'Idle'
$p.WaitForExit()
if ($Decompile.Count) {
    # The script's println lines carry the C; Ghidra prefixes them with the script name.
    Get-Content $log | Where-Object { $_ -match 'DecompileAt\.java> ' } | ForEach-Object { $_ -replace '^.*DecompileAt\.java> ', '' -replace ' \(GhidraScript\)\s*$', '' }
} else {
    Get-Content $log -Tail 5
}
exit $p.ExitCode
