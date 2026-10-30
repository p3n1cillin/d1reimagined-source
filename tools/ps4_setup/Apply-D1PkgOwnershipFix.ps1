param(
    [string]$Workspace = (& "$PSScriptRoot\D1Paths.ps1" Workspace 'd1-server'),
    [string]$Patch = (Join-Path $PSScriptRoot 'd1-pkg-local-owner.patch')
)
$ErrorActionPreference = 'Stop'
# Apply our GPL-2.0-or-later change to the separate supplied crate, not a
# replacement format reader. Exact source pins refuse unrelated edits.
$taskPins = @(
    @{ Path='crates/d1-pkg/src/lib.rs'; Before='e4b55b2a22c9f21825b23a1964b3f9ba54b780b2636dd636e14db9eee3a388e5'; After='50a9b7ea63e14a13553095abe78c3b66a107249647877e818e0d3670bc1d74f5' },
    @{ Path='crates/d1-pkg/src/bin/d1-pkg.rs'; Before='3cefeb9e28176196b739cb1af0272c37e89f99b7dbb7b724d72b69af887653d2'; After='5c7358277e30c8c0aa937a5194028d4fa0feed5cfe5a9bcee69fd51d55c4d5d7' }
)
$taskPatchHash = (Get-FileHash -LiteralPath $Patch -Algorithm SHA256).Hash.ToLowerInvariant()
if ($taskPatchHash -ne '29d96e11e0d527a2afcab4d20f7bfc986d777403e68477ac65267bdf72529a62') {
    throw 'Ownership patch identity differs'
}
$taskStates = @($taskPins | ForEach-Object {
    $taskHash = (Get-FileHash -LiteralPath (Join-Path $Workspace $_.Path) -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($taskHash -eq $_.Before) { 'before' }
    elseif ($taskHash -eq $_.After) { 'after' }
    else { throw "Unrecognised crate source: $($_.Path)" }
})
if (@($taskStates | Where-Object { $_ -eq 'after' }).Count -eq 2) {
    Write-Output 'Exact ownership fix already installed; no files changed.'
    return
}
if (@($taskStates | Where-Object { $_ -eq 'before' }).Count -ne 2) {
    throw 'Mixed source state; inspect before applying'
}
$taskPatchPath = (Resolve-Path -LiteralPath $Patch).Path
& git -c core.autocrlf=false -C $Workspace apply --check $taskPatchPath
if ($LASTEXITCODE -ne 0) { throw 'Ownership patch check failed' }
& git -c core.autocrlf=false -C $Workspace apply $taskPatchPath
if ($LASTEXITCODE -ne 0) { throw 'Ownership patch apply failed' }
foreach ($taskPin in $taskPins) {
    if ((Get-FileHash -LiteralPath (Join-Path $Workspace $taskPin.Path) -Algorithm SHA256).Hash.ToLowerInvariant() -ne $taskPin.After) {
        throw "Patched source identity differs: $($taskPin.Path)"
    }
}
Write-Output 'Ownership fix installed. Rebuild d1-pkg and run its tests and controls before use.'
