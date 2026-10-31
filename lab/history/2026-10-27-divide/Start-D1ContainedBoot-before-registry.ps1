param(
    [Parameter(Mandatory)][ValidatePattern('^[A-Za-z0-9_-]{1,64}$')][string]$Name,
    [string]$Build = '',
    [string]$Repo = 'F:\d1\d1preservation-main',
    [string]$Emulator = 'F:\d1\tools\shadPS4\0.19.0\shadPS4.exe',
    [string]$HostsList = 'F:\d1\tools\shadPS4\d1-hosts.txt',
    [string]$Distro = 'Ubuntu-24.04',
    [string]$ServerScript = 'F:\d1\tools\wsl-servers-loopback.sh',
    [int]$Minutes = 8,
    [int]$TitleDelaySeconds = 60,
    [int]$TitleTaps = 3,
    [int]$TapIntervalSeconds = 12,
    [string]$TitleKey = 'NumPad2',
    # bootflow:profile_setup is the local first-run wizard (screen bounds, brightness, subtitles).
    # It advances only on confirm, and left alone the client halts with a hitch. While the emulator
    # log says the client is in that state, tap $TitleKey every $WizardIntervalSeconds, at most
    # $WizardTaps times. Ignored with -OperatorInput.
    [int]$WizardTaps = 15,
    [int]$WizardIntervalSeconds = 4,
    [string]$Manifest = '',
    # Explicit, address-specific client patch (synthetic). Never applied by default.
    [string]$ClientPatch = '',
    [switch]$OperatorInput,
    # By default each virtual tap first brings the emulator window to the foreground and is then
    # sent as real keyboard input: the emulator ignores keys posted to an unfocused window, so the
    # profile_setup wizard never saw them. -NoForeground restores the posted-message-only taps.
    [switch]$NoForeground,
    # Extra responder knobs for this run, as KEY=VALUE (lab mode). They override the canonical set
    # below and are recorded in server-environment.txt with it.
    [string[]]$ExtraServerEnv = @(),
    # Lab mode: also run d1-activity-host on <LabIp>:4110 and point BAP messages 7/17 at it (the
    # rung-30 deliverable; see capture-rig/run-boot.sh for why the three values must agree).
    [switch]$ActivityHost,
    [string]$Activity = '',
    [switch]$AllowUnredirected,
    # The isolated "D1 Lab" adapter address from d1-lab-net.cmd (the game rejects a loopback-only
    # transport). Empty = the original loopback setup.
    [ValidateSet('', '10.77.0.2')][string]$LabIp = ''
)
$ErrorActionPreference = 'Stop'
# A contained Windows boot of the selected build: shadPS4 on Windows, the existing servers in WSL
# bound to loopback. Containment is checked first and the run refuses on any failure: the exact
# emulator has an enabled outbound block on every profile, all firewall profiles are on, and the
# hosts redirects are applied. No client patch is passed and automatic patch loading is disabled
# (-i): patches are address-specific. Evidence stays in private staging.

function Linux([string]$p) { '/mnt/' + $p.Substring(0, 1).ToLowerInvariant() + ($p.Substring(2) -replace '\\', '/') }
$registry = Get-Content -LiteralPath (Join-Path $Repo 'capture-rig\builds.json') -Raw | ConvertFrom-Json
if ($registry.schema -ne 1) { throw 'Unsupported build registry schema' }
if (-not $Build) { $Build = if ($env:D1_BUILD) { $env:D1_BUILD } else { $registry.default } }
$entry = $registry.builds.$Build
if ($null -eq $entry) { throw "Unknown build: $Build" }
$launch = Join-Path $Repo $entry.launch
if (-not $Manifest) { $Manifest = Join-Path $Repo $entry.manifest }
$out = Join-Path $Repo "cfw-staging\contained-boot\$((Get-Date).ToUniversalTime().ToString('yyyyMMdd-HHmmss'))-$Name"
if (Test-Path -LiteralPath $out) { throw "Refusing existing output: $out" }
New-Item -ItemType Directory -Path $out | Out-Null
$checks = [ordered]@{}
function Check([string]$label, [bool]$ok, $detail) { $checks[$label] = [ordered]@{ pass = $ok; detail = $detail } }

