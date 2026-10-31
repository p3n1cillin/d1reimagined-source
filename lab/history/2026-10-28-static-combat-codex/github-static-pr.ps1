param([ValidateSet('Read','Create','Inspect')][string]$Mode='Read',[int]$Number=60,[string]$BodyFile)
$ErrorActionPreference='Stop'
$lines="protocol=https`nhost=github.com`n`n" | git credential fill 2>$null
if($LASTEXITCODE){throw 'GitHub credential unavailable'}
$fields=@{};foreach($line in $lines){$a=$line -split '=',2;if($a.Length -eq 2){$fields[$a[0]]=$a[1]}}
if(!$fields.password){throw 'No GitHub credential'}
$headers=@{Authorization=('Bearer '+$fields.password);Accept='application/vnd.github+json';'X-GitHub-Api-Version'='2022-11-28';'User-Agent'='Codex-D1-static-research'}
$uri='https://api.github.com/repos/p3n1cillin/d1reimagined-source'
$branch='codex/static-combat-dock13'
if($Mode -in @('Read','Inspect')) {
 $p=Invoke-RestMethod -Uri "$uri/pulls/$Number" -Headers $headers
 [pscustomobject]@{number=$p.number;url=$p.html_url;head=$p.head.ref;sha=$p.head.sha;state=$p.state;merged=$p.merged;draft=$p.draft} | ConvertTo-Json
 if($Mode -eq 'Read'){
  $files=Invoke-RestMethod -Uri "$uri/pulls/$Number/files" -Headers $headers
  $files | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $PSScriptRoot "pr$Number-files.json") -Encoding utf8
  $files | ForEach-Object { $_.filename; $_.patch }
 }
} else {
 if((git branch --show-current) -ne $branch){throw 'Wrong source branch'}
 if(git status --porcelain){throw 'Uncommitted changes'}
 $local=git rev-parse HEAD;$remote=git ls-remote origin "refs/heads/$branch"
 if(!$remote.StartsWith($local)){throw 'Push complete branch before creating PR'}
 $repo=Invoke-RestMethod -Uri $uri -Headers $headers
 if($repo.full_name -ne 'p3n1cillin/d1reimagined-source' -or !$repo.permissions.push){throw 'Wrong repository or no push permission'}
 $existing=Invoke-RestMethod -Uri ($uri+'/pulls?state=open&head='+[Uri]::EscapeDataString('p3n1cillin:'+$branch)) -Headers $headers
 if($existing.Count){throw 'PR already exists'}
 $body=@{title='56578: encode native pilot programs and document the missing activity clock';head=$branch;base='main';draft=$false;body=[IO.File]::ReadAllText((Get-Item -LiteralPath $BodyFile).FullName)}|ConvertTo-Json
 $p=Invoke-RestMethod -Method Post -Uri "$uri/pulls" -Headers $headers -ContentType application/json -Body $body
 if($p.head.ref -ne $branch -or $p.base.ref -ne 'main'){throw 'Created PR identity mismatch'}
 [pscustomobject]@{number=$p.number;url=$p.html_url;head=$p.head.ref;sha=$p.head.sha;draft=$p.draft}|ConvertTo-Json
}
