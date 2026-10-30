param(
    [string]$Package = (& "$PSScriptRoot\D1Paths.ps1" PkgDir 'Destiny_The_Taken_King_Legendary_Edition_PS4.pkg'),
    [string]$Library = (& "$PSScriptRoot\D1Paths.ps1" LibOrbisPkg 'LibOrbisPkg.ReadFully.dll'),
    [string]$Audit = (& "$PSScriptRoot\D1Paths.ps1" Staging '2026-10-11\rollback-header-audit.json'),
    [string]$Output = (& "$PSScriptRoot\D1Paths.ps1" Staging '2026-10-11\recovered-01.29-content-v2'),
    [switch]$Resume,
    [int]$MaxPackages = 0
)
$ErrorActionPreference = 'Stop'
# Existing LGPL reader owns Sony/PFS decryption. Rust audit owns Bungie header
# interpretation. Full payload acceptance still requires d1-pkg controls/verify.
Add-Type -Path $Library
if ($null -eq [LibOrbisPkg.PFS.PFSCReader].GetField('ShortReadCount')) {
    throw 'Recovery requires the read-until-filled PFSC reader build'
}
$taskAudit = Get-Content -LiteralPath $Audit -Raw | ConvertFrom-Json
if (-not $taskAudit.reference_manifest_matches -or $taskAudit.eligible_headers -ne 1647 -or
    $taskAudit.independent_header_matches -lt 2 -or $taskAudit.controls_passed -ne 5) {
    throw 'Exact-reference header audit gate failed'
}
$taskNames = @($taskAudit.eligible)
if ($taskNames.Count -ne 1647 -or @($taskNames | Select-Object -Unique).Count -ne 1647) {
    throw 'Incomplete/duplicate recovery list'
}
foreach ($taskName in $taskNames) {
    if ($taskName -notmatch '^ps4_[a-zA-Z0-9_]+\.pkg$') { throw "Unsafe package name: $taskName" }
}
$taskOriginal = Get-Item -LiteralPath $Package
$taskPlan = [ordered]@{
    source = $taskOriginal.FullName; bytes = $taskOriginal.Length
    lastWriteUtcTicks = $taskOriginal.LastWriteTimeUtc.Ticks
    auditSha256 = (Get-FileHash -LiteralPath $Audit).Hash
    readerSha256 = (Get-FileHash -LiteralPath $Library).Hash
    packageCount = $taskNames.Count
}
$taskLedger = Join-Path $Output 'files.jsonl'
$taskPlanPath = Join-Path $Output 'plan.json'
if (Test-Path -LiteralPath $Output) {
    if (-not $Resume -or -not (Test-Path -LiteralPath $taskPlanPath)) { throw 'Existing output requires a matching owned recovery plan and -Resume' }
    $taskOldPlan = Get-Content -LiteralPath $taskPlanPath -Raw | ConvertFrom-Json
    foreach ($taskKeyName in $taskPlan.Keys) {
        if ([string]$taskOldPlan.$taskKeyName -ne [string]$taskPlan[$taskKeyName]) { throw "Recovery plan differs: $taskKeyName" }
    }
} else {
    New-Item -ItemType Directory -Path (Join-Path $Output 'packages') -Force | Out-Null
    $taskPlan | ConvertTo-Json | Set-Content -LiteralPath $taskPlanPath -Encoding utf8
}
$taskRecorded = @{}
if (Test-Path -LiteralPath $taskLedger) {
    foreach ($taskLine in Get-Content -LiteralPath $taskLedger) {
        $taskRecord = $taskLine | ConvertFrom-Json
        if (-not ($taskNames -contains $taskRecord.name)) { throw 'Ledger contains an unplanned package' }
        if ($taskRecorded.ContainsKey($taskRecord.name)) { throw 'Duplicate recovery ledger record' }
        $taskRecorded[$taskRecord.name] = $taskRecord
    }
}
$taskMap = [IO.MemoryMappedFiles.MemoryMappedFile]::CreateFromFile($Package,
    [IO.FileMode]::Open, ('D1ContentRecovery-' + [guid]::NewGuid()), 0,
    [IO.MemoryMappedFiles.MemoryMappedFileAccess]::Read)
