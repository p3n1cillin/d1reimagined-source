[CmdletBinding()]
param(
    [string]$Repo = (& "$PSScriptRoot\D1Paths.ps1" Workspace),
    [string]$EmulatorDirectory = (& "$PSScriptRoot\D1Paths.ps1" ShadPS4),
    [string]$HostsList = (& "$PSScriptRoot\D1Paths.ps1" Tools 'shadPS4\d1-hosts.txt'),
    [string]$Report = (& "$PSScriptRoot\D1Paths.ps1" Staging '2026-10-11\readiness.json')
)
# Read-only preflight, apart from its report. This does not launch a game or alter containment.
$ErrorActionPreference = 'Stop'
$checks = [System.Collections.Generic.List[object]]::new()
function Check([string]$name, [bool]$pass, [string]$detail) {
    $checks.Add([ordered]@{name=$name;pass=$pass;detail=$detail})
}
function CheckHash([string]$name, [string]$path, [string]$hash, [long]$size = -1) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { Check $name $false "Missing: $path"; return }
    $file = Get-Item -LiteralPath $path
    $actual = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
    Check $name ($actual -eq $hash -and ($size -lt 0 -or $file.Length -eq $size)) "bytes=$($file.Length); sha256=$actual"
}
$game = Join-Path $Repo 'dumps\patch-01.29-CUSA00219'
CheckHash 'Decrypted CUSA00219 01.29 eboot' (Join-Path $game 'eboot.bin') '7271fcb926401df8defb126cb8eb2b247134138b81e401bbacb2ab60791b7954' 29148264
$packages = @(Get-ChildItem -LiteralPath (Join-Path $game 'packages') -File -Filter '*.pkg' -ErrorAction SilentlyContinue)
Check 'Installed Bungie content packages' ($packages.Count -eq 1647) "files=$($packages.Count); expected 1647 pinned-estate files; install PKGs do not satisfy this check"
CheckHash 'Full English content manifest' (Join-Path $Repo 'capture-rig\manifests\full_en.bin') '0c5bba3c63e9b68021b536513639cfc8896ec736950323e4ab06c5d7ed0770be' 52797
$patch = Join-Path $Repo 'patches\CUSA00219-lab-manifest-T1.xml'
try { $xml = [xml](Get-Content -LiteralPath $patch -Raw); Check 'Lab patch XML' $true $patch }
catch { Check 'Lab patch XML' $false $_.Exception.Message }
$exe = Join-Path $EmulatorDirectory 'shadPS4.exe'
Check 'Portable emulator' (Test-Path -LiteralPath $exe -PathType Leaf) $exe
try {
    $general = (Get-Content -LiteralPath (Join-Path $EmulatorDirectory 'user\config.json') -Raw | ConvertFrom-Json).General
    $local = $general.shad_net_enabled -eq $true -and $general.connected_to_network -eq $true -and
        $general.enable_upnp -eq $false -and $general.shadnet_server -eq '127.0.0.1:31313' -and
        $general.shadnet_webapi_server -eq 'http://127.0.0.1:31315'
    Check 'Local shadNet configuration' $local 'shadNet and WebAPI loopback; UPnP disabled'
} catch { Check 'Local shadNet configuration' $false $_.Exception.Message }
try {
    $profiles = @(Get-NetFirewallProfile -PolicyStore ActiveStore)
    Check 'Windows firewall profiles active' ($profiles.Count -eq 3 -and @($profiles | Where-Object { -not $_.Enabled }).Count -eq 0) 'All three profiles must be enabled'
    $rules = @(Get-NetFirewallRule -PolicyStore ActiveStore -DisplayName 'D1 shadPS4 - block outbound' -ErrorAction Stop)
    $valid = @($rules | Where-Object { $_.Enabled -eq 'True' -and $_.Direction -eq 'Outbound' -and $_.Action -eq 'Block' -and $_.Profile -eq 'Any' } |
        Get-NetFirewallApplicationFilter | Where-Object { $_.Program -eq $exe })
    Check 'Effective emulator outbound block' ($valid.Count -gt 0) 'ActiveStore; exact executable; all profiles'
} catch { Check 'Effective emulator outbound block' $false $_.Exception.Message }
try {
    $hostsText = Get-Content -LiteralPath "$env:SystemRoot\System32\drivers\etc\hosts"
    $expectedHosts = @(Get-Content -LiteralPath $HostsList | Where-Object { $_ -match '^127\.0\.0\.1\s+\S+' } | ForEach-Object { ($_ -split '\s+')[1] } | Sort-Object -Unique)
    $badHosts = @()
    foreach ($hostName in $expectedHosts) {
        $mapping = @($hostsText | Where-Object { ($_ -split '#',2)[0] -match ('^\s*127\.0\.0\.1\s+' + [regex]::Escape($hostName) + '\s*$') })
        $addresses = @([System.Net.Dns]::GetHostAddresses($hostName))
        if ($mapping.Count -eq 0 -or $addresses.Count -eq 0 -or @($addresses | Where-Object { -not [System.Net.IPAddress]::IsLoopback($_) }).Count -gt 0) { $badHosts += $hostName }
    }
    Check 'Windows service redirects' ($expectedHosts.Count -eq 50 -and $badHosts.Count -eq 0) "unique hosts=$($expectedHosts.Count); failed=$($badHosts -join ','); reference has 51 rows including duplicate CDN"
} catch { Check 'Windows service redirects' $false $_.Exception.Message }
$moduleDirectory = Join-Path $EmulatorDirectory 'user\sys_modules'
$moduleFailures = @()
$moduleCount = 0
try {
    foreach ($line in Get-Content -LiteralPath (Join-Path $moduleDirectory 'MANIFEST-11.00.txt')) {
        if ($line -match '^([a-fA-F0-9]{64})\s+(.+)$') {
            $moduleCount++
            $want = $Matches[1]; $module = $Matches[2]
            $path = Join-Path $moduleDirectory $module
            if (-not (Test-Path -LiteralPath $path -PathType Leaf) -or (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -ne $want) { $moduleFailures += $module }
        }
    }
    Check 'Firmware 11.00 module receipt' ($moduleCount -eq 37 -and $moduleFailures.Count -eq 0) "modules=$moduleCount; failed=$($moduleFailures -join ','); upstream reference used firmware 10.01"
} catch { Check 'Firmware 11.00 module receipt' $false $_.Exception.Message }
Check 'No existing emulator process' (@(Get-Process shadPS4,rpcs3 -ErrorAction SilentlyContinue).Count -eq 0) 'One emulator at a time'
CheckHash 'Canonical preparation launcher installed' (Join-Path $Repo 'capture-rig\run-boot.sh') '22536378f698e53671848fa4307e9b0bbfccf21510c383f5545eb5c3b8d1cac6'
CheckHash 'Canonical server-check delegate installed' (Join-Path $Repo 'capture-rig\run-signon-experiment.sh') '6a3b2c89b098d9bd00f7dae2e0a0e7f5557a9bd483b7fc92af0e733c2c190d2d'
# Static file/config checks cannot establish a successful Windows/WSL runtime port.
Check 'Native Windows/WSL runtime validated' $false 'Canonical preparation and contained Linux servers validated separately. Windows guest transport, probe hooks and dump-dependent fixtures remain unaccepted.'
$result = [ordered]@{atUtc=[DateTime]::UtcNow.ToString('o');ready=(@($checks | Where-Object {-not $_.pass}).Count -eq 0);checks=$checks}
New-Item -ItemType Directory -Path (Split-Path -Parent $Report) -Force | Out-Null
$result | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $Report -Encoding utf8
$result | ConvertTo-Json -Depth 6
if (-not $result.ready) { exit 2 }
