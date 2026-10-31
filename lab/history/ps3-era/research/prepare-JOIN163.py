from pathlib import Path

rt = Path(r'F:\d1\<pc>-import\d1xenia\local')
source = rt / 'SIGNON_LOCAL/JOIN162-launch.ps1'
target = rt / 'SIGNON_LOCAL/JOIN163-launch.ps1'
assert not target.exists(), 'Unique fixture already exists'
text = source.read_text(encoding='utf-8-sig').replace('JOIN162', 'JOIN163')
text = text.replace('$completed=$false\ntry {', '''$completed=$false
$armingJob=Start-Job -ScriptBlock {
    param($rt, $rpc, $logs)
    $ErrorActionPreference='Stop'
    . 'F:\\d1\\d1xenia\\scripts\\Final124RunHelpers.ps1'
    $deadline=[DateTime]::UtcNow.AddMinutes(10)
    $manifestPath=Join-Path $logs 'FINAL124_JOIN163-processes.json'
    $bap=Join-Path $logs 'FINAL124_JOIN163-bap.log'
    while ([DateTime]::UtcNow -lt $deadline) {
        if ((Test-Path -LiteralPath $manifestPath) -and (Test-Path -LiteralPath $bap) -and
            (Select-String -LiteralPath $bap -Pattern 'sent mm rsp' -Quiet)) {
            $manifest=Read-D1ProcessManifest $manifestPath
            $client=$manifest.processes | Where-Object tag -EQ 'rpcs3'
            $p=Get-D1OwnedProcess $client
            if (-not $p) { throw 'Exact client missing before controlled arm' }
            $sha=(Get-FileHash -LiteralPath $client.executable -Algorithm SHA256).Hash.ToLowerInvariant()
            if ($sha -ne 'ef67aa5cf3c015de130cef958f1abfe345bd92a74964e2c20515e35e238f7645') { throw 'Unexpected late-arm build' }
            $layer=@($p.Modules | Where-Object ModuleName -EQ 'VkLayer_khronos_validation.dll' | Select-Object ModuleName,FileName)
            if ($layer.Count -ne 1) { throw 'Core validation not verified before arm' }
            $marker=Join-Path $rpc 'd1\\d1descriptorring'
            if (Test-Path -LiteralPath $marker) { throw 'Ring was already armed' }
            if (-not (Get-D1OwnedProcess $client)) { throw 'Exact client closed before arm' }
            $at=[DateTime]::UtcNow.ToString('o')
            New-Item -ItemType File -Path $marker -ErrorAction Stop | Out-Null
            [ordered]@{at=$at;armed_at=[DateTime]::UtcNow.ToString('o');client=$client;build=$sha;layer=$layer;
                trigger='Current unique run has sent mm rsp; ring OFF throughout sign-in/selection';
                descriptor_text_off=(-not (Test-Path -LiteralPath (Join-Path $rpc 'd1\\d1descriptor')));
                scope='Controlled late arm; native reset callback and combined visual acceptance remain unproved'} |
                ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $logs 'JOIN163-ring-arm.json') -Encoding utf8
            return
        }
        Start-Sleep -Seconds 1
    }
    throw 'No current matchmaking reply before late-arm deadline'
} -ArgumentList $rt,$rpc,$logs
try {''')
needle = '    $completed=$true\n'
assert text.count(needle) == 1
text = text.replace(needle, '''    Receive-Job $armingJob -Wait -ErrorAction Stop | Out-Null
    if ($armingJob.State -ne 'Completed' -or -not (Test-Path -LiteralPath (Join-Path $logs 'JOIN163-ring-arm.json'))) { throw 'Late arm did not complete' }
    $completed=$true
''')
text = text.replace('} finally {\n    $errors=', '''} finally {
    # End the armer before client/control cleanup so it cannot arm a closed run.
    if ($armingJob.State -eq 'Running') { Stop-Job $armingJob }
    Remove-Job $armingJob -Force
    $errors=''')
target.write_text(text, encoding='utf-8')
print(target)
