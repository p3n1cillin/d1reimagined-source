# Safe Codex send: -Check focuses and captures the window (look at it before sending);
# -Text sends, but only when the window is focused. Any failure aborts before a click or keystroke.
# Always clears codex.lock on exit.
param([string]$Text = '', [switch]$Check)
$S = $PSScriptRoot
$shot = Join-Path $S 'codex-send-check.png'
try {
    $o = & "$S\codex-key.ps1" -Action shot -Shot $shot
    $h = (Get-Process ChatGPT | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1).MainWindowHandle
    # Judge focus by the foreground window itself ($LASTEXITCODE is $null when no native command ran).
    for ($i = 0; $i -lt 15 -and [CXW]::GetForegroundWindow() -ne $h; $i++) { Start-Sleep -Milliseconds 200 }
    if (($o -join ' ') -match 'no focus' -or [CXW]::GetForegroundWindow() -ne $h) { 'ABORT: could not focus Codex'; exit 1 }
    if ($Check) { "checked: $shot"; exit 0 }
    if (-not $Text) { 'ABORT: no text'; exit 1 }
    if ([CXW]::GetForegroundWindow() -ne $h) { 'ABORT: focus lost'; exit 1 }
    Set-Clipboard -Value $Text
    Add-Type -AssemblyName System.Windows.Forms
    [CXW]::SetCursorPos(830, 940) | Out-Null; [CXW]::mouse_event(2,0,0,0,[UIntPtr]::Zero); [CXW]::mouse_event(4,0,0,0,[UIntPtr]::Zero); Start-Sleep -Milliseconds 300
    if ([CXW]::GetForegroundWindow() -ne $h) { 'ABORT: focus lost before paste'; exit 1 }
    [System.Windows.Forms.SendKeys]::SendWait('^a'); Start-Sleep -Milliseconds 200   # replace any unsent draft
    [System.Windows.Forms.SendKeys]::SendWait('^v'); Start-Sleep -Milliseconds 600
    [CXW]::SetCursorPos(1474, 990) | Out-Null; [CXW]::mouse_event(2,0,0,0,[UIntPtr]::Zero); [CXW]::mouse_event(4,0,0,0,[UIntPtr]::Zero); Start-Sleep -Seconds 3
    'sent'
} finally {
    [System.IO.File]::Delete((Join-Path $S 'codex.lock'))
}
