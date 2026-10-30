# Destiny 1 (PS4) lab network for shadPS4 on Windows: an isolated, non-loopback guest address.
#   apply  : configure the "Microsoft KM-TEST Loopback Adapter" as "D1 Lab" with 10.77.0.2/24 (no
#            gateway, no DNS registration), point the D1 hosts redirects at 10.77.0.2, and narrow the
#            shadPS4 outbound block to every address EXCEPT 10.77.0.0/24.
#   remove : restore 127.0.0.1 redirects and the full outbound block; remove the lab address.
# The game rejects a loopback-only transport (error marionberry / transport_method_invalid), which
# is why it needs a real-looking interface. The adapter is virtual: nothing on it leaves this PC.
# Run elevated via d1-lab-net.cmd (apply) or d1-lab-net-remove.cmd (remove).
param([ValidateSet('apply','remove')][string]$Mode = 'apply')
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\D1Paths.ps1"
if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Host 'Right-click the .cmd and choose Run as administrator.' -ForegroundColor Red; Read-Host 'Enter to close'; exit 1
}
$labIp   = '10.77.0.2'
$alias   = 'D1 Lab'
$exe     = Get-D1Path ShadPS4 'shadPS4.exe'
$rule    = 'D1 shadPS4 - block outbound'
$hosts   = "$env:SystemRoot\System32\drivers\etc\hosts"
$begin   = "# >>> D1 local redirects ($(Get-D1Path Tools 'contain-d1.ps1'))"
$end     = '# <<< D1 local redirects'
$names   = Get-Content -LiteralPath (Get-D1Path Tools 'shadPS4\d1-hosts.txt') | Where-Object { $_ -match '^\s*127\.0\.0\.1\s' } | ForEach-Object { ($_ -split '\s+')[1] }

function Write-Hosts([string]$target) {
    $lines = @(Get-Content -LiteralPath $hosts)
    $keep = New-Object System.Collections.Generic.List[string]; $skip = $false
    foreach ($l in $lines) { if ($l -eq $begin) { $skip = $true; continue }; if ($l -eq $end) { $skip = $false; continue }; if (-not $skip) { $keep.Add($l) } }
    Copy-Item -LiteralPath $hosts -Destination "$hosts.d1-backup-$(Get-Date -Format yyyyMMdd-HHmmss)"
    $keep.Add($begin); $names | ForEach-Object { $keep.Add(('{0,-11} {1}' -f $target, $_)) }; $keep.Add($end)
    $tmp = Join-Path $env:TEMP "hosts.d1.$PID.tmp"
    [IO.File]::WriteAllLines($tmp, [string[]]$keep, [Text.Encoding]::ASCII)
    for ($i = 1; $i -le 10; $i++) { try { Copy-Item -LiteralPath $tmp -Destination $hosts -Force -ErrorAction Stop; Remove-Item $tmp; return } catch { Start-Sleep -Milliseconds 500 } }
    throw "Could not write $hosts; previous content is in $tmp"
}
function Set-Block([string[]]$remote) {
    Get-NetFirewallRule -DisplayName $rule -ErrorAction SilentlyContinue | Remove-NetFirewallRule
    New-NetFirewallRule -DisplayName $rule -Direction Outbound -Program $exe -Action Block -Profile Any `
        -RemoteAddress $remote -Description 'Destiny 1 preservation: the emulator may only talk to local servers.' | Out-Null
}

if ($Mode -eq 'apply') {
    $nic = Get-NetAdapter -IncludeHidden | Where-Object { $_.InterfaceDescription -like 'Microsoft KM-TEST Loopback Adapter*' } | Select-Object -First 1
    if (-not $nic) {
        Write-Host 'The Microsoft KM-TEST Loopback Adapter is not installed yet. Opening the hardware wizard:' -ForegroundColor Yellow
        Write-Host '  Next > Install the hardware that I manually select > Network adapters > Next >'
        Write-Host '  Manufacturer: Microsoft > Model: Microsoft KM-TEST Loopback Adapter > Next > Finish.'
        Write-Host 'Then run this .cmd again.'
        Start-Process hdwwiz.exe; Read-Host 'Enter to close'; exit 2
    }
    if ($nic.Name -ne $alias) { Rename-NetAdapter -Name $nic.Name -NewName $alias; $nic = Get-NetAdapter -Name $alias }
    Get-NetIPAddress -InterfaceAlias $alias -AddressFamily IPv4 -ErrorAction SilentlyContinue | Remove-NetIPAddress -Confirm:$false -ErrorAction SilentlyContinue
    Set-NetIPInterface -InterfaceAlias $alias -AddressFamily IPv4 -Dhcp Disabled -InterfaceMetric 9999
    New-NetIPAddress -InterfaceAlias $alias -IPAddress $labIp -PrefixLength 24 | Out-Null   # no DefaultGateway
    Set-DnsClient -InterfaceAlias $alias -RegisterThisConnectionsAddress $false
    Set-DnsClientServerAddress -InterfaceAlias $alias -ResetServerAddresses
    Disable-NetAdapterBinding -Name $alias -ComponentID ms_tcpip6 -ErrorAction SilentlyContinue
    Write-Hosts $labIp
    # Block every remote address except the lab /24 (block rules beat allow rules, so carve it out).
    Set-Block @('0.0.0.0-10.76.255.255', '10.78.0.0-255.255.255.255', '::-ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff')
    Write-Host "Applied: '$alias' = $labIp/24 (no gateway), $($names.Count) redirects -> $labIp, shadPS4 blocked except 10.77.0.0/24." -ForegroundColor Green
} else {
    Get-NetIPAddress -InterfaceAlias $alias -AddressFamily IPv4 -ErrorAction SilentlyContinue | Remove-NetIPAddress -Confirm:$false -ErrorAction SilentlyContinue
    Write-Hosts '127.0.0.1'
    Set-Block @('Any')
    Write-Host "Removed the lab address; redirects back to 127.0.0.1; full shadPS4 outbound block restored." -ForegroundColor Green
}
ipconfig /flushdns | Out-Null
Read-Host 'Enter to close'
