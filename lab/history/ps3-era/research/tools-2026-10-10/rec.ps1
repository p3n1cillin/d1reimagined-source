param([Parameter(Mandatory)][string]$Run)
# Toggle RPCS3's native recorder (F11) for an owned run and confirm it from the emulator log.
# RPCS3 registers gw_toggle_recording only while the game window is active. First try an
# activation message without stealing focus; if the log shows no shortcut, briefly bring the
# game window to the foreground, post F11, and restore the previous foreground window.
$ErrorActionPreference = 'Stop'
Set-Location F:\d1\d1xenia
. .\scripts\Final124RunHelpers.ps1
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class RecWin {
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint a, uint b, bool attach);
  [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
  [DllImport("user32.dll")] public static extern uint MapVirtualKey(uint code, uint type);
}
'@
$m = Read-D1ProcessManifest "F:\d1\<pc>-import\d1xenia\local\SIGNON_LOCAL\logs\FINAL124_$Run-processes.json"
$client = $m.processes | Where-Object tag -EQ 'rpcs3'
if (-not (Get-D1OwnedProcess $client)) { throw 'Owned emulator is not running.' }
$proc = Get-Process -Id $client.id
$h = $proc.MainWindowHandle
$game = Get-Process -Id $client.id | ForEach-Object { $_ }
# The game window carries the BLES01857 title; MainWindowHandle can be the main GUI.
Add-Type -AssemblyName System.Windows.Forms | Out-Null
$found = [System.Collections.Generic.List[IntPtr]]::new()
Add-Type @'
using System; using System.Runtime.InteropServices; using System.Text; using System.Collections.Generic;
public static class RecEnum {
  public delegate bool Cb(IntPtr h, IntPtr p);
  [DllImport("user32.dll")] public static extern bool EnumWindows(Cb cb, IntPtr p);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  public static List<IntPtr> Find(uint pid, string needle) {
    var r = new List<IntPtr>();
    EnumWindows((w, p) => { uint q; GetWindowThreadProcessId(w, out q); if (q == pid) { var sb = new StringBuilder(512); GetWindowText(w, sb, 512); if (sb.ToString().Contains(needle)) r.Add(w); } return true; }, IntPtr.Zero);
    return r;
  }
}
'@
$wins = [RecEnum]::Find([uint32]$client.id, 'BLES01857')
if (-not $wins.Count) { throw 'No RPCS3 game window.' }
$g = $wins[0]
$log = Join-Path (Split-Path $client.executable) 'log\RPCS3.log'
function Count-Shortcuts { $s=[IO.File]::Open($log,'Open','Read','ReadWrite,Delete'); try { $r=[IO.StreamReader]::new($s); ([regex]::Matches($r.ReadToEnd(),'Shortcut pressed: gw_toggle_recording')).Count } finally { $s.Close() } }
function Post-F11($w) {
  $scan = [RecWin]::MapVirtualKey(0x7A, 0)
  $down = 1 -bor ($scan -shl 16); $up = $down -bor (1 -shl 30) -bor (1 -shl 31)
  [void][RecWin]::PostMessage($w, 0x100, [IntPtr]0x7A, [IntPtr][int64]$down); Start-Sleep -Milliseconds 120
  [void][RecWin]::PostMessage($w, 0x101, [IntPtr]0x7A, [IntPtr][int64]([uint32]$up))
}
$state = Join-Path $PSScriptRoot "rec-$Run.state"
$recDir = Join-Path (Split-Path $client.executable) 'recordings\BLES01857'
if (Test-Path $state) {
  if ((Get-Content $state -Raw).Trim() -ne 'on') { Remove-Item $state; "left the existing recording running"; exit 0 }
  Remove-Item $state
} else {
  $active = @(Get-ChildItem $recDir -Filter *.mp4 -ErrorAction SilentlyContinue | Where-Object { $_.LastWriteTime -gt (Get-Date).AddSeconds(-45) -or ($_.Length -eq 0 -and $_.CreationTimeUtc -gt [DateTime]::new([long]$client.startTimeUtcTicks,[DateTimeKind]::Utc)) })
  if ($active.Count) { Set-Content $state 'skip'; "recording already active ($($active[-1].Name)); not toggled"; exit 0 }
  Set-Content $state 'on'
}
$before = Count-Shortcuts
# 1: activation message only (WM_ACTIVATE, WA_ACTIVE), no focus change.
[void][RecWin]::PostMessage($g, 0x0006, [IntPtr]1, [IntPtr]::Zero); Start-Sleep -Milliseconds 200
Post-F11 $g; Start-Sleep -Seconds 3
if ((Count-Shortcuts) -gt $before) { "recorder toggled (activation message)"; exit 0 }
# 2: brief foreground switch, then restore.
$prev = [RecWin]::GetForegroundWindow()
$pidOut = 0; $fgThread = [RecWin]::GetWindowThreadProcessId($prev, [ref]$pidOut); $me = [RecWin]::GetCurrentThreadId()
[void][RecWin]::AttachThreadInput($me, $fgThread, $true)
[void][RecWin]::BringWindowToTop($g); [void][RecWin]::SetForegroundWindow($g)
[void][RecWin]::AttachThreadInput($me, $fgThread, $false)
Start-Sleep -Milliseconds 400
Post-F11 $g; Start-Sleep -Seconds 2
if ($prev -ne [IntPtr]::Zero) { [void][RecWin]::SetForegroundWindow($prev) }
if ((Count-Shortcuts) -gt $before) { "recorder toggled (foreground)" } else { "recorder NOT toggled" }
