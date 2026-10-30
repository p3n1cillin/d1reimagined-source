param(
    [string]$Package = (& "$PSScriptRoot\D1Paths.ps1" PkgDir 'Destiny_The_Taken_King_Legendary_Edition_PS4.pkg'),
    [string]$Library = (& "$PSScriptRoot\D1Paths.ps1" LibOrbisPkg 'LibOrbisPkg.ReadFully.dll'),
    [string]$Output = (& "$PSScriptRoot\D1Paths.ps1" Staging '2026-10-11\rollback-repack-inode-review.json')
)
$ErrorActionPreference = 'Stop'
# Inspect the existing reader's complete super-root and parsed inode objects.
# No raw PFS parser, file carving or cryptographic implementation is added.
if (Test-Path -LiteralPath $Output) { throw 'Refusing to replace inode evidence' }
Add-Type -Path $Library
$taskMap = [IO.MemoryMappedFiles.MemoryMappedFile]::CreateFromFile($Package,
    [IO.FileMode]::Open, ('D1InodeReview-' + [guid]::NewGuid()), 0,
    [IO.MemoryMappedFiles.MemoryMappedFileAccess]::Read)
try {
    $taskStream = $taskMap.CreateViewStream(0, 0, [IO.MemoryMappedFiles.MemoryMappedFileAccess]::Read)
    try { $taskPkg = [LibOrbisPkg.PKG.PkgReader]::new($taskStream).ReadPkg() }
    finally { $taskStream.Dispose() }
    $taskKey = $taskPkg.GetEkpfs()
    if ($null -eq $taskKey -or -not $taskPkg.CheckEkpfs($taskKey)) { throw 'Repack image-key gate failed' }
    $taskView = $taskMap.CreateViewAccessor([long]$taskPkg.Header.pfs_image_offset,
        [long]$taskPkg.Header.pfs_image_size, [IO.MemoryMappedFiles.MemoryMappedFileAccess]::Read)
    try {
        $taskOuter = [LibOrbisPkg.PFS.PfsReader]::new($taskView, $taskPkg.Header.pfs_flags, $taskKey)
        $taskInner = [LibOrbisPkg.PFS.PfsReader]::new(
            [LibOrbisPkg.PFS.PFSCReader]::new($taskOuter.GetFile('pfs_image.dat').GetView()))
        $taskFiles = @($taskInner.GetSuperRoot().GetAllFiles())
        $taskInodes = [LibOrbisPkg.PFS.PfsReader].GetField('dinodes',
            [Reflection.BindingFlags]'Instance,NonPublic').GetValue($taskInner)
        $taskLinked = [Collections.Generic.HashSet[uint]]::new()
        foreach ($taskFile in $taskFiles) { [void]$taskLinked.Add($taskFile.ino) }
        $taskOrphans = @()
        $taskRegular = 0
        for ($taskIndex = 0; $taskIndex -lt $taskInodes.Count; $taskIndex++) {
            $taskInode = $taskInodes[$taskIndex]
            if ($taskInode.Mode.HasFlag([LibOrbisPkg.PFS.InodeMode]::file)) {
                $taskRegular++
                if (-not $taskLinked.Contains([uint]$taskIndex)) {
                    $taskOrphans += [ordered]@{inode=$taskIndex;bytes=$taskInode.Size;links=$taskInode.Nlink}
                }
            }
        }
        $taskEboots = @($taskFiles | Where-Object name -eq 'eboot.bin')
        if ($taskEboots.Count -ne 1 -or $taskEboots[0].size -ne 29276248 -or $taskFiles.Count -lt 2119) {
            throw 'Known repack filesystem positive control failed'
        }
        [ordered]@{
            atUtc = [DateTime]::UtcNow.ToString('o'); scope = 'Parsed filesystem metadata, not raw unallocated-sector carving'
            readerSha256 = (Get-FileHash -LiteralPath $Library).Hash
            inodeCount = $taskInodes.Count; regularInodes = $taskRegular
            superRootFiles = $taskFiles.Count; userRootFiles = @($taskInner.GetAllFiles()).Count
            unlinkedRegularInodes = $taskOrphans
            executablePaths = @($taskEboots | ForEach-Object FullName)
            filesOutsideUserRoot = @($taskFiles | Where-Object { -not $_.FullName.StartsWith('/uroot/') } |
                ForEach-Object { [ordered]@{name=$_.FullName;bytes=$_.size} })
            knownExecutablePositiveControl = $true; keyMaterialRecorded = $false
        } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $Output -Encoding utf8
        Get-Content -LiteralPath $Output
    } finally { $taskView.Dispose(); $taskKey = $null }
} finally { $taskMap.Dispose() }