# -- containment ----------------------------------------------------------------------------------
$profiles = Get-NetFirewallProfile
Check 'all firewall profiles enabled' (@($profiles | Where-Object { -not $_.Enabled }).Count -eq 0) (@($profiles | ForEach-Object { "$($_.Name)=$($_.Enabled)" }) -join ' ')
$exe = (Get-Item -LiteralPath $Emulator).FullName
$blocks = @(Get-NetFirewallRule -PolicyStore ActiveStore -Direction Outbound -Action Block -Enabled True -ErrorAction SilentlyContinue |
    Where-Object { $_.Profile -eq 'Any' -and (($_ | Get-NetFirewallApplicationFilter).Program -eq $exe) })
Check 'enabled outbound block for the exact emulator, all profiles' ($blocks.Count -ge 1) (@($blocks.DisplayName) -join '; ')
$target = if ($LabIp) { $LabIp } else { '127.0.0.1' }
if ($LabIp) {
    $labAddr = Get-NetIPAddress -IPAddress $LabIp -AddressFamily IPv4 -ErrorAction SilentlyContinue
    $nic = if ($labAddr) { Get-NetAdapter -InterfaceIndex $labAddr.InterfaceIndex -IncludeHidden } else { $null }
    Check 'lab address on the virtual KM-TEST adapter' ([bool]$nic -and $nic.InterfaceDescription -like 'Microsoft KM-TEST Loopback Adapter*') ([ordered]@{ alias = $labAddr.InterfaceAlias; description = $nic.InterfaceDescription })
    $labRoutes = @(if ($labAddr) { Get-NetRoute -InterfaceIndex $labAddr.InterfaceIndex -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue })
    Check 'lab adapter has no default route' ($labRoutes.Count -eq 0) $labRoutes.Count
    $blockRanges = @(foreach ($b in $blocks) { ($b | Get-NetFirewallAddressFilter).RemoteAddress }) | Sort-Object -Unique
    $expectedRanges = @('0.0.0.0-10.76.255.255', '10.78.0.0-255.255.255.255', '::-ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff') | Sort-Object
    Check 'emulator block covers everything except 10.77.0.0/24' ((@($blockRanges) -join ',') -eq ($expectedRanges -join ',')) $blockRanges
}
$hostsText = Get-Content -LiteralPath "$env:SystemRoot\System32\drivers\etc\hosts"
$wanted = @(Get-Content -LiteralPath $HostsList | Where-Object { $_ -match '^\s*127\.0\.0\.1\s+(\S+)' } | ForEach-Object { ($_ -split '\s+')[1] })
$missing = @($wanted | Where-Object { $n = $_; -not ($hostsText | Where-Object { $_ -match "^\s*$([regex]::Escape($target))\s+$([regex]::Escape($n))\s*$" }) })
Check 'hosts redirects applied' ($missing.Count -eq 0 -or $AllowUnredirected) ([ordered]@{ wanted = $wanted.Count; missing = $missing; allowedBecauseFirewallBlocks = [bool]$AllowUnredirected })
foreach ($hostName in $wanted | Where-Object { $missing -notcontains $_ }) {
    $resolved = @(try { [Net.Dns]::GetHostAddresses($hostName) | ForEach-Object { $_.ToString() } } catch { 'UNRESOLVED' })
    if (@($resolved | Where-Object { $_ -ne $target -and $_ -ne '::1' }).Count) { Check "resolves to ${target}: $hostName" $false $resolved }
}
# shadPS4 0.19's -i also drops an explicit -p patch, so with -ClientPatch the emulator's automatic
# patch folder must be empty instead.
$autoPatches = @(Get-ChildItem -LiteralPath (Join-Path (Split-Path $exe) 'user\patches') -Recurse -File -ErrorAction SilentlyContinue)
Check 'emulator automatic patch folder empty' ($autoPatches.Count -eq 0) $autoPatches.Count
$running = @(Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -match '^shadps4$|^rpcs3$' })
Check 'no emulator already running' ($running.Count -eq 0) (@($running.Id) -join ' ')

