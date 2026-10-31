# Destiny 1 (PS4) containment for shadPS4 on Windows.
#   apply  : hosts redirects (every game hostname -> 127.0.0.1) + outbound firewall block for shadPS4.exe
#   remove : undo both
# Run elevated via contain-d1.cmd (apply) or contain-d1-remove.cmd (remove).
param([ValidateSet('apply','remove')][string]$Mode = 'apply')
if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Host 'Right-click the .cmd and choose Run as administrator.' -ForegroundColor Red; Read-Host 'Enter to close'; exit 1
}
$hosts = "$env:SystemRoot\System32\drivers\etc\hosts"
$begin = '# >>> D1 local redirects (F:\d1\tools\contain-d1.ps1)'
$end   = '# <<< D1 local redirects'
$exe   = 'F:\d1\tools\shadPS4\0.19.0\shadPS4.exe'
$rule  = 'D1 shadPS4 - block outbound'

function Write-Hosts($content) {
    # Write through a temp file with retries: Defender often holds the hosts file open while scanning,
    # and Set-Content can truncate it and then fail ("Stream was not readable").
    $tmp = Join-Path $env:TEMP "hosts.d1.$PID.tmp"
    [IO.File]::WriteAllLines($tmp, [string[]]$content, [Text.Encoding]::ASCII)
    for ($i = 1; $i -le 10; $i++) {
        try { Copy-Item -LiteralPath $tmp -Destination $hosts -Force -ErrorAction Stop; Remove-Item $tmp; return }
        catch { Start-Sleep -Milliseconds 500 }
    }
    throw "Could not write $hosts (still locked). Your previous hosts content is in $tmp."
}

# If an earlier failed run left the hosts file empty, start from the newest non-empty backup.
$lines = @(Get-Content -LiteralPath $hosts)
if ($lines.Count -eq 0) {
    $bk = Get-ChildItem (Split-Path $hosts) -Filter 'hosts.d1-backup-*' | Where-Object Length -gt 0 | Sort-Object Name | Select-Object -Last 1
    if ($bk) { $lines = @(Get-Content -LiteralPath $bk.FullName); Write-Host "hosts was empty; restored from $($bk.Name)" -ForegroundColor Yellow }
}

# Strip any previous D1 block (idempotent).
$keep = New-Object System.Collections.Generic.List[string]; $skip = $false
foreach ($l in $lines) { if ($l -eq $begin) { $skip = $true; continue }; if ($l -eq $end) { $skip = $false; continue }; if (-not $skip) { $keep.Add($l) } }

if ($Mode -eq 'apply') {
    if ((Get-Item -LiteralPath $hosts).Length -gt 0) { Copy-Item -LiteralPath $hosts -Destination "$hosts.d1-backup-$(Get-Date -Format yyyyMMdd-HHmmss)" }
    $block = Get-Content -LiteralPath 'F:\d1\tools\shadPS4\d1-hosts.txt' | Where-Object { $_ -match '^\s*127\.0\.0\.1\s' }
    $keep.Add($begin); $block | ForEach-Object { $keep.Add($_) }; $keep.Add($end)
    Write-Hosts $keep
    Get-NetFirewallRule -DisplayName $rule -ErrorAction SilentlyContinue | Remove-NetFirewallRule
    New-NetFirewallRule -DisplayName $rule -Direction Outbound -Program $exe -Action Block -Profile Any `
        -Description 'Destiny 1 preservation: the emulator may only talk to local servers (loopback is not filtered).' | Out-Null
    Write-Host "Applied: $($block.Count) host redirects, firewall rule '$rule'." -ForegroundColor Green
} else {
    Write-Hosts $keep
    Get-NetFirewallRule -DisplayName $rule -ErrorAction SilentlyContinue | Remove-NetFirewallRule
    Write-Host 'Removed the D1 host redirects and firewall rule.' -ForegroundColor Green
}
ipconfig /flushdns | Out-Null
Read-Host 'Enter to close'
