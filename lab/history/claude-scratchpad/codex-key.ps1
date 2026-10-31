# Focus the Codex app window and act on its composer: -Action enter | click-send | shot
param([string]$Action = 'shot', [string]$Shot = '')
$lock = Join-Path $PSScriptRoot 'codex.lock'
Set-Content -LiteralPath $lock -Value 'codex' ; Start-Sleep -Milliseconds 400
Add-Type @"
using System; using System.Runtime.InteropServices;
public class CXW { [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
 [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
 [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
 [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint x, uint y, uint d, UIntPtr e);
 [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
 [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint f); }
"@
Add-Type -AssemblyName System.Drawing, System.Windows.Forms
$h = (Get-Process ChatGPT | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1).MainWindowHandle
function Focus { for ($i = 0; $i -lt 10; $i++) { [CXW]::keybd_event(0x12, 0, 0, [UIntPtr]::Zero); [CXW]::ShowWindow($h, 3) | Out-Null; [CXW]::BringWindowToTop($h) | Out-Null; [CXW]::SetForegroundWindow($h) | Out-Null; [CXW]::keybd_event(0x12, 0, 2, [UIntPtr]::Zero); Start-Sleep -Milliseconds 400; if ([CXW]::GetForegroundWindow() -eq $h) { return $true }; Start-Sleep -Milliseconds 600 }; return $false }
if (-not (Focus)) { 'no focus'; exit 1 }
if ($Action -eq 'enter') {
    [CXW]::SetCursorPos(1100, 938) | Out-Null; [CXW]::mouse_event(2,0,0,0,[UIntPtr]::Zero); [CXW]::mouse_event(4,0,0,0,[UIntPtr]::Zero); Start-Sleep -Milliseconds 200
    [System.Windows.Forms.SendKeys]::SendWait('{ENTER}'); Start-Sleep -Seconds 2
} elseif ($Action -eq 'click-send') {
    [CXW]::SetCursorPos(1474, 990) | Out-Null; [CXW]::mouse_event(2,0,0,0,[UIntPtr]::Zero); [CXW]::mouse_event(4,0,0,0,[UIntPtr]::Zero); Start-Sleep -Seconds 3
}
if ($Shot) {
    $bmp = New-Object System.Drawing.Bitmap 1920, 1032; $g = [System.Drawing.Graphics]::FromImage($bmp); $dc = $g.GetHdc(); [CXW]::PrintWindow($h, $dc, 2) | Out-Null; $g.ReleaseHdc($dc); $g.Dispose()
    $bmp.Save($Shot); $bmp.Dispose()
}
"done $Action"
# the caller removes codex.lock when it has finished with the Codex window
