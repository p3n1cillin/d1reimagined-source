[CmdletBinding()]
param(
    [string]$PartsDirectory = (& "$PSScriptRoot\D1Paths.ps1" Root 'PS4 Destiny'),
    [string]$OutputDirectory = (& "$PSScriptRoot\D1Paths.ps1" Staging '2026-10-11')
)
$ErrorActionPreference = 'Stop'
$stem = 'UP0002-CUSA00219_00-DESTINYTHEGAME01-A0129-V0100'
$parts = @(0..7 | ForEach-Object {
    Get-Item -LiteralPath (Join-Path $PartsDirectory ($stem + '_' + $_ + '.pkg'))
})
for ($i = 0; $i -lt 8; $i++) {
    $expected = if ($i -lt 7) { 4294967296L } else { 1402798080L }
    if ($parts[$i].Length -ne $expected) { throw "Unexpected size for part $i" }
}
$expectedSize = 31467569152L
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$final = Join-Path $OutputDirectory ($stem + '.pkg')
$partial = $final + '.partial'
$receipt = $final + '.receipt.json'
if ((Test-Path -LiteralPath $final) -or (Test-Path -LiteralPath $partial)) {
    throw 'Output already exists. Inspect it before retrying; originals are never overwritten.'
}
$start = [DateTime]::UtcNow
$buffer = New-Object byte[] (8MB)
$sourceRecords = @()
$wholeHash = [System.Security.Cryptography.IncrementalHash]::CreateHash([System.Security.Cryptography.HashAlgorithmName]::SHA256)
$output = [System.IO.File]::Open($partial, [System.IO.FileMode]::CreateNew, [System.IO.FileAccess]::Write, [System.IO.FileShare]::None)
try {
    foreach ($part in $parts) {
        $source = [System.IO.File]::Open($part.FullName, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::Read)
        $partHash = [System.Security.Cryptography.IncrementalHash]::CreateHash([System.Security.Cryptography.HashAlgorithmName]::SHA256)
        try {
            while (($count = $source.Read($buffer, 0, $buffer.Length)) -gt 0) {
                $output.Write($buffer, 0, $count)
                $partHash.AppendData($buffer, 0, $count)
                $wholeHash.AppendData($buffer, 0, $count)
            }
            $sourceRecords += [ordered]@{
                path = $part.FullName
                bytes = $source.Length
                sha256 = [Convert]::ToHexString($partHash.GetHashAndReset()).ToLowerInvariant()
            }
            Write-Output ('Joined ' + $part.Name)
        } finally { $source.Dispose(); $partHash.Dispose() }
    }
    $output.Flush($true)
    if ($output.Length -ne $expectedSize) { throw 'Merged package size mismatch' }
    $streamHash = [Convert]::ToHexString($wholeHash.GetHashAndReset()).ToLowerInvariant()
} finally { $output.Dispose(); $wholeHash.Dispose() }
$check = [System.IO.File]::OpenRead($partial)
try {
    $magic = New-Object byte[] 4
    if ($check.Read($magic, 0, 4) -ne 4 -or [Convert]::ToHexString($magic) -ne '7F434E54') {
        throw 'Merged package is missing Sony CNT magic'
    }
} finally { $check.Dispose() }
$diskHash = (Get-FileHash -LiteralPath $partial -Algorithm SHA256).Hash.ToLowerInvariant()
if ($diskHash -ne $streamHash) { throw 'Merged package read-back hash mismatch' }
Move-Item -LiteralPath $partial -Destination $final
[ordered]@{
    startedUtc = $start.ToString('o')
    completedUtc = [DateTime]::UtcNow.ToString('o')
    sources = $sourceRecords
    output = [ordered]@{ path = $final; bytes = $expectedSize; sha256 = $diskHash }
    readBackVerified = $true
    buildVerified = $false
    note = 'Concatenation verified. This is an install package, not proof of a decrypted 01.29 dump.'
} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $receipt -Encoding utf8
Write-Output "Verified package: $final"
Write-Output "Receipt: $receipt"
