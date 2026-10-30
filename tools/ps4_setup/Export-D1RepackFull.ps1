param(
    [string]$Package = (& "$PSScriptRoot\D1Paths.ps1" PkgDir 'Destiny_The_Taken_King_Legendary_Edition_PS4.pkg'),
    [string]$Library = (& "$PSScriptRoot\D1Paths.ps1" LibOrbisPkg 'LibOrbisPkg.ReadFully.dll'),
    [string]$Output = (& "$PSScriptRoot\D1Paths.ps1" Workspace 'dumps\later-56578-CUSA00219'),
    [switch]$Resume
)
$ErrorActionPreference = 'Stop'
# Full private extraction of the owned repack's game directory (/uroot) through the
# existing LGPL reader. The original package is opened read-only. Every file is
# written as .partial, length-checked, hashed, renamed and appended to a ledger;
# the final SHA256SUMS and manifest.json are written only after every file is done.
Add-Type -Path $Library
if ($null -eq [LibOrbisPkg.PFS.PFSCReader].GetField('ShortReadCount')) {
    throw 'Extraction requires the read-until-filled PFSC reader build'
}
$taskOriginal = Get-Item -LiteralPath $Package
$taskPlan = [ordered]@{
    source = $taskOriginal.FullName; bytes = $taskOriginal.Length
    lastWriteUtcTicks = $taskOriginal.LastWriteTimeUtc.Ticks
    readerSha256 = (Get-FileHash -LiteralPath $Library).Hash
    scope = '/uroot (all regular files)'
}
$taskLedger = Join-Path $Output 'files.jsonl'
$taskPlanPath = Join-Path $Output 'plan.json'
$taskRoot = Join-Path $Output 'CUSA00219'
if (Test-Path -LiteralPath $Output) {
    if (-not $Resume -or -not (Test-Path -LiteralPath $taskPlanPath)) { throw 'Existing output requires a matching extraction plan and -Resume' }
    $taskOldPlan = Get-Content -LiteralPath $taskPlanPath -Raw | ConvertFrom-Json
    foreach ($taskKeyName in $taskPlan.Keys) {
        if ([string]$taskOldPlan.$taskKeyName -ne [string]$taskPlan[$taskKeyName]) { throw "Extraction plan differs: $taskKeyName" }
    }
    if (Test-Path -LiteralPath (Join-Path $Output 'SHA256SUMS')) { throw 'Extraction already complete' }
} else {
    New-Item -ItemType Directory -Path $taskRoot -Force | Out-Null
    $taskPlan | ConvertTo-Json | Set-Content -LiteralPath $taskPlanPath -Encoding utf8
}
$taskRecorded = @{}
if (Test-Path -LiteralPath $taskLedger) {
    foreach ($taskLine in Get-Content -LiteralPath $taskLedger) {
        $taskRecord = $taskLine | ConvertFrom-Json
        if ($taskRecorded.ContainsKey($taskRecord.path)) { throw 'Duplicate extraction ledger record' }
        $taskRecorded[$taskRecord.path] = $taskRecord
    }
}
$taskMap = [IO.MemoryMappedFiles.MemoryMappedFile]::CreateFromFile($Package,
    [IO.FileMode]::Open, ('D1FullExtract-' + [guid]::NewGuid()), 0,
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
        $taskFiles = @($taskInner.GetAllFiles() | Where-Object { $_.FullName -like '/uroot/*' } |
            Sort-Object FullName)
        if ($taskFiles.Count -eq 0) { throw 'No game files found' }
        foreach ($taskFile in $taskFiles) {
            $taskRel = $taskFile.FullName.Substring(7)
            if ($taskRel -notmatch '^[A-Za-z0-9_.\-]+(/[A-Za-z0-9_.\-]+)*$' -or $taskRel -match '(^|/)\.\.?(/|$)') {
                throw "Unsafe path: $($taskFile.FullName)"
            }
        }
        $taskTotalBytes = [long]0; foreach ($taskFile in $taskFiles) { $taskTotalBytes += $taskFile.size }
        $taskDone = 0; $taskBytesDone = [long]0
        foreach ($taskFile in $taskFiles) {
            $taskRel = $taskFile.FullName.Substring(7)
            $taskFinal = Join-Path $taskRoot ($taskRel -replace '/', '\')
            if ($taskRecorded.ContainsKey($taskRel)) {
                if (-not (Test-Path -LiteralPath $taskFinal) -or
                    (Get-Item -LiteralPath $taskFinal).Length -ne $taskFile.size) {
                    throw "Existing extraction failed verification: $taskRel"
                }
            } else {
                if (Test-Path -LiteralPath $taskFinal) { throw "Unrecorded output exists: $taskRel" }
                $taskPartial = $taskFinal + '.partial'
                if (Test-Path -LiteralPath $taskPartial) { Remove-Item -LiteralPath $taskPartial }
                New-Item -ItemType Directory -Path (Split-Path $taskFinal) -Force | Out-Null
                $taskFile.Save($taskPartial)
                if ((Get-Item -LiteralPath $taskPartial).Length -ne $taskFile.size) { throw "Extracted length mismatch: $taskRel" }
                $taskHash = (Get-FileHash -LiteralPath $taskPartial).Hash.ToLowerInvariant()
                [IO.File]::Move($taskPartial, $taskFinal)
                $taskRecord = [ordered]@{ path = $taskRel; bytes = $taskFile.size; sha256 = $taskHash }
                $taskRecord | ConvertTo-Json -Compress | Add-Content -LiteralPath $taskLedger -Encoding utf8
                $taskRecorded[$taskRel] = [pscustomobject]$taskRecord
            }
            $taskDone++; $taskBytesDone += $taskFile.size
            if ($taskDone % 25 -eq 0 -or $taskDone -eq $taskFiles.Count) {
                [ordered]@{ atUtc = [DateTime]::UtcNow.ToString('o'); files = $taskDone
                    total = $taskFiles.Count; bytes = $taskBytesDone; totalBytes = $taskTotalBytes
                    shortSectorReadsCompleted = [LibOrbisPkg.PFS.PFSCReader]::ShortReadCount } |
                    ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Output 'progress.json') -Encoding utf8
            }
        }
        $taskRows = foreach ($taskFile in $taskFiles) { $taskRecorded[$taskFile.FullName.Substring(7)] }
        ($taskRows | ForEach-Object { "$($_.sha256)  $($_.path)" }) -join "`n" |
            Set-Content -LiteralPath (Join-Path $Output 'SHA256SUMS') -Encoding ascii -NoNewline
        [ordered]@{
            completedUtc = [DateTime]::UtcNow.ToString('o'); source = $taskOriginal.FullName
            files = $taskFiles.Count; bytes = $taskTotalBytes
            packages = @($taskRows | Where-Object { $_.path -like 'packages/*.pkg' }).Count
            eboot = $taskRecorded['eboot.bin']; imageKeyDigestVerified = $true
            keyMaterialRecorded = $false; originalOpenedReadOnly = $true
        } | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $Output 'manifest.json') -Encoding utf8
        "Extracted $($taskFiles.Count) files, $taskTotalBytes bytes."
    } finally { $taskView.Dispose(); $taskKey = $null }
} finally { $taskMap.Dispose() }