# -- inputs ---------------------------------------------------------------------------------------
$launchItem = Get-Item -LiteralPath $launch
$launchSha = (Get-FileHash -LiteralPath $launch).Hash.ToLowerInvariant()
Check "launch executable is build $Build" ($launchItem.Length -eq $entry.launch_bytes -and $launchSha -eq $entry.launch_sha256) "$($launchItem.Length) $launchSha"
$sfo = Join-Path (Split-Path $launch) 'sce_sys\param.sfo'
Check 'param.sfo beside the executable' (Test-Path -LiteralPath $sfo) $sfo
$manifestItem = Get-Item -LiteralPath $Manifest -ErrorAction SilentlyContinue
$manifestSha = if ($manifestItem) { (Get-FileHash -LiteralPath $Manifest).Hash.ToLowerInvariant() } else { $null }
$manifestPinned = $entry.manifest_sha256 -and $manifestSha -eq $entry.manifest_sha256 -and $manifestItem.Length -eq $entry.manifest_bytes
Check 'served manifest matches the recorded build pin' $manifestPinned ([ordered]@{ path = $Manifest; sha256 = $manifestSha })

$receipt = [ordered]@{
    name = $Name; labIp = $(if ($LabIp) { $LabIp } else { $null }); build = $Build; build_label = $entry.build_label; addresses_valid = $entry.addresses_valid
    startedUtc = [DateTime]::UtcNow.ToString('o'); emulator = $exe
    emulatorSha256 = (Get-FileHash -LiteralPath $exe).Hash.ToLowerInvariant()
    launch = $launch; autoPatchesDisabled = (-not $ClientPatch); autoPatchFolderEmpty = ($autoPatches.Count -eq 0)
    clientPatch = if ($ClientPatch) { [ordered]@{ path = (Get-Item -LiteralPath $ClientPatch).FullName
        sha256 = (Get-FileHash -LiteralPath $ClientPatch).Hash.ToLowerInvariant(); synthetic = $true } } else { $null }
    input = if ($OperatorInput) { 'operator' } else { "virtual key $TitleKey x$TitleTaps after ${TitleDelaySeconds}s; up to $WizardTaps wizard taps every ${WizardIntervalSeconds}s while in profile_setup" }
    checks = $checks
}
$failed = @($checks.Keys | Where-Object { -not $checks[$_].pass })
if ($failed.Count) {
    $receipt.refused = $failed
    $receipt | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $out 'receipt.json') -Encoding utf8
    throw "Refusing launch: $($failed -join '; ')"
}

