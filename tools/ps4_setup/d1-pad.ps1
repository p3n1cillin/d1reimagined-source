param(
    # Keys to press in order, as System.Windows.Forms.Keys names (NumPad2 = cross, NumPad6 = circle,
    # NumPad8 = triangle, NumPad4 = square, Return = options, Up/Down/Left/Right = d-pad, W/A/S/D =
    # left stick, Q/U = L1/R1, E/O = L2/R2). "Hold:<key>:<ms>" holds a key.
    [Parameter(Mandatory)][string[]]$Keys,
    [int]$GapMilliseconds = 700,
    # Save a frame of the window after the last key.
    [string]$Shot = ''
)
$ErrorActionPreference = 'Stop'
# Through `powershell -File` an array arrives as ONE comma-joined string ("Hold:S:600,NumPad2"), and
# such a key name matched nothing, so every multi-key call silently pressed nothing: split it here.
$Keys = @($Keys | ForEach-Object { $_ -split ',' } | Where-Object { $_ })
# Operator-pad stand-in for a contained run started with -OperatorInput: focuses the running
# shadPS4 window and sends real key input (the emulator ignores keys posted to an unfocused window).
Add-Type -AssemblyName System.Windows.Forms
Add-Type @'
using System; using System.Runtime.InteropServices;
namespace D1Pad { public static class W {
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
[DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
[DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
[DllImport("user32.dll")] public static extern uint MapVirtualKey(uint code, uint type);
[DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
} }
'@
$emu = Get-Process shadPS4 -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne [IntPtr]::Zero } | Select-Object -First 1
if (-not $emu) { throw 'No shadPS4 window' }
$h = $emu.MainWindowHandle
function Focus {
    if ([D1Pad.W]::GetForegroundWindow() -eq $h) { return $true }
    [D1Pad.W]::keybd_event(0x12, 0, 0, [UIntPtr]::Zero); [D1Pad.W]::keybd_event(0x12, 0, 2, [UIntPtr]::Zero)
    [void][D1Pad.W]::ShowWindow($h, 9); [void][D1Pad.W]::SetForegroundWindow($h)
    Start-Sleep -Milliseconds 250
    return ([D1Pad.W]::GetForegroundWindow() -eq $h)
}
foreach ($k in $Keys) {
    $hold = 120
    if ($k -match '^Hold:(\w+):(\d+)$') { $k = $Matches[1]; $hold = [int]$Matches[2] }
    $vk = [byte][uint32][Windows.Forms.Keys]::$k
    $scan = [byte][D1Pad.W]::MapVirtualKey($vk, 0)
    if (-not (Focus)) { "focus lost before $k"; continue }
    [D1Pad.W]::keybd_event($vk, $scan, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds $hold
    [D1Pad.W]::keybd_event($vk, $scan, 2, [UIntPtr]::Zero)
    "pressed $k ($hold ms)"
    # Time of the last pad input, for the idle guard (d1-idle-guard.sh) that prevents the 15-minute kick.
    try { Set-Content -LiteralPath (Join-Path $PSScriptRoot 'd1-pad-last.txt') -Value ([DateTimeOffset]::UtcNow.ToUnixTimeSeconds()) } catch {}
    Start-Sleep -Milliseconds $GapMilliseconds
}
if ($Shot) { & (Join-Path $PSScriptRoot 'd1-shot.ps1') -Out $Shot }
