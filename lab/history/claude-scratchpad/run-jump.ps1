param([int]$RunMs = 400, [int]$SecondJumpMs = 250, [int]$AfterMs = 900, [switch]$Single)
# Hold W (left stick forward) through a jump and, unless -Single, a second jump in the air.
Add-Type @'
using System; using System.Runtime.InteropServices;
namespace D1Jump { public static class W {
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
[DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
[DllImport("user32.dll")] public static extern uint MapVirtualKey(uint code, uint type);
[DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
} }
'@
$emu = Get-Process shadPS4 | Where-Object { $_.MainWindowHandle -ne [IntPtr]::Zero } | Select-Object -First 1
$h = $emu.MainWindowHandle
[D1Jump.W]::keybd_event(0x12, 0, 0, [UIntPtr]::Zero); [D1Jump.W]::keybd_event(0x12, 0, 2, [UIntPtr]::Zero)
[void][D1Jump.W]::SetForegroundWindow($h); Start-Sleep -Milliseconds 250
if ([D1Jump.W]::GetForegroundWindow() -ne $h) { throw 'emulator not focused' }
function Down($vk) { [D1Jump.W]::keybd_event($vk, [byte][D1Jump.W]::MapVirtualKey($vk, 0), 0, [UIntPtr]::Zero) }
function Up($vk) { [D1Jump.W]::keybd_event($vk, [byte][D1Jump.W]::MapVirtualKey($vk, 0), 2, [UIntPtr]::Zero) }
function Tap($vk) { Down $vk; Start-Sleep -Milliseconds 90; Up $vk }
$W = 0x57; $J = 0x62   # W, NumPad2 (cross)
Down $W; Start-Sleep -Milliseconds $RunMs
Tap $J
if (-not $Single) { Start-Sleep -Milliseconds $SecondJumpMs; Tap $J }
Start-Sleep -Milliseconds $AfterMs; Up $W
Set-Content -Path 'F:\d1\tools\d1-pad-last.txt' -Value ([DateTimeOffset]::UtcNow.ToUnixTimeSeconds())
'jumped'
