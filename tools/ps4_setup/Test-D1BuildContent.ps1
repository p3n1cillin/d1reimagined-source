param(
    [string]$Build = 'later-56578',
    [string]$Repo = (& "$PSScriptRoot\D1Paths.ps1" Workspace),
    [string]$Binary = (& "$PSScriptRoot\D1Paths.ps1" Workspace 'd1-server\target\release\d1-pkg.exe'),
    [Parameter(Mandatory)][string]$Output
)
$ErrorActionPreference = 'Stop'
# Content acceptance for a registered build's full extraction, through the existing d1-pkg reader:
# every package table/layout and every stored-block hash, then the served English manifest with
# the existing default policy. The 01.29 golden controls are reference-estate pins and are not run
# here. Records the generated manifest identity; does not install it or accept a boot.
$registry = Get-Content -LiteralPath (Join-Path $Repo 'capture-rig\builds.json') -Raw | ConvertFrom-Json
$entry = $registry.builds.$Build
if ($null -eq $entry) { throw "Unknown build: $Build" }
$extraction = Split-Path (Join-Path $Repo $entry.game_dir)
$sums = Join-Path $extraction 'SHA256SUMS'
if (-not (Test-Path -LiteralPath $sums)) { throw 'Extraction is not complete (no SHA256SUMS)' }
$packages = Join-Path $Repo $entry.packages
if (Test-Path -LiteralPath $Output) { throw 'Refusing to replace validation evidence' }
New-Item -ItemType Directory -Path $Output | Out-Null

# Re-hash every extracted file against the extraction manifest before trusting it.
$rows = @(Get-Content -LiteralPath $sums | ForEach-Object { $h, $p = $_ -split '  ', 2; [pscustomobject]@{ sha = $h; path = $p } })
$bad = @(foreach ($r in $rows) {
    $f = Join-Path (Join-Path $Repo $entry.game_dir) ($r.path -replace '/', '\')
    if (-not (Test-Path -LiteralPath $f) -or (Get-FileHash -LiteralPath $f).Hash.ToLowerInvariant() -ne $r.sha) { $r.path }
})
$bad | Set-Content -LiteralPath (Join-Path $Output 'rehash-failures.txt') -Encoding utf8
if ($bad.Count) { throw "$($bad.Count) extracted files fail their recorded SHA-256" }
$partials = @(Get-ChildItem -LiteralPath (Join-Path $Repo $entry.game_dir) -Recurse -File -Filter '*.partial')
if ($partials.Count) { throw 'Unfinished .partial files remain in the extraction' }

$env:D1_BUILD = $Build
& $Binary --dir $packages verify --blocks 1> (Join-Path $Output 'verify-blocks.log') 2> (Join-Path $Output 'verify-blocks.stderr.log')
$verifyExit = $LASTEXITCODE
& $Binary --dir $packages stats 1> (Join-Path $Output 'stats.log') 2>&1
$manifest = Join-Path $Output 'full_en.bin'
# --allow-overflow: the 1,325-descriptor cap was measured on the 01.29 client; the row count is
# recorded below so an over-cap body is visible rather than silently accepted or refused.
& $Binary --dir $packages manifest --out $manifest --allow-overflow 1> (Join-Path $Output 'manifest.log') 2> (Join-Path $Output 'manifest.stderr.log')
$manifestExit = $LASTEXITCODE
& $Binary --dir $packages manifest --out (Join-Path $Output 'unused.bin') --dry-run --policy newest 1> (Join-Path $Output 'newest-dry-run.log') 2>&1
$pkgFiles = @(Get-ChildItem -LiteralPath $packages -File -Filter '*.pkg')
[ordered]@{
    atUtc = [DateTime]::UtcNow.ToString('o'); build = $Build; scope = 'Bungie content only'
    extractedFiles = $rows.Count; rehashFailures = 0; packageFiles = $pkgFiles.Count
    packageBytes = ($pkgFiles | Measure-Object Length -Sum).Sum
    tableAndStoredBlockVerificationExit = $verifyExit
    verificationLogSha256 = (Get-FileHash -LiteralPath (Join-Path $Output 'verify-blocks.log')).Hash.ToLowerInvariant()
    manifestExit = $manifestExit
    manifestLine = (Get-Content -LiteralPath (Join-Path $Output 'manifest.log') -ErrorAction SilentlyContinue | Select-Object -Last 1)
    manifestOverReferenceCap = [bool]((Get-Content -LiteralPath (Join-Path $Output 'manifest.log') -Raw -ErrorAction SilentlyContinue) -match ': (\d+) packages' -and [int]$Matches[1] -gt 1325)
    manifestBytes = if ($manifestExit -eq 0) { (Get-Item -LiteralPath $manifest).Length } else { $null }
    manifestSha256 = if ($manifestExit -eq 0) { (Get-FileHash -LiteralPath $manifest).Hash.ToLowerInvariant() } else { $null }
    referenceGoldenControlsRun = $false; nativeBootAccepted = $false
    verifierSha256 = (Get-FileHash -LiteralPath $Binary).Hash.ToLowerInvariant()
} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Output 'acceptance.json') -Encoding utf8
Get-Content -LiteralPath (Join-Path $Output 'verify-blocks.log') -Tail 5
Get-Content -LiteralPath (Join-Path $Output 'manifest.log')
if ($verifyExit -ne 0) { throw "Package/payload verification failed: $verifyExit" }
if ($manifestExit -ne 0) { throw "Manifest generation failed: $manifestExit" }
"Content acceptance recorded: $Output"
