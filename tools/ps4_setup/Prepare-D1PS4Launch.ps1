# SPDX-License-Identifier: GPL-2.0-or-later
[CmdletBinding()]
param(
    [ValidateSet('Plan','ServerCheck')][string]$Mode = 'Plan',
    [string]$Name = ('prepare-' + [DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss')),
    [string]$Repo = (& "$PSScriptRoot\D1Paths.ps1" Workspace),
    [string]$Distribution = 'Ubuntu-24.04',
    [string]$EmulatorDirectory = (& "$PSScriptRoot\D1Paths.ps1" ShadPS4)
)
# Only orchestrates the canonical Bash launcher. No game, emulator or input is started.
$ErrorActionPreference = 'Stop'
if ($Name -notmatch '^[A-Za-z0-9_-]{1,64}$') { throw 'Invalid preparation name' }
$taskRepo = (Resolve-Path -LiteralPath $Repo).Path
function WslPath([string]$Path) {
    $full = [System.IO.Path]::GetFullPath($Path)
    if ($full -notmatch '^([A-Za-z]):\\(.+)$') { throw 'A local drive path is required' }
    '/mnt/' + $Matches[1].ToLowerInvariant() + '/' + $Matches[2].Replace('\','/')
}
function WriteNewJson([string]$Path, $Value) {
    $bytes = [System.Text.UTF8Encoding]::new($false).GetBytes(($Value | ConvertTo-Json -Depth 12) + "`n")
    $file = [System.IO.File]::Open($Path, [System.IO.FileMode]::CreateNew, [System.IO.FileAccess]::Write, [System.IO.FileShare]::None)
    try { $file.Write($bytes); $file.Flush($true) } finally { $file.Dispose() }
}
$taskOut = Join-Path $taskRepo ('cfw-staging\launch-preparation\' + $Name + '-' + $Mode.ToLowerInvariant())
if (Test-Path -LiteralPath $taskOut) { throw "Refusing existing preparation output: $taskOut" }
$taskLauncher = WslPath (Join-Path $taskRepo 'capture-rig\run-boot.sh')
$taskMode = if ($Mode -eq 'Plan') { '--plan' } else { '--server-check' }
& wsl.exe -d $Distribution -u d1 --exec env ('RUN_OUT=' + (WslPath $taskOut)) bash $taskLauncher $taskMode $Name
$taskCode = $LASTEXITCODE
if ($taskCode -ne 0) { throw "Canonical $Mode failed with exit $taskCode; evidence retained at $taskOut" }
if ($Mode -eq 'Plan') {
    $taskContract = Get-Content -LiteralPath (Join-Path $taskOut 'launch-contract.json') -Raw | ConvertFrom-Json
    $taskReceipt = [ordered]@{
        atUtc = [DateTime]::UtcNow.ToString('o')
        scope = 'Windows preparation from the canonical Linux contract; no emulator started'
        canonicalContract = (Join-Path $taskOut 'launch-contract.json')
        canonicalLauncherSha256 = $taskContract.canonical_launcher_sha256
        canonicalDelegateSha256 = $taskContract.canonical_delegate_sha256
        proposedWindowsEmulator = (Join-Path $EmulatorDirectory 'shadPS4.exe')
        proposedWindowsGame = (Join-Path $taskRepo 'dumps\patch-01.29-CUSA00219\eboot.bin')
        canonicalEnvironment = $taskContract.environment
        linuxOnlySettingsRequiringPort = @('VK_DRIVER_FILES','SDL_IGNORE_PADS','PAD_FIFO','D1_ACCOUNT_TRACE_OUT','D1_PROFILE_FILE','D1_CHARACTER_FILE','D1_ROSTER_DIR')
        exactExecutablePresent = $taskContract.exact_executable_present
        pinnedManifestPresent = $taskContract.pinned_manifest_present
        missingConfiguredFixtures = @($taskContract.missing_configured_fixtures)
        windowsTransportValidated = $false
        nativeBootAccepted = $false
    }
    WriteNewJson (Join-Path $taskOut 'windows-preparation.json') $taskReceipt
} else {
    $taskChecks = Get-Content -LiteralPath (Join-Path $taskOut 'server-checks.json') -Raw | ConvertFrom-Json
    if (-not $taskChecks.passed -or $taskChecks.native_boot_accepted -or $taskChecks.windows_transport_validated) {
        throw 'Server-only receipt failed or claims a native acceptance outside its scope'
    }
    $taskCleanup = @($taskChecks.owned_pids | ForEach-Object {
        & wsl.exe -d $Distribution -u d1 --exec test '!' -e ('/proc/' + $_)
        [ordered]@{pid=$_;gone=($LASTEXITCODE -eq 0)}
    })
    WriteNewJson (Join-Path $taskOut 'server-child-cleanup.json') ([ordered]@{
        atUtc=[DateTime]::UtcNow.ToString('o'); scope='Canonical server-only child shutdown'; checks=$taskCleanup
        passed=(@($taskCleanup | Where-Object {-not $_.gone}).Count -eq 0)
    })
    if (@($taskCleanup | Where-Object {-not $_.gone}).Count -ne 0) { throw 'A server child survived; inspect the private receipt' }
}
Write-Output "Prepared $Mode evidence: $taskOut. Native Windows game boot remains unaccepted."
