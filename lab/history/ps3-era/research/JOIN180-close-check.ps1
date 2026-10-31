$ErrorActionPreference='Stop'
. 'F:\d1\d1xenia\scripts\Final124RunHelpers.ps1'
$runtime='F:\d1\<pc>-import\d1xenia\local'
$logs=Join-Path $runtime 'SIGNON_LOCAL\logs'
$research=Join-Path $runtime 'RESEARCH'
$rpc=Join-Path $runtime 'RPCS3_FINAL124_SMOKE001'
$manifest=Read-D1ProcessManifest -Path (Join-Path $logs 'FINAL124_JOIN180-processes.json')
$identities=@($manifest.processes)
foreach($name in @('FINAL124_JOIN180-proxy-process.json','JOIN180-session-probe-process.json')) {
  $identities+=Get-Content -LiteralPath (Join-Path $logs $name) -Raw | ConvertFrom-Json
}
foreach($name in @('JOIN180-route-helper-identity.json','JOIN180-observer-helper-identity.json','JOIN180-corridor-helper-identity.json','JOIN180-after-corridor-helper-identity.json')) {
  $identities+=Get-Content -LiteralPath (Join-Path $research $name) -Raw | ConvertFrom-Json
}
$checks=@($identities | ForEach-Object { [pscustomobject]@{id=$_.id; startTimeUtcTicks=$_.startTimeUtcTicks; executable=$_.executable; exact_owned_alive=[bool](Get-D1OwnedProcess -Identity $_)} })
$files=@(
  @{name='title';path=(Join-Path $rpc 'config\custom_configs\config_BLES01857.yml');expected='22C081E2680A9F81C565860262232AF1296EBEA2831849338BA79BD6E7C2350B'},
  @{name='global';path=(Join-Path $rpc 'config\config.yml');expected='71C7E08939860F78C12A53D095DDC1CCD1525A46D1493714DEE6F9F85D229AB9'},
  @{name='watch';path=(Join-Path $rpc 'd1\d1ppu.txt');expected='442847E5847064F2D570943381CAA88DFD66E9DC212FFD299D9AC6D2F1EDD4B6'}
)
$hashes=@($files | ForEach-Object { $actual=(Get-FileHash -LiteralPath $_.path -Algorithm SHA256).Hash; [pscustomobject]@{name=$_.name;path=$_.path;sha256=$actual;unchanged=$actual -eq $_.expected} })
$result=[pscustomobject]@{at=(Get-Date).ToUniversalTime().ToString('o');checks=$checks;all_exact_owned_closed= -not ($checks.exact_owned_alive -contains $true);configuration=$hashes;all_hashes_match= -not ($hashes.unchanged -contains $false)}
$result | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $research 'JOIN180-independent-closed.json') -Encoding utf8
$result | ConvertTo-Json -Depth 8
