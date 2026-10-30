param(
    [string]$Source = (& "$PSScriptRoot\D1Paths.ps1" LibOrbisPkg 'LibOrbisPkg-643477263b2644e0803e0f58b8726ea4e3f3b7d4'),
    [string]$Output = (& "$PSScriptRoot\D1Paths.ps1" LibOrbisPkg 'LibOrbisPkg.ReadFully.dll')
)
$ErrorActionPreference = 'Stop'
# Builds a separately retained LGPL-3.0 reader with one modern-.NET compatibility
# correction. Original upstream files are unchanged; modified source is retained
# beside the DLL. No cryptographic algorithm or keyset is changed.
$taskPfs = Join-Path $Source 'LibOrbisPkg\PFS\PFSCReader.cs'
if ((Get-FileHash -LiteralPath $taskPfs).Hash -ne
    '0504B720CC82D049E15BC28B7361778441FA2F3D242F57CBEF987B95F8C834EA') {
    throw 'Original upstream PFSC source hash differs'
}
$taskOriginal = [IO.File]::ReadAllText($taskPfs)
$taskNeedle = '          ds.Read(output, 0, hdr.BlockSz);'
if (($taskOriginal.Split($taskNeedle).Count - 1) -ne 1) { throw 'Upstream PFSC read statement differs' }
if (Test-Path -LiteralPath $Output) { throw 'Refusing to replace an existing reader build' }
$taskReplacement = @'
          int filled = 0;
          while (filled < hdr.BlockSz)
          {
            int remaining = hdr.BlockSz - filled;
            int read = ds.Read(output, filled, remaining);
            if (read == 0)
              throw new EndOfStreamException("Truncated decompressed PFSC sector");
            if (read < remaining)
              System.Threading.Interlocked.Increment(ref ShortReadCount);
            filled += read;
          }
'@
$taskModified = $taskOriginal.Replace($taskNeedle, $taskReplacement).Replace(
    '    public const int Magic = 0x43534650;',
    "    public const int Magic = 0x43534650;`n    public static long ShortReadCount;")
$taskModifiedPath = $Output + '.PFSCReader.cs'
[IO.File]::WriteAllText($taskModifiedPath, $taskModified)
$taskFiles = @(Get-ChildItem -LiteralPath (Join-Path $Source 'LibOrbisPkg') -Recurse -Filter '*.cs' |
    Where-Object { $_.Name -ne 'AssemblyInfo.cs' -and $_.FullName -ne $taskPfs } |
    ForEach-Object FullName) + @($taskModifiedPath)
Add-Type -Path $taskFiles -OutputAssembly $Output -CompilerOptions @(
    '/define:CORE', '/unsafe', '/optimize+', '/nowarn:SYSLIB0021,SYSLIB0041') -IgnoreWarnings
[ordered]@{
    atUtc = [DateTime]::UtcNow.ToString('o')
    upstream = 'https://github.com/maxton/LibOrbisPkg'
    sourceCommit = '643477263b2644e0803e0f58b8726ea4e3f3b7d4'
    license = 'LGPL-3.0'; originalSourceUnchanged = $true; builtSourceModified = $true
    change = 'PFSC DeflateStream reads until sector filled; refuses premature EOF; counts short reads'
    originalPfsSha256 = (Get-FileHash -LiteralPath $taskPfs).Hash
    modifiedPfsSource = $taskModifiedPath
    modifiedPfsSha256 = (Get-FileHash -LiteralPath $taskModifiedPath).Hash
    library = $Output; librarySha256 = (Get-FileHash -LiteralPath $Output).Hash
    compilationInputs = @($taskFiles | ForEach-Object {
        [ordered]@{ path = $_; sha256 = (Get-FileHash -LiteralPath $_).Hash }
    })
} | ConvertTo-Json | Set-Content -LiteralPath ($Output + '.receipt.json') -Encoding utf8
Write-Output "Built modified LGPL reader: $Output"
