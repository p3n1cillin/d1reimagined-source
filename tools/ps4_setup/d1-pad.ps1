param(
    # Keys to press in order, as System.Windows.Forms.Keys names (NumPad2 = cross, NumPad6 = circle,
    # NumPad8 = triangle, NumPad4 = square, Return = options, Up/Down/Left/Right = d-pad, W/A/S/D =
    # left stick, Q/U = L1/R1, E/O = L2/R2). "Hold:<key>:<ms>" holds a key; "Hold:A+S:900" holds
    # several together (a diagonal on the stick).
    [Parameter(Mandatory)][string[]]$Keys,
    [int]$GapMilliseconds = 700,
    # Save a frame of the window after the last key.
    [string]$Shot = '',
    # Post the keys to the emulator's window instead of focusing it and sending real input. The
    # desktop keeps its focus. It works with the window in the background (runs fg4b-fg5b drove the
    # Director this way), the way the contained boot's own title taps are sent.
    [switch]$Post,
    # Append one line per key to this file (a run's boot\input.log).
    [string]$Log = ''
)
$ErrorActionPreference = 'Stop'
# Through `powershell -File` an array arrives as ONE comma-joined string ("Hold:S:600,NumPad2"), and
# such a key name matched nothing, so every multi-key call silently pressed nothing: split it here.
$Keys = @($Keys | ForEach-Object { $_ -split ',' } | Where-Object { $_ })
# Operator-pad stand-in for a contained run started with -OperatorInput: by default it focuses the
# running shadPS4 window and sends real key input; -Post posts the key messages to it instead.
#
# Driving the Director in orbit (notes/ps4-56578-full-game-test-2026-11-06.md): the cursor starts in
# the middle; "Hold:A+S:900", "Hold:W+D:220" puts it on SET DESTINATION, and NumPad2 opens the
# solar-system map. Check a frame before every press: an orbit popup such as the Destiny 2 pre-order
# has a store button under X (NumPad6, circle, dismisses it).
Add-Type -AssemblyName System.Windows.Forms
Add-Type @'
using System; using System.Runtime.InteropServices;
namespace D1Pad { public static class W {
[DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
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
function Key([string]$name, [bool]$down) {
    $vk = [byte][uint32][Windows.Forms.Keys]::$name
    $scan = [byte][D1Pad.W]::MapVirtualKey($vk, 0)
    # The arrow cluster uses an E0 prefix. Without it, Down/Up/Left/Right become the
    # numpad keys with the same scan code (2/8/4/6) in SDL instead of the d-pad arrows.
    $extended = $vk -ge 0x25 -and $vk -le 0x28
    if ($Post) {
        $l = 1 -bor ([int]$scan -shl 16) -bor $(if ($extended) { 1 -shl 24 } else { 0 })
        if (-not $down) { $l = $l -bor 0xC0000000 }
        [void][D1Pad.W]::PostMessage($h, $(if ($down) { 0x100 } else { 0x101 }), [IntPtr][int]$vk, [IntPtr][int64]$l)
    } else {
        $keyFlags = [uint32]$(if ($extended) { 1 } else { 0 }) -bor $(if ($down) { 0 } else { 2 })
        [D1Pad.W]::keybd_event($vk, $scan, $keyFlags, [UIntPtr]::Zero)
    }
}
foreach ($k in $Keys) {
    $hold = 120
    if ($k -match '^Hold:([\w+]+):(\d+)$') { $k = $Matches[1]; $hold = [int]$Matches[2] }
    $chord = @($k -split '\+' | Where-Object { $_ })
    if (-not $Post -and -not (Focus)) { "focus lost before $k"; continue }
    foreach ($c in $chord) { Key $c $true }
    Start-Sleep -Milliseconds $hold
    foreach ($c in $chord) { Key $c $false }
    "pressed $k ($hold ms)"
    if ($Log) { Add-Content -LiteralPath $Log "$([DateTime]::UtcNow.ToString('o')) pad $k ($hold ms$(if ($Post) { ', posted' }); d1-pad.ps1)" }
    # Time of the last pad input, for the idle guard (d1-idle-guard.sh) that prevents the 15-minute kick.
    try { Set-Content -LiteralPath (Join-Path $PSScriptRoot 'd1-pad-last.txt') -Value ([DateTimeOffset]::UtcNow.ToUnixTimeSeconds()) } catch {}
    Start-Sleep -Milliseconds $GapMilliseconds
}
if ($Shot) { & (Join-Path $PSScriptRoot 'd1-shot.ps1') -Out $Shot }
