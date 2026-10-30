param([int]$ActivityId = 407, [int]$SettleSeconds = 25, [string]$After = '')
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\D1Paths.ps1"
# From setup:orbit in a running -OperatorInput session: the three SYNTHETIC steps again (launch
# poke, task-7 suppress, rung-34 done-mask force), then wait for activity:in_world and run $After.
$L = Get-D1Path ShadPS4 'user\log\shad_log.txt'
$g = Join-Path $PSScriptRoot 'd1-winguest.py'
function Count([string]$p) { @(Select-String -LiteralPath $L -Pattern $p).Count }
function WaitMore([string]$p, [int]$before, [int]$seconds) {
    $end = (Get-Date).AddSeconds($seconds)
    while ((Get-Date) -lt $end) { if ((Count $p) -gt $before) { return $true }; Start-Sleep 2 }
    return $false
}
$s34 = "Entering state 'activity:initial_slice_set_loading'"; $s36 = "Entering state 'activity:in_world'"
$n34 = Count $s34; $n36 = Count $s36
python $g launch --activity-id $ActivityId --confirm | Select-Object -Last 1
if (-not (WaitMore $s34 $n34 480)) { 'rung 34 not reached'; exit 2 }
Start-Sleep 10; python $g suppress --confirm | Select-Object -Last 1
Start-Sleep 30; python $g masks --force-done --confirm | Select-Object -Last 1
if (-not (WaitMore $s36 $n36 240)) { 'in_world not reached'; exit 2 }
"in_world; settling $SettleSeconds s"; Start-Sleep $SettleSeconds
if ($After) { Invoke-Expression $After }
