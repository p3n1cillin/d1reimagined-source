# SPDX-License-Identifier: GPL-2.0-or-later
param(
    [string]$Package = (& "$PSScriptRoot\D1Paths.ps1" PkgDir 'Destiny_The_Taken_King_Legendary_Edition_PS4.pkg'),
    [string]$Library = (& "$PSScriptRoot\D1Paths.ps1" LibOrbisPkg 'LibOrbisPkg.ReadFully.dll'),
    [string]$Scanner = (& "$PSScriptRoot\D1Paths.ps1" Workspace 'd1-server\target\release\d1-byte-scan.exe'),
    [string]$Output = (& "$PSScriptRoot\D1Paths.ps1" Staging '2026-10-12\repack-slack-review')
)
$ErrorActionPreference = 'Stop'
# Platform adapter only. Existing LibOrbisPkg supplies PFS extents and decrypted
# logical reads; Rust owns complement planning, byte signatures and controls.
if (Test-Path -LiteralPath $Output) { throw 'Refusing existing slack evidence' }
$taskLibHash=(Get-FileHash -LiteralPath $Library).Hash.ToLowerInvariant()
if ($taskLibHash -ne 'c2d266a65c4f30fe3a2efe33b642311a240924b71f978fcfcc80bad46fd1149b') { throw 'Corrected reader identity differs' }
& $Scanner controls
if ($LASTEXITCODE -ne 0) { throw 'Byte scanner controls failed' }
Add-Type -Path $Library
$taskSource=[IO.File]::Open($Package,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
try {
    if($taskSource.Length -ne 52138213376) { throw 'Owned repack size differs' }
    $taskMap=[IO.MemoryMappedFiles.MemoryMappedFile]::CreateFromFile($taskSource,('D1Slack-'+[guid]::NewGuid()),0,
        [IO.MemoryMappedFiles.MemoryMappedFileAccess]::Read,[IO.HandleInheritability]::None,$true)
    try {
        $taskStream=$taskMap.CreateViewStream(0,0,[IO.MemoryMappedFiles.MemoryMappedFileAccess]::Read)
        try { $taskPkg=[LibOrbisPkg.PKG.PkgReader]::new($taskStream).ReadPkg() } finally { $taskStream.Dispose() }
        $taskKey=$taskPkg.GetEkpfs()
        if($null -eq $taskKey -or -not $taskPkg.CheckEkpfs($taskKey)) { throw 'Known repack image-key control failed' }
        $taskView=$taskMap.CreateViewAccessor([long]$taskPkg.Header.pfs_image_offset,[long]$taskPkg.Header.pfs_image_size,[IO.MemoryMappedFiles.MemoryMappedFileAccess]::Read)
        try {
            $taskOuter=[LibOrbisPkg.PFS.PfsReader]::new($taskView,$taskPkg.Header.pfs_flags,$taskKey)
            $taskRaw=[LibOrbisPkg.PFS.PFSCReader]::new($taskOuter.GetFile('pfs_image.dat').GetView())
            $taskInner=[LibOrbisPkg.PFS.PfsReader]::new($taskRaw)
            $taskHdr=[LibOrbisPkg.PFS.PFSCReader].GetField('hdr',[Reflection.BindingFlags]'Instance,NonPublic').GetValue($taskRaw)
            $taskFiles=@($taskInner.GetSuperRoot().GetAllFiles())
            if($taskFiles.Count -ne 2120 -or @($taskFiles|Where-Object {$null -ne $_.blocks -or $_.size -ne $_.compressed_size}).Count) {
                throw 'Expected contiguous, uncompressed file extent model differs; no scan attempted'
            }
            $taskEboot=@($taskFiles|Where-Object name -eq 'eboot.bin')
            if($taskEboot.Count -ne 1 -or $taskEboot[0].size -ne 29276248) { throw 'Known executable metadata control failed' }
            New-Item -ItemType Directory -Path $Output | Out-Null
            [ordered]@{
                atUtc=[DateTime]::UtcNow.ToString('o');scope='Contiguous inner-PFS payload extents from existing reader; original opened read-only'
                logicalBytes=[long]$taskHdr.DataLength;sourceBytes=$taskSource.Length;readerSha256=$taskLibHash
                control=@{start=[long]$taskEboot[0].offset;length=[long]$taskEboot[0].size;sha256='ebee5df27aca00057f223b96edab461917e1a7e837b4017b7d6d84b0ae7a2a02'}
                files=@($taskFiles|ForEach-Object {@{start=[long]$_.offset;length=[long]$_.size}})
                outerFiles=@($taskOuter.GetSuperRoot().GetAllFiles()|ForEach-Object {@{name=$_.FullName;bytes=$_.size}})
                keyMaterialRecorded=$false
            } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $Output 'layout.json') -Encoding utf8
            & $Scanner plan --layout (Join-Path $Output 'layout.json') --out (Join-Path $Output 'plan.json')
            if($LASTEXITCODE -ne 0) { throw 'Bounded byte plan refused; layout evidence retained' }
            $taskPlan=Get-Content -LiteralPath (Join-Path $Output 'plan.json') -Raw | ConvertFrom-Json
            Write-Output "Planned $($taskPlan.regions.Count - 1) non-payload regions, $($taskPlan.gap_bytes) bytes, plus the complete known executable control."
            $taskBuffer=New-Object byte[] 1048576
            $taskIndex=0
            foreach($taskRegion in $taskPlan.regions) {
                if($taskRegion.file -notmatch '^(control|gap-[0-9]{6})\.bin$') { throw 'Unsafe generated region filename' }
                $taskDest=Join-Path $Output $taskRegion.file
                $taskFile=[IO.File]::Open($taskDest,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
                try {
                    $taskPosition=[long]$taskRegion.start;$taskRemaining=[long]$taskRegion.length
                    while($taskRemaining -gt 0) {
                        $taskCount=[int][Math]::Min($taskRemaining,$taskBuffer.Length)
                        $taskRaw.Read($taskPosition,$taskBuffer,0,$taskCount)
                        $taskFile.Write($taskBuffer,0,$taskCount)
                        $taskPosition+=$taskCount;$taskRemaining-=$taskCount
                    }
                    $taskFile.Flush($true)
                } finally { $taskFile.Dispose() }
                if($taskRegion.role -eq 'known-executable-control' -and (Get-FileHash -LiteralPath $taskDest).Hash.ToLowerInvariant() -ne $taskRegion.sha256) {
                    throw 'Same-window raw logical executable SHA-256 control failed'
                }
                $taskIndex++
                if($taskIndex % 250 -eq 0) { Write-Output "Exported $taskIndex / $($taskPlan.regions.Count) bounded regions" }
            }
            & $Scanner scan --directory $Output --out (Join-Path $Output 'scan.json')
            if($LASTEXITCODE -ne 0) { throw 'Byte scan failed; raw private evidence retained' }
            $taskScan=Get-Content -LiteralPath (Join-Path $Output 'scan.json') -Raw | ConvertFrom-Json
            [ordered]@{atUtc=[DateTime]::UtcNow.ToString('o');scope=$taskScan.scope;scannerSha256=(Get-FileHash -LiteralPath $Scanner).Hash.ToLowerInvariant();readerSha256=$taskLibHash
                logicalBytes=$taskScan.logical_bytes;gapBytesScanned=$taskScan.gap_bytes_scanned;knownExecutableControlPassed=$taskScan.known_executable_control_passed
                nonPayloadHits=@($taskScan.regions|Where-Object role -eq 'outside-file-payloads'|ForEach-Object {foreach($hit in $_.hits) {@{region=$_.file;marker=$hit.marker;logicalOffset=([long]$_.start+[long]$hit.offset)}}})
                crossRegionHits=@($taskScan.cross_region_hits);executableRecovered=$false;nativeBootAccepted=$false;keyMaterialRecorded=$false
            } | ConvertTo-Json -Depth 7 | Set-Content -LiteralPath (Join-Path $Output 'summary.json') -Encoding utf8
            Get-Content -LiteralPath (Join-Path $Output 'summary.json')
        } finally { $taskView.Dispose();$taskKey=$null }
    } finally { $taskMap.Dispose() }
} finally { $taskSource.Dispose() }
