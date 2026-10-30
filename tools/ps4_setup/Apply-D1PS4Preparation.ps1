# SPDX-License-Identifier: GPL-2.0-or-later
param(
    [string]$Workspace = (& "$PSScriptRoot\D1Paths.ps1" Workspace),
    [string]$Patch = (Join-Path $PSScriptRoot 'canonical-preparation.patch')
)
$ErrorActionPreference = 'Stop'
# Minimal changes to the supplied canonical launchers; refuse unrelated edits.
$taskPins = @(
    @{Path='capture-rig/run-boot.sh';Before='04bd05f0406b27c0a4fa32cce5c019eb18ffc4389ece367460c2f26542b59908';After='22536378f698e53671848fa4307e9b0bbfccf21510c383f5545eb5c3b8d1cac6'},
    @{Path='capture-rig/run-signon-experiment.sh';Before='6ba220308343c4508231a5604fecbbd8c275cc4173bc8e7edc48fea341f7b224';After='6a3b2c89b098d9bd00f7dae2e0a0e7f5557a9bd483b7fc92af0e733c2c190d2d'}
)
if ((Get-FileHash -LiteralPath $Patch).Hash.ToLowerInvariant() -ne '3f67edd29e940b740837a6d46bf5e5928122fcbe04bf72bb658384d5ce88bb50') {
    throw 'Canonical preparation patch identity differs'
}
$taskStates = @($taskPins | ForEach-Object {
    $hash = (Get-FileHash -LiteralPath (Join-Path $Workspace $_.Path)).Hash.ToLowerInvariant()
    if ($hash -eq $_.Before) { 'before' }
    elseif ($hash -eq $_.After) { 'after' }
    else { throw "Unrecognised canonical source: $($_.Path)" }
})
if (@($taskStates | Where-Object {$_ -eq 'after'}).Count -eq 2) {
    Write-Output 'Exact canonical preparation already installed; no files changed.'
    return
}
if (@($taskStates | Where-Object {$_ -eq 'before'}).Count -ne 2) { throw 'Mixed source state; inspect before applying' }
$taskPatchPath = (Resolve-Path -LiteralPath $Patch).Path
& git -c core.autocrlf=false -C $Workspace apply --check $taskPatchPath
if ($LASTEXITCODE -ne 0) { throw 'Canonical preparation patch check failed' }
& git -c core.autocrlf=false -C $Workspace apply $taskPatchPath
if ($LASTEXITCODE -ne 0) { throw 'Canonical preparation patch apply failed' }
foreach ($pin in $taskPins) {
    if ((Get-FileHash -LiteralPath (Join-Path $Workspace $pin.Path)).Hash.ToLowerInvariant() -ne $pin.After) {
        throw "Patched source identity differs: $($pin.Path)"
    }
}
Write-Output 'Canonical preparation installed. Build d1-platform in the separate crate and run its controls before use.'