# -- servers --------------------------------------------------------------------------------------
# Loopback runs use the WSL servers (mirrored networking forwards 127.0.0.1). With -LabIp the same
# servers run natively on Windows: under mirrored networking Windows owns 10.77.0.2 itself, so a
# Windows client never reaches a WSL listener on that address.
$winServers = @()
function Stop-WinServers {
    foreach ($p in $winServers) { if (-not $p.HasExited) { Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue; $p.WaitForExit(10000) | Out-Null } }
    $alive = @($winServers | Where-Object { -not $_.HasExited } | ForEach-Object { $_.Id })
    "stopped $([DateTime]::UtcNow.ToString('o')): $(@($winServers.Id) -join ' ') alive=$($alive -join ' ')" |
        Set-Content -LiteralPath (Join-Path $out 'servers-stopped.txt')
}
if ($LabIp) {
    $bin = Join-Path $Repo 'd1-server\target\release'
    $openssl = 'C:\Program Files\Git\mingw64\bin\openssl.exe'
    & $openssl req -x509 -newkey rsa:2048 -nodes -keyout (Join-Path $out 'tls.key') -out (Join-Path $out 'tls.crt') -days 2 `
        -subj '/CN=signon.deadorbit.net' `
        -addext 'subjectAltName=DNS:signon.deadorbit.net,DNS:signon.gravityshavings.net,DNS:www1.signon.deadorbit.net,DNS:www1.signon.gravityshavings.net' 2>$null
    if (-not (Test-Path -LiteralPath (Join-Path $out 'tls.key'))) { throw 'Lab certificate generation failed' }
    $busy = @(Get-NetTCPConnection -State Listen -ErrorAction SilentlyContinue | Where-Object { $_.LocalPort -in 80,443,8080,8788,3075,31313,31315,32556 })
    if ($busy.Count) { throw "Refusing: server ports already in use: $(@($busy | ForEach-Object { "$($_.LocalAddress):$($_.LocalPort)" }) -join ' ')" }
    # The canonical responder knobs (same as wsl-servers-loopback.sh), transport pointed at the lab.
    $serverEnv = [ordered]@{
        D1_ACCOUNT_SOID = '0xD100000000000001'; D1_CHARACTER_SOID = '0xD100000000000101'; D1_503_SOID = '1'
        # The profile fixture is a serialised 01.29 profile object; other builds get the sparse
        # encoder's minimal profile (SOID + orbit destination) with their own instance checksum.
        D1_CHARACTER_LIFECYCLE = '1'
        D1_PROFILE_FILE = $(if ($Build -eq $registry.reference) { Join-Path $Repo 'capture-rig\profiles\profile-old-russia.bin' } else { '' })
        D1_PUSH_CHARACTER = '1'; D1_PUSH_FAMILY = '4,3'; D1_PUSH_AT = 'echo'; D1_ACTIVITY_HOST = '0'
        D1_IP = $LabIp; D1_BAP_IP = $LabIp; D1_CLIENT_IP = $LabIp; D1_MANIFEST_FILE = $Manifest
        D1_UPLOAD_DIR = (Join-Path $out 'uploads'); D1_CHARACTER_TRANSCRIPT_DIR = (Join-Path $out 'character-transcripts')
    }
    if ($ActivityHost) {
        $serverEnv['D1_ACTIVITY_HOST'] = '1'
        $serverEnv['D1_AH_HOST_TOKEN'] = '0xD1A0000000000001'
        $serverEnv['D1_AH_HOST_TEXT'] = 'd1ah-d1a0000000000001'
        $serverEnv['D1_AH_PORT'] = '4110'
        if ($Activity) { $serverEnv['D1_AH_ACTIVITY'] = $Activity }
    }
    foreach ($kv in $ExtraServerEnv) {
        if ($kv -notmatch '^(D1_[A-Z0-9_]+)=(.*)$') { throw "ExtraServerEnv entry is not D1_KEY=VALUE: $kv" }
        $serverEnv[$Matches[1]] = $Matches[2]
    }
    foreach ($k in $serverEnv.Keys) { Set-Item -Path "env:$k" -Value $serverEnv[$k] }
    $serverEnv.GetEnumerator() | ForEach-Object { "$($_.Key)=$($_.Value)" } | Set-Content -LiteralPath (Join-Path $out 'server-environment.txt')
    $cert = @('--cert', (Join-Path $out 'tls.crt'), '--key', (Join-Path $out 'tls.key'))
    $winServers += Start-Process -FilePath (Join-Path $bin 'shadnet-server.exe') -ArgumentList @('--bind', '127.0.0.1:31313') -PassThru -WindowStyle Hidden `
        -RedirectStandardOutput (Join-Path $out 'shadnet.log') -RedirectStandardError (Join-Path $out 'shadnet.err.log')
    $winServers += Start-Process -FilePath (Join-Path $bin 'd1-responder.exe') -ArgumentList (@('--bind', $LabIp) + $cert) -PassThru -WindowStyle Hidden `
        -RedirectStandardOutput (Join-Path $out 'responder.log') -RedirectStandardError (Join-Path $out 'responder.err.log')
    if ($ActivityHost) {
        $ahArgs = @('serve', '--bind', "${LabIp}:4110")
        if ($Activity) { $ahArgs += @('--activity', $Activity) }
        $winServers += Start-Process -FilePath (Join-Path $bin 'd1-activity-host.exe') -ArgumentList $ahArgs -PassThru -WindowStyle Hidden `
            -RedirectStandardOutput (Join-Path $out 'activity-host.log') -RedirectStandardError (Join-Path $out 'activity-host.err.log')
    }
    # The emulator's own WebAPI (friend list) is configured on 127.0.0.1:31315.
    $winServers += Start-Process -FilePath (Join-Path $bin 'd1-responder.exe') -ArgumentList (@('--bind', '127.0.0.1') + $cert) -PassThru -WindowStyle Hidden `
        -RedirectStandardOutput (Join-Path $out 'responder-loopback.log') -RedirectStandardError (Join-Path $out 'responder-loopback.err.log')
    $deadline = (Get-Date).AddSeconds(30)
    do {
        Start-Sleep -Milliseconds 300
        $listen = @(Get-NetTCPConnection -State Listen -ErrorAction SilentlyContinue | Where-Object { $_.OwningProcess -in $winServers.Id })
        $ready = @($listen | Where-Object { $_.LocalAddress -eq $LabIp -and $_.LocalPort -eq 443 }).Count -and
                 @($listen | Where-Object { $_.LocalAddress -eq '127.0.0.1' -and $_.LocalPort -eq 31313 }).Count
        if (@($winServers | Where-Object HasExited).Count -or (Get-Date) -gt $deadline) { Stop-WinServers; throw 'Windows servers did not become ready' }
    } until ($ready)
    $udp = @(Get-NetUDPEndpoint -ErrorAction SilentlyContinue | Where-Object { $_.OwningProcess -in $winServers.Id })
    $listeners = @($listen | ForEach-Object { "tcp $($_.LocalAddress):$($_.LocalPort)" }) + @($udp | ForEach-Object { "udp $($_.LocalAddress):$($_.LocalPort)" })
    $listeners | Set-Content -LiteralPath (Join-Path $out 'listeners.txt')
    $outside = @($listen + $udp | Where-Object { $_.LocalAddress -notin @('127.0.0.1', $LabIp) })
    if ($outside.Count) { Stop-WinServers; throw "Refusing: a server listener is outside loopback/lab: $(@($outside | ForEach-Object { "$($_.LocalAddress):$($_.LocalPort)" }) -join ' ')" }
    'ready' | Set-Content -LiteralPath (Join-Path $out 'servers.ready')
} else {
$linuxOut = Linux $out
$serverLog = Join-Path $out 'wsl-servers.console.log'
$server = Start-Process -FilePath 'wsl.exe' -PassThru -WindowStyle Hidden `
    -ArgumentList @('-d', $Distro, '-u', 'root', '--', 'bash', (Linux $ServerScript), $linuxOut, (Linux $Manifest), $target) `
    -RedirectStandardOutput $serverLog -RedirectStandardError "$serverLog.err"
$deadline = (Get-Date).AddSeconds(45)
while (-not (Test-Path -LiteralPath (Join-Path $out 'servers.ready'))) {
    if ($server.HasExited -or (Get-Date) -gt $deadline) {
        New-Item -ItemType File -Path (Join-Path $out 'stop') -Force | Out-Null
        throw "Servers did not become ready; see $serverLog"
    }
    Start-Sleep -Milliseconds 250
}
}
$tcp = Test-NetConnection -ComputerName $target -Port 443 -WarningAction SilentlyContinue
$receipt.windowsReaches443 = [ordered]@{ address = $target; ok = $tcp.TcpTestSucceeded }

# -- emulator -------------------------------------------------------------------------------------
Add-Type -Namespace D1 -Name Win -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
[DllImport("user32.dll")] public static extern uint MapVirtualKey(uint code, uint type);
[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
[DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
[DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
[DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
[StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
'@
Add-Type -AssemblyName System.Drawing
function Snap([IntPtr]$h, [string]$file) {
    $r = New-Object D1.Win+RECT
    if (-not [D1.Win]::GetWindowRect($h, [ref]$r)) { return }
    $w = $r.Right - $r.Left; $hh = $r.Bottom - $r.Top
    if ($w -le 0 -or $hh -le 0) { return }
    $bmp = New-Object Drawing.Bitmap $w, $hh
    $g = [Drawing.Graphics]::FromImage($bmp)
    # A locked or disconnected session makes CopyFromScreen throw; a missing frame must never
    # abort the run (and orphan the emulator and servers).
    try { $g.CopyFromScreen($r.Left, $r.Top, 0, 0, $bmp.Size); $bmp.Save($file, [Drawing.Imaging.ImageFormat]::Png) }
    catch { Add-Content -LiteralPath (Join-Path $out 'frames-skipped.log') "$([DateTime]::UtcNow.ToString('o')) $($_.Exception.Message)" }
    finally { $g.Dispose(); $bmp.Dispose() }
}
function Tap([IntPtr]$h, [string]$key) {
    $vk = [uint32][Windows.Forms.Keys]::$key
    $scan = [D1.Win]::MapVirtualKey($vk, 0)
    if (-not $NoForeground) {
        if ([D1.Win]::GetForegroundWindow() -ne $h) {
            # A background process may only take the foreground right after keyboard input; a
            # bare Alt press/release grants that without typing anything.
            [D1.Win]::keybd_event(0x12, 0, 0, [UIntPtr]::Zero); [D1.Win]::keybd_event(0x12, 0, 2, [UIntPtr]::Zero)
            [void][D1.Win]::ShowWindow($h, 9)
            [void][D1.Win]::SetForegroundWindow($h)
            Start-Sleep -Milliseconds 300
        }
        if ([D1.Win]::GetForegroundWindow() -eq $h) {
            [D1.Win]::keybd_event([byte]$vk, [byte]$scan, 0, [UIntPtr]::Zero)
            Start-Sleep -Milliseconds 120
            [D1.Win]::keybd_event([byte]$vk, [byte]$scan, 2, [UIntPtr]::Zero)
            return 'foreground'
        }
    }
    [void][D1.Win]::PostMessage($h, 0x100, [IntPtr]$vk, [IntPtr](1 -bor ($scan -shl 16)))
    Start-Sleep -Milliseconds 120
    [void][D1.Win]::PostMessage($h, 0x101, [IntPtr]$vk, [IntPtr]((1 -bor ($scan -shl 16)) -bor 0xC0000000))
    'posted'
}
Add-Type -AssemblyName System.Windows.Forms
$userDir = Join-Path (Split-Path $exe) 'user'
# The D1 shadPS4 build (shadps4-0.19.0-local-ip.patch) reports this as the guest's own address;
# a stock build ignores it and falls back to its blocked 1.1.1.1 probe.
if ($LabIp) { $env:D1_GUEST_IP = $LabIp } else { Remove-Item env:D1_GUEST_IP -ErrorAction SilentlyContinue }
$receipt.guestIpOverride = $env:D1_GUEST_IP
$started = Get-Date
$emu = Start-Process -FilePath $exe -WorkingDirectory (Split-Path $exe) -PassThru `
    -ArgumentList ($(if ($ClientPatch) { @('-p', "`"$((Get-Item -LiteralPath $ClientPatch).FullName)`"") } else { @('-i') }) + @('-g', "`"$launch`""))
$receipt.emulatorPid = $emu.Id
$taps = 0; $nextSnap = $started.AddSeconds(15); $shot = 0
$shadLog = Join-Path $userDir 'log\shad_log.txt'
$logPos = 0L; $inWizard = $false; $wizardTapCount = 0; $nextWizardTap = Get-Date; $nextLogRead = Get-Date
$end = $started.AddMinutes($Minutes)
while (-not $emu.HasExited -and (Get-Date) -lt $end) {
    Start-Sleep -Milliseconds 500
    $emu.Refresh()
    $h = $emu.MainWindowHandle
    if ($h -eq [IntPtr]::Zero) { continue }
    if ((Get-Date) -ge $nextSnap) {
        $shot++; Snap $h (Join-Path $out ('frame-{0:D3}.png' -f $shot)); $nextSnap = (Get-Date).AddSeconds(15)
    }
    if (-not $OperatorInput -and $taps -lt $TitleTaps -and
        (Get-Date) -ge $started.AddSeconds($TitleDelaySeconds + $taps * $TapIntervalSeconds)) {
        $how = Tap $h $TitleKey; $taps++
        Add-Content -LiteralPath (Join-Path $out 'input.log') "$([DateTime]::UtcNow.ToString('o')) tap $TitleKey ($how)"
    }
    if (-not $OperatorInput -and (Get-Date) -ge $nextLogRead -and (Test-Path -LiteralPath $shadLog)) {
        $nextLogRead = (Get-Date).AddSeconds(1)
        try {
            $fs = [IO.File]::Open($shadLog, 'Open', 'Read', 'ReadWrite, Delete')
            try {
                if ($fs.Length -lt $logPos) { $logPos = 0L }
                [void]$fs.Seek($logPos, 'Begin')
                $text = (New-Object IO.StreamReader($fs)).ReadToEnd()
                $logPos = $fs.Length
            } finally { $fs.Dispose() }
            if ($text -match "Entering state 'bootflow:profile_setup'") { $inWizard = $true }
            if ($text -match "Leaving state 'bootflow:profile_setup'") {
                $inWizard = $false
                Add-Content -LiteralPath (Join-Path $out 'input.log') "$([DateTime]::UtcNow.ToString('o')) left profile_setup after $wizardTapCount wizard tap(s)"
            }
        } catch { }
    }
    if ($inWizard -and $wizardTapCount -lt $WizardTaps -and (Get-Date) -ge $nextWizardTap) {
        $how = Tap $h $TitleKey; $wizardTapCount++; $nextWizardTap = (Get-Date).AddSeconds($WizardIntervalSeconds)
        Add-Content -LiteralPath (Join-Path $out 'input.log') "$([DateTime]::UtcNow.ToString('o')) wizard tap $TitleKey ($wizardTapCount, $how)"
    }
}
$receipt.wizardTaps = $wizardTapCount
$receipt.emulatorExitedByItself = $emu.HasExited
if (-not $emu.HasExited) { Stop-Process -Id $emu.Id; $emu.WaitForExit(15000) | Out-Null }
$receipt.emulatorExitCode = if ($emu.HasExited) { $emu.ExitCode } else { $null }
$receipt.endedUtc = [DateTime]::UtcNow.ToString('o')

# -- evidence -------------------------------------------------------------------------------------
New-Item -ItemType File -Path (Join-Path $out 'stop') | Out-Null
if ($LabIp) { Stop-WinServers } else { $server.WaitForExit(30000) | Out-Null }
$receipt.serverHost = if ($LabIp) { 'windows' } else { 'wsl' }
$receipt.serversStopped = Test-Path -LiteralPath (Join-Path $out 'servers-stopped.txt')
# shad_log.txt is the emulator's full log; shadps4.log holds only the launcher lines.
Copy-Item -LiteralPath (Join-Path $userDir 'log\shad_log.txt') -Destination (Join-Path $out 'emulator.log') -ErrorAction SilentlyContinue
Copy-Item -LiteralPath (Join-Path $userDir 'log\shadps4.log') -Destination (Join-Path $out 'launcher.log') -ErrorAction SilentlyContinue
$receipt.emulatorFault = @(Select-String -LiteralPath (Join-Path $out 'emulator.log') -Pattern 'Unhandled Exception code' -ErrorAction SilentlyContinue | ForEach-Object { $_.Line })
$guest = Join-Path $out 'guest-files'
foreach ($sub in 'temp', 'data', 'home') {
    $root = Join-Path $userDir $sub
    if (-not (Test-Path -LiteralPath $root)) { continue }
    Get-ChildItem -LiteralPath $root -Recurse -File | Where-Object { $_.LastWriteTime -ge $started } | ForEach-Object {
        $dest = Join-Path $guest ($sub + $_.FullName.Substring($root.Length))
        New-Item -ItemType Directory -Path (Split-Path $dest) -Force | Out-Null
        Copy-Item -LiteralPath $_.FullName -Destination $dest
    }
}
$receipt.guestCrashReport = @(Get-ChildItem -LiteralPath $guest -Recurse -Filter 'crash_info.txt' -ErrorAction SilentlyContinue).FullName
$receipt.nativeBootAccepted = $false
$receipt | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $out 'receipt.json') -Encoding utf8
"Run evidence: $out"
