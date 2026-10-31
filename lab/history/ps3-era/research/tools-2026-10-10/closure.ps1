param([Parameter(Mandatory)][string]$Run)
$rt='F:\d1\<pc>-import\d1xenia\local'; $rpc="$rt\RPCS3_FINAL124_SMOKE001"; $r="$rt\RESEARCH"
. F:\d1\d1xenia\scripts\Final124RunHelpers.ps1
$m=Read-D1ProcessManifest "$rt\SIGNON_LOCAL\logs\FINAL124_$Run-processes.json"
$checks=@(foreach($p in $m.processes){ [ordered]@{identity=$p; exact_owned_alive=[bool](Get-D1OwnedProcess $p)} })
$cfg=@(@('title','config\custom_configs\config_BLES01857.yml','22C081E2680A9F81C565860262232AF1296EBEA2831849338BA79BD6E7C2350B'),@('global','config\config.yml','71C7E08939860F78C12A53D095DDC1CCD1525A46D1493714DEE6F9F85D229AB9'),@('watch','d1\d1ppu.txt','442847E5847064F2D570943381CAA88DFD66E9DC212FFD299D9AC6D2F1EDD4B6') | ForEach-Object { $h=(Get-FileHash (Join-Path $rpc $_[1])).Hash; [ordered]@{name=$_[0];sha256=$h;unchanged=($h -eq $_[2])} })
$o=[ordered]@{utc=[datetime]::UtcNow.ToString('o');checks=$checks;all_exact_owned_closed=-not @($checks|Where-Object exact_owned_alive).Count;configuration=$cfg;all_hashes_match=-not @($cfg|Where-Object {-not $_.unchanged}).Count}
$o|ConvertTo-Json -Depth 6|Set-Content "$r\$Run-independent-closure-verified.json" -Encoding utf8
"$Run closed=$($o.all_exact_owned_closed) hashes=$($o.all_hashes_match)"
