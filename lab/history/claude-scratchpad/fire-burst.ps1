param([int]$Ms = 900)
# Aim down sights (L2 = E) and hold fire (R2 = O) together for $Ms, then release both.
Add-Type @'
using System; using System.Runtime.InteropServices;
namespace D1Fire { public static class W {
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
[DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
[DllImport("user32.dll")] public static extern uint MapVirtualKey(uint code, uint type);
[DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
} }
'@
$emu = Get-Process shadPS4 | Where-Object { $_.MainWindowHandle -ne [IntPtr]::Zero } | Select-Object -First 1
$h = $emu.MainWindowHandle
[D1Fire.W]::keybd_event(0x12, 0, 0, [UIntPtr]::Zero); [D1Fire.W]::keybd_event(0x12, 0, 2, [UIntPtr]::Zero)
[void][D1Fire.W]::SetForegroundWindow($h); Start-Sleep -Milliseconds 200
if ([D1Fire.W]::GetForegroundWindow() -ne $h) { throw 'emulator not focused' }
function Down($vk) { [D1Fire.W]::keybd_event($vk, [byte][D1Fire.W]::MapVirtualKey($vk, 0), 0, [UIntPtr]::Zero) }
function Up($vk) { [D1Fire.W]::keybd_event($vk, [byte][D1Fire.W]::MapVirtualKey($vk, 0), 2, [UIntPtr]::Zero) }
$E = 0x45; $O = 0x4F
Down $E; Start-Sleep -Milliseconds 250
Down $O; Start-Sleep -Milliseconds $Ms; Up $O
Start-Sleep -Milliseconds 100; Up $E
Set-Content -Path 'F:\d1\tools\d1-pad-last.txt' -Value ([DateTimeOffset]::UtcNow.ToUnixTimeSeconds())
