param(
    [string]$Package = (& "$PSScriptRoot\D1Paths.ps1" PkgDir 'Destiny_The_Taken_King_Legendary_Edition_PS4.pkg'),
    [string]$Library = (& "$PSScriptRoot\D1Paths.ps1" LibOrbisPkg 'LibOrbisPkg.ReadFully.dll'),
    [string]$Output = (& "$PSScriptRoot\D1Paths.ps1" Staging '2026-10-11\rollback-headers')
)
$ErrorActionPreference = 'Stop'
# Platform orchestration only: LibOrbisPkg owns the Sony/PFS parser. The existing
# d1-pkg Rust crate owns the Bungie header parser. These are header clips, not dumps.
if (Test-Path -LiteralPath $Output) { throw "Refusing existing output: $Output" }
Add-Type -Path $Library
$taskMap = [IO.MemoryMappedFiles.MemoryMappedFile]::CreateFromFile(
    $Package, [IO.FileMode]::Open, ('D1HeaderExport-' + [guid]::NewGuid()), 0,
    [IO.MemoryMappedFiles.MemoryMappedFileAccess]::Read)
try {
    $taskStream = $taskMap.CreateViewStream(0, 0, [IO.MemoryMappedFiles.MemoryMappedFileAccess]::Read)
    try { $taskPkg = [LibOrbisPkg.PKG.PkgReader]::new($taskStream).ReadPkg() }
    finally { $taskStream.Dispose() }
    $taskKey = $taskPkg.GetEkpfs()
    if ($null -eq $taskKey -or -not $taskPkg.CheckEkpfs($taskKey)) {
        throw 'Repack image-key verification failed; no export attempted'
    }
    $taskView = $taskMap.CreateViewAccessor([long]$taskPkg.Header.pfs_image_offset,
        [long]$taskPkg.Header.pfs_image_size, [IO.MemoryMappedFiles.MemoryMappedFileAccess]::Read)
    try {
        $taskOuter = [LibOrbisPkg.PFS.PfsReader]::new($taskView, $taskPkg.Header.pfs_flags, $taskKey)
        $taskInner = [LibOrbisPkg.PFS.PfsReader]::new(
            [LibOrbisPkg.PFS.PFSCReader]::new($taskOuter.GetFile('pfs_image.dat').GetView()))
        $taskFiles = @($taskInner.GetAllFiles() | Where-Object { $_.FullName -match '^/uroot/packages/[^/]+\.pkg$' })
        if ($taskFiles.Count -eq 0) { throw 'No installed Bungie packages found' }
        New-Item -ItemType Directory -Path $Output | Out-Null
        $taskRows = foreach ($taskFile in $taskFiles) {
            if ($taskFile.size -lt 0x140) { throw "Short package: $($taskFile.FullName)" }
            $taskName = [IO.Path]::GetFileName($taskFile.FullName)
            $taskClipName = $taskName + '.hdr'
            $taskClip = New-Object byte[] 0x140
            $taskFile.GetView().Read(0, $taskClip, 0, $taskClip.Length)
            $taskClipPath = Join-Path $Output $taskClipName
            [IO.File]::WriteAllBytes($taskClipPath, $taskClip)
            [ordered]@{ name = $taskName; header_file = $taskClipName; size = $taskFile.size }
        }
        [ordered]@{
            atUtc = [DateTime]::UtcNow.ToString('o'); source = $Package
            packageCount = $taskFiles.Count; headerBytesEach = 0x140
            imageKeyDigestVerified = $true; headerClipsOnly = $true
            completeGameDump = $false; keyMaterialRecorded = $false; packages = @($taskRows)
        } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $Output 'inventory.json') -Encoding utf8
        "Exported $($taskFiles.Count) header clips; no full package payloads copied."
    } finally { $taskView.Dispose(); $taskKey = $null }
} finally { $taskMap.Dispose() }
