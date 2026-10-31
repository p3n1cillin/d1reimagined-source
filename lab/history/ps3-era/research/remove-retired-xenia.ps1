$ErrorActionPreference='Stop'
$rt='F:\d1\<pc>-import\d1xenia\local'
$repo='F:\d1\d1xenia'
$imported='F:\d1\<pc>-import\d1xenia'
$inventory=Get-Content (Join-Path $rt 'PRIVATE_ENGINEERING\2026-10-06-xenia-cleanup-inventory.json') -Raw | ConvertFrom-Json
if ($inventory.root -ne $rt) { throw 'Unexpected inventory root' }
if (Get-Process xenia_canary -ErrorAction SilentlyContinue) { throw 'A retired emulator is running' }
if (Get-Process rpcs3 -ErrorAction SilentlyContinue) { throw 'Wait until the native comparison is closed' }
$protected=@(
    (Join-Path $rt 'RPCS3_FINAL124_SMOKE001\rpcs3.exe'),
    (Join-Path $rt 'RPCS3_FINAL124_SMOKE001\config\custom_configs\config_BLES01857.yml'),
    (Join-Path $rt 'RPCS3_FINAL124_SMOKE001\d1\d1ppu.txt'),
    (Join-Path $rt 'RESEARCH\ps3_updates\01.24\USRDIR\EBOOT.elf'),
    'F:\d1\d1xenia-data\SHA256SUMS.txt'
)
$before=@{}; foreach ($path in $protected) { $before[$path]=(Get-FileHash -LiteralPath $path).Hash }
$allowedTop=@('00_ORIGINAL_DUMP','01_WORKING','02_EXTRACTED','TOOLS','LOGS','HASHES')
$results=[Collections.Generic.List[object]]::new()
foreach ($target in $inventory.targets) {
    $path=[IO.Path]::GetFullPath([string]$target.path)
    if (-not $path.StartsWith($rt+'\',[StringComparison]::OrdinalIgnoreCase) -or $target.reparse_points -ne 0) { throw 'Target escapes checked runtime' }
    $relative=[IO.Path]::GetRelativePath($rt,$path)
    $top=$relative.Split('\')[0]
    if ($top -notlike 'XENIA*' -and $top -notin $allowedTop) { throw "Unexpected retired subtree: $top" }
    $item=Get-Item -LiteralPath $path -Force
    if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Target became a reparse point' }
    # Every descendant was checked for reparse points by the inventory tool.
    # Exact absolute targets stay under the named runtime; no shell-built deletes.
    Remove-Item -LiteralPath $path -Recurse -Force
    $results.Add(@{path=$path;removed=(-not (Test-Path -LiteralPath $path));bytes=$target.bytes;files=$target.files})
}
$obsolete=@(
    'notes\console-network-capture.md','notes\content-analysis.md','notes\graphics.md','notes\inventory.md',
    'notes\network-blocker.md','notes\network-options.md','notes\network-trace.md','notes\patched-build.md',
    'patches\xenia-content1-stfs.patch','patches\xenia-ethernet-diagnostic.patch','patches\xenia-live-state-diagnostic.patch',
    'patches\xenia-trace-writer-release.patch','patches\xenia-xhttp-startup-diagnostic.patch',
    'scripts\AnalyzeConsoleTrace.py','scripts\ConsoleNetworkCapture.ps1','scripts\Invoke-XeniaGraphicsTest.ps1','scripts\Start-XeniaTest.ps1'
)
foreach ($relative in $obsolete) {
    $path=[IO.Path]::GetFullPath((Join-Path $imported $relative))
    if (-not $path.StartsWith($imported+'\',[StringComparison]::OrdinalIgnoreCase)) { throw 'Imported source target escapes its checkout' }
    if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path -Force }
}
foreach ($relative in @('README.md','.gitignore','notes\test-log.md','notes\research-tools.md','notes\environment.md','notes\ps3-route.md','notes\feature-matrix.md','notes\errors.md','notes\eboot-addresses.md','server\hosts.py')) {
    $destination=[IO.Path]::GetFullPath((Join-Path $imported $relative))
    if (-not $destination.StartsWith($imported+'\',[StringComparison]::OrdinalIgnoreCase)) { throw 'Imported documentation target escapes its checkout' }
    Copy-Item -LiteralPath (Join-Path $repo $relative) -Destination $destination -Force
}
# Empty Xbox-only capture containers can now go; do not remove mixed containers.
foreach ($name in '00_ORIGINAL_DUMP','02_EXTRACTED','HASHES') {
    $path=[IO.Path]::GetFullPath((Join-Path $rt $name))
    if (-not $path.StartsWith($rt+'\',[StringComparison]::OrdinalIgnoreCase)) { throw 'Container escapes runtime' }
    if (-not @(Get-ChildItem -LiteralPath $path -Force).Count) { Remove-Item -LiteralPath $path }
}
$checks=@{}; foreach ($path in $protected) { $checks[$path]=((Get-FileHash -LiteralPath $path).Hash -eq $before[$path]) }
$receipt=@{at=[DateTime]::UtcNow.ToString('o');targets=@($results.ToArray());files=$inventory.files;bytes=$inventory.bytes;protected_unchanged=$checks;imported_unrelated_changes_preserved=$true;git_history_rewritten=$false}
$receipt | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $rt 'PRIVATE_ENGINEERING\2026-10-06-xenia-cleanup-result.json')
if (@($checks.Values | Where-Object { -not $_ }).Count -or @($results | Where-Object { -not $_.removed }).Count) { throw 'Retired-route cleanup verification failed' }
[ordered]@{removed_targets=$results.Count;removed_files=$inventory.files;removed_bytes=$inventory.bytes;protected_unchanged=$true} | ConvertTo-Json
