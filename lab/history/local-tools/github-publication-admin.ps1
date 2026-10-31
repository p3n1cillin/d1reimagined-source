param([ValidateSet('aster-chess','d1reimagined-source')][string]$Repository,
      [ValidateSet('Read','PublishAster','Topics')][string]$Mode='Read')
$ErrorActionPreference='Stop'
$credentialLines="protocol=https`nhost=github.com`n`n" | git credential fill 2>$null
if($LASTEXITCODE-ne 0){throw 'GitHub credential lookup failed'}
$credentialFields=@{}
foreach($line in $credentialLines){$parts=$line -split '=',2; if($parts.Length-eq 2){$credentialFields[$parts[0]]=$parts[1]}}
if(!$credentialFields.password){throw 'GitHub credential unavailable'}
$headers=@{Authorization=('Bearer '+$credentialFields.password);Accept='application/vnd.github+json';'X-GitHub-Api-Version'='2022-11-28';'User-Agent'='Codex-repository-publication'}
$uri='https://api.github.com/repos/p3n1cillin/'+$Repository
$repo=Invoke-RestMethod -Uri $uri -Headers $headers
if($repo.full_name-ne ('p3n1cillin/'+$Repository) -or !$repo.permissions.admin){throw 'Repository identity/admin gate failed'}
if($Mode-eq 'PublishAster'){
    if($Repository-ne 'aster-chess'){throw 'Visibility mutation restricted to authorized Aster repository'}
    $body=@{visibility='public';has_issues=$true;description='Independent C# chess engine, playable browser interface and reproducible playing-strength research.'} | ConvertTo-Json
    $repo=Invoke-RestMethod -Method Patch -Uri $uri -Headers $headers -ContentType application/json -Body $body
    if($repo.private -or $repo.visibility-ne 'public'){throw 'Public visibility not confirmed'}
}
if($Mode-eq 'Topics'){
    $topics=if($Repository-eq 'aster-chess'){@('chess','chess-engine','uci','dotnet','csharp','research')}else{@('destiny','preservation','emulation','rust','python','rpcs3','shadps4')}
    Invoke-RestMethod -Method Put -Uri ($uri+'/topics') -Headers $headers -ContentType application/json -Body (@{names=$topics}|ConvertTo-Json) | Out-Null
}
[pscustomobject]@{repository=$repo.full_name;visibility=$repo.visibility;url=$repo.html_url;branch=$repo.default_branch;license=$repo.license.spdx_id} | ConvertTo-Json