try {
    $taskStream = $taskMap.CreateViewStream(0, 0, [IO.MemoryMappedFiles.MemoryMappedFileAccess]::Read)
    try { $taskPkg = [LibOrbisPkg.PKG.PkgReader]::new($taskStream).ReadPkg() }
    finally { $taskStream.Dispose() }
    $taskKey = $taskPkg.GetEkpfs()
    if ($null -eq $taskKey -or -not $taskPkg.CheckEkpfs($taskKey)) { throw 'Repack image-key verification failed' }
    $taskView = $taskMap.CreateViewAccessor([long]$taskPkg.Header.pfs_image_offset,
        [long]$taskPkg.Header.pfs_image_size, [IO.MemoryMappedFiles.MemoryMappedFileAccess]::Read)
    try {
        $taskOuter = [LibOrbisPkg.PFS.PfsReader]::new($taskView, $taskPkg.Header.pfs_flags, $taskKey)
        $taskInner = [LibOrbisPkg.PFS.PfsReader]::new(
            [LibOrbisPkg.PFS.PFSCReader]::new($taskOuter.GetFile('pfs_image.dat').GetView()))
        $taskDone = 0; $taskNew = 0; $taskBytesDone = [long]0
        foreach ($taskName in $taskNames) {
            $taskFile = $taskInner.GetFile('packages/' + $taskName)
            if ($null -eq $taskFile) { throw "Missing planned package: $taskName" }
            $taskFinal = Join-Path (Join-Path $Output 'packages') $taskName
            if ($taskRecorded.ContainsKey($taskName)) {
                $taskRecord = $taskRecorded[$taskName]
                if (-not (Test-Path -LiteralPath $taskFinal) -or
                    (Get-Item -LiteralPath $taskFinal).Length -ne $taskFile.size -or
                    (Get-FileHash -LiteralPath $taskFinal).Hash -ne $taskRecord.sha256) {
                    throw "Existing recovery failed verification: $taskName"
                }
            } else {
                if (Test-Path -LiteralPath $taskFinal) { throw "Unrecorded output exists: $taskName" }
                $taskPartial = $taskFinal + '.partial'
                if (Test-Path -LiteralPath $taskPartial) { throw "Unfinished output retained for inspection: $taskPartial" }
                $taskFile.Save($taskPartial)
                if ((Get-Item -LiteralPath $taskPartial).Length -ne $taskFile.size) { throw 'Extracted length mismatch' }
                $taskHash = (Get-FileHash -LiteralPath $taskPartial).Hash
                [IO.File]::Move($taskPartial, $taskFinal)
                [ordered]@{ name = $taskName; bytes = $taskFile.size; sha256 = $taskHash } |
                    ConvertTo-Json -Compress | Add-Content -LiteralPath $taskLedger -Encoding utf8
                $taskNew++
            }
            $taskDone++; $taskBytesDone += $taskFile.size
            [ordered]@{ atUtc = [DateTime]::UtcNow.ToString('o'); packages = $taskDone
                total = $taskNames.Count; bytes = $taskBytesDone; complete = ($taskDone -eq $taskNames.Count)
                shortSectorReadsCompleted = [LibOrbisPkg.PFS.PFSCReader]::ShortReadCount
                payloadHashesValidatedByD1Pkg = $false; nativeExecutableRecovered = $false } |
                ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Output 'progress.json') -Encoding utf8
            if ($taskDone % 50 -eq 0 -or ($MaxPackages -gt 0 -and $taskNew -ge $MaxPackages)) {
                "Recovered $taskDone/$($taskNames.Count) packages, $taskBytesDone bytes."
            }
            if ($MaxPackages -gt 0 -and $taskNew -ge $MaxPackages) { break }
        }
        "Recovery checkpoint: $taskDone/$($taskNames.Count); d1-pkg payload verification still required."
    } finally { $taskView.Dispose(); $taskKey = $null }
} finally { $taskMap.Dispose() }
