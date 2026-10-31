# Combined emulator test: 60 Hz native-clock patch + D1_FLIP_RATE=0 + D1 mouse cursor detour, orbit lab.
#   emu-test.ps1 -Name emuNN
# The lab must already be stopped (by exact PID).
param([Parameter(Mandatory)][string]$Name)
if (Get-Process d1-activity-host, d1-responder, shadnet-server, shadPS4 -ErrorAction SilentlyContinue) { throw 'lab still running' }
$C = 'F:\d1\d1preservation-main\cfw-staging'
New-Item -ItemType Directory -Force "$C\$Name" | Out-Null
$env:D1_AUDIO_TAP = 'F:\d1\tools\shadPS4\audio-tap'; $env:D1_ROOT = 'F:\d1'
$env:D1_FLIP_RATE = '0'; $env:D1_MOUSE_CURSOR = '1'
Start-Process pwsh -WindowStyle Hidden -ArgumentList @('-NoProfile', '-File', 'F:\d1\tools\d1-lab-served.ps1', '-Name', $Name, '-Minutes', '60',
    '-ClientPatch', 'F:\d1\d1preservation-main\patches\CUSA00219-56578-lab-orbit-60hz-native-clock.xml') `
    -RedirectStandardOutput "$C\$Name\boot.log" -RedirectStandardError "$C\$Name\boot-error.log" | Out-Null
"$Name launched at $(Get-Date -Format HH:mm:ss)"
