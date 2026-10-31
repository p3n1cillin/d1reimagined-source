param([Parameter(Mandatory)][string]$Run)
$ErrorActionPreference='Stop'
. F:\d1\d1xenia\scripts\Final124RunHelpers.ps1
$rt='F:\d1\<pc>-import\d1xenia\local'
$rpc=Join-Path $rt 'RPCS3_FINAL124_SMOKE001'
$logs=Join-Path $rt 'SIGNON_LOCAL\logs'
$research=Join-Path $rt 'RESEARCH'
$ids=@((Read-D1ProcessManifest (Join-Path $logs "FINAL124_$Run-processes.json")).processes)
foreach($path in @((Join-Path $logs "FINAL124_$Run-proxy-process.json"),
    (Join-Path $logs "$Run-session-probe-process.json"),
    (Join-Path $research "$Run-performance-probe-process.json"),
    (Join-Path $research "$Run-recorder-owned-process.json"))) {
    if(Test-Path -LiteralPath $path) {
        $value=Get-Content -LiteralPath $path -Raw | ConvertFrom-Json
        $ids+= $(if($value.recorder) {$value.recorder} else {$value})
    }
}
$checks=@($ids | ForEach-Object {
    if(-not $_.id -or -not $_.startTimeUtcTicks -or -not $_.executable) {throw 'Incomplete identity'}
    [ordered]@{identity=$_;exact_owned_alive=[bool](Get-D1OwnedProcess $_)}
})
$config=@(
    @{name='title';path='config\custom_configs\config_BLES01857.yml';hash='22C081E2680A9F81C565860262232AF1296EBEA2831849338BA79BD6E7C2350B'},
    @{name='global';path='config\config.yml';hash='71C7E08939860F78C12A53D095DDC1CCD1525A46D1493714DEE6F9F85D229AB9'},
    @{name='watch';path='d1\d1ppu.txt';hash='442847E5847064F2D570943381CAA88DFD66E9DC212FFD299D9AC6D2F1EDD4B6'}
)|ForEach-Object {
    $actual=(Get-FileHash -LiteralPath (Join-Path $rpc $_.path)).Hash
    [ordered]@{name=$_.name;sha256=$actual;unchanged=($actual -eq $_.hash)}
}
$result=[ordered]@{utc=[datetime]::UtcNow.ToString('o');checks=$checks;
    all_exact_owned_closed=(-not($checks|Where-Object exact_owned_alive));configuration=@($config);
    all_hashes_match=(-not($config|Where-Object {-not $_.unchanged}))}
$output=Join-Path $research "$Run-independent-closure-verified.json"
if(Test-Path -LiteralPath $output) {throw 'Preserve prior verification'}
$result|ConvertTo-Json -Depth 8|Set-Content -LiteralPath $output -Encoding utf8
if(-not $result.all_exact_owned_closed -or -not $result.all_hashes_match) {throw 'Closure verification failed'}
"$Run : $($checks.Count) exact identities closed; all three configuration hashes restored."
