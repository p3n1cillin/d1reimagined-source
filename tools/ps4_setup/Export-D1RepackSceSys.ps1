param(
    [string]$Package = (& "$PSScriptRoot\D1Paths.ps1" PkgDir 'Destiny_The_Taken_King_Legendary_Edition_PS4.pkg'),
    [string]$Library = (& "$PSScriptRoot\D1Paths.ps1" LibOrbisPkg 'LibOrbisPkg.ReadFully.dll'),
    [string]$Output = (& "$PSScriptRoot\D1Paths.ps1" Workspace 'dumps\later-56578-CUSA00219')
)
$ErrorActionPreference = 'Stop'
# The unencrypted package-level system entries (param.sfo, icons, changeinfo, ...) live in the PKG
# entry table, outside the PFS image that Export-D1RepackFull.ps1 extracts. Copy them, read-only
# through the existing LGPL reader's entry table, into <game>/sce_sys/. Encrypted entries (keys,
# licence) are never read. Refuses to overwrite; records SHA256SUMS.sce_sys beside the extraction.
Add-Type -Path $Library
$root = Join-Path $Output 'CUSA00219\sce_sys'
$sums = Join-Path $Output 'SHA256SUMS.sce_sys'
if (-not (Test-Path -LiteralPath (Join-Path $Output 'SHA256SUMS'))) { throw 'Complete the PFS extraction first' }
if (Test-Path -LiteralPath $sums) { throw 'System entries already exported' }
$stream = [IO.File]::OpenRead($Package)
try {
    $pkg = [LibOrbisPkg.PKG.PkgReader]::new($stream).ReadPkg()
    $rows = foreach ($meta in $pkg.Metas.Metas) {
        if ($meta.Encrypted) { continue }
        $name = $null
        if (-not [LibOrbisPkg.PKG.EntryNames]::IdToName.TryGetValue($meta.id, [ref]$name)) { continue }
        if ($name -notmatch '^[a-z0-9_]+(/[a-z0-9_]+)*\.(sfo|png|dds|xml|json|at9|trp|dat)$') { continue }
        if ($name -match '^(license|pubtoolinfo|psreserved)') { continue }
        $dest = Join-Path $root ($name -replace '/', '\')
        if (Test-Path -LiteralPath $dest) { throw "Refusing to overwrite $name" }
        $bytes = New-Object byte[] $meta.DataSize
        $stream.Position = [long]$meta.DataOffset
        $read = 0
        while ($read -lt $bytes.Length) {
            $n = $stream.Read($bytes, $read, $bytes.Length - $read)
            if ($n -le 0) { throw "Short read: $name" }
            $read += $n
        }
        New-Item -ItemType Directory -Path (Split-Path $dest) -Force | Out-Null
        [IO.File]::WriteAllBytes($dest, $bytes)
        $sha = [BitConverter]::ToString([Security.Cryptography.SHA256]::HashData($bytes)).Replace('-', '').ToLowerInvariant()
        "$sha  sce_sys/$name"
    }
} finally { $stream.Dispose() }
if (-not ($rows -match '  sce_sys/param\.sfo$')) { throw 'No param.sfo entry found' }
($rows -join "`n") | Set-Content -LiteralPath $sums -Encoding ascii -NoNewline
"Exported $(@($rows).Count) unencrypted system entries."
$rows
