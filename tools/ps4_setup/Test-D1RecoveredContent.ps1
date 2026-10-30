param(
    [string]$Recovery = (& "$PSScriptRoot\D1Paths.ps1" Staging '2026-10-11\recovered-01.29-content-v2'),
    [string]$Binary = (& "$PSScriptRoot\D1Paths.ps1" Workspace 'd1-server\target\release\d1-pkg.exe'),
    [string]$Output = (& "$PSScriptRoot\D1Paths.ps1" Staging '2026-10-11\rollback-content-validation')
)
$ErrorActionPreference = 'Stop'
# Orchestrates existing d1-pkg controls/verification. No format or hash-table
# implementation is duplicated. Acceptance is content-only, never a native boot.
$taskProgress = Get-Content -LiteralPath (Join-Path $Recovery 'progress.json') -Raw | ConvertFrom-Json
if (-not $taskProgress.complete -or $taskProgress.packages -ne 1647 -or
    $taskProgress.bytes -ne 49917028352) { throw 'Full recovery completion gate failed' }
$taskDir = Join-Path $Recovery 'packages'
$taskFiles = @(Get-ChildItem -LiteralPath $taskDir -File)
if ($taskFiles.Count -ne 1647 -or @($taskFiles | Where-Object Extension -ne '.pkg').Count -ne 0) {
    throw 'Unfinished or unexpected recovery files'
}
if (Test-Path -LiteralPath $Output) { throw 'Refusing to replace validation evidence' }
New-Item -ItemType Directory -Path $Output | Out-Null
$taskManifest = Join-Path $Output 'full_en.bin'
$taskBefore = & $Binary --dir $taskDir controls --served $taskManifest --json 2> (Join-Path $Output 'controls-before.stderr.log')
$taskBeforeExit = $LASTEXITCODE
$taskBefore | Set-Content -LiteralPath (Join-Path $Output 'controls-before.json') -Encoding utf8
if ($taskBeforeExit -ne 0) { throw "Pinned package controls failed: $taskBeforeExit" }
Write-Output 'Pinned manifest controls passed; verifying all tables and stored blocks.'
& $Binary --dir $taskDir verify --blocks 1> (Join-Path $Output 'verify-blocks.log') 2> (Join-Path $Output 'verify-blocks.stderr.log')
$taskVerifyExit = $LASTEXITCODE
if ($taskVerifyExit -ne 0) { throw "Package/payload verification failed: $taskVerifyExit" }
& $Binary --dir $taskDir manifest --out $taskManifest 1> (Join-Path $Output 'manifest.log') 2> (Join-Path $Output 'manifest.stderr.log')
if ($LASTEXITCODE -ne 0) { throw 'Manifest generation failed' }
$taskHash = (Get-FileHash -LiteralPath $taskManifest -Algorithm SHA256).Hash.ToLowerInvariant()
if ((Get-Item -LiteralPath $taskManifest).Length -ne 52797 -or $taskHash -ne
    '0c5bba3c63e9b68021b536513639cfc8896ec736950323e4ab06c5d7ed0770be') {
    throw 'Generated full manifest identity differs'
}
$taskAfter = & $Binary --dir $taskDir controls --served $taskManifest --json 2> (Join-Path $Output 'controls-after.stderr.log')
$taskAfterExit = $LASTEXITCODE
$taskAfter | Set-Content -LiteralPath (Join-Path $Output 'controls-after.json') -Encoding utf8
if ($taskAfterExit -ne 0) { throw "Served-manifest controls failed: $taskAfterExit" }
$taskControls = ($taskAfter -join "`n") | ConvertFrom-Json
if (-not $taskControls.passed -or @($taskControls.controls | Where-Object skipped).Count -ne 0) {
    throw 'Final controls contain a failure or skip'
}
[ordered]@{
    atUtc = [DateTime]::UtcNow.ToString('o'); scope = 'Bungie content only'
    recovery = $Recovery; packageCount = 1647; bytes = 49917028352
    packageControlPasses = $taskControls.controls.Count; packageControlSkips = 0
    tableAndStoredBlockVerificationExit = $taskVerifyExit
    verificationLogSha256 = (Get-FileHash -LiteralPath (Join-Path $Output 'verify-blocks.log')).Hash
    fullManifest = $taskManifest; manifestBytes = 52797; manifestSha256 = $taskHash
    executableRecovered = $false; nativeBootAccepted = $false
    readerSha256 = (Get-Content -LiteralPath (Join-Path $Recovery 'plan.json') -Raw | ConvertFrom-Json).readerSha256
    verifierSha256 = (Get-FileHash -LiteralPath $Binary).Hash
} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Output 'acceptance.json') -Encoding utf8
Get-Content -LiteralPath (Join-Path $Output 'verify-blocks.log')
Write-Output "Content-only acceptance recorded: $Output"
