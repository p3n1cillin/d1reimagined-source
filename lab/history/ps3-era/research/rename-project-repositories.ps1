$ErrorActionPreference='Stop'
$env:GIT_TERMINAL_PROMPT='0'
$env:GCM_INTERACTIVE='Never'
$credentialLines=@("protocol=https", "host=github.com", "", "") | git credential fill
if ($LASTEXITCODE -ne 0) { throw 'Existing GitHub Git credential unavailable.' }
$credential=@{}
foreach ($line in $credentialLines) {
    $parts=$line -split '=',2
    if($parts.Count -eq 2) { $credential[$parts[0]]=$parts[1] }
}
if(-not $credential.password) { throw 'GitHub credential has no access token.' }
$headers=@{Authorization=('Bearer '+$credential.password);Accept='application/vnd.github+json';'X-GitHub-Api-Version'='2022-11-28'}
$pairs=@(
    @{old='d1xenia';new='d1reimagined';path='F:\d1\d1xenia'},
    @{old='d1xenia-data';new='d1reimagined-data';path='F:\d1\d1xenia-data'})
$receipt=[Collections.Generic.List[object]]::new()
foreach($pair in $pairs) {
    $before=Invoke-RestMethod -Uri ('https://api.github.com/repos/p3n1cillin/'+$pair.old) -Headers $headers
    if($pair.old -eq 'd1xenia-data' -and -not $before.private) { throw 'Companion data visibility differs from documented private policy; investigate.' }
    $item=Invoke-RestMethod -Method Patch -Uri ('https://api.github.com/repos/p3n1cillin/'+$pair.old) -Headers $headers -ContentType 'application/json' -Body (@{name=$pair.new}|ConvertTo-Json -Compress)
    if($item.name -ne $pair.new -or $item.id -ne $before.id -or $item.private -ne $before.private) { throw 'Renamed repository identity or visibility check failed.' }
    git -C $pair.path remote set-url origin $item.clone_url
    if($LASTEXITCODE -ne 0) { throw 'Local origin update failed.' }
    $verified=Invoke-RestMethod -Uri ('https://api.github.com/repos/p3n1cillin/'+$pair.new) -Headers $headers
    if($verified.id -ne $before.id) { throw 'Renamed API endpoint identity check failed.' }
    $result=[ordered]@{at=[DateTime]::UtcNow.ToString('o');old_name=$pair.old;name=$item.name;full_name=$item.full_name;private=$item.private;default_branch=$item.default_branch;repository_id=$item.id;url=$item.html_url;checkout=$pair.path;origin=$item.clone_url}
    $receipt.Add($result)
    $result | ConvertTo-Json -Compress
}
$receipt.ToArray() | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath 'lab/history\2026-10-06-repository-renames.json' -Encoding utf8
