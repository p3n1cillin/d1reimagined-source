param([ValidateSet('Create','Read')][string]$Mode='Read',[int]$Number=0,[string]$Title,[string]$BodyFile,[ValidateSet('codex/sensor-sense','codex/world-boot-wait','codex/live-unit-tracking','codex/sense-validation-notes')][string]$Branch='codex/sensor-sense',[switch]$Draft)
$ErrorActionPreference='Stop'
$lines="protocol=https`nhost=github.com`n`n" | git credential fill 2>$null
if($LASTEXITCODE){throw 'GitHub credential lookup failed'}
$fields=@{};foreach($line in $lines){$parts=$line -split '=',2;if($parts.Length-eq 2){$fields[$parts[0]]=$parts[1]}}
if(!$fields.password){throw 'GitHub credential unavailable'}
$headers=@{Authorization=('Bearer '+$fields.password);Accept='application/vnd.github+json';'X-GitHub-Api-Version'='2022-11-28';'User-Agent'='Codex-D1-sense-research'}
$uri='https://api.github.com/repos/p3n1cillin/d1reimagined-source';$branch=$Branch
if($Mode-eq 'Create'){
 $existing=Invoke-RestMethod -Uri ($uri+'/pulls?state=open&head='+[Uri]::EscapeDataString('p3n1cillin:'+$branch)) -Headers $headers
 if($existing.Count){throw 'PR already exists'}
 $body=@{title=$Title;head=$branch;base='main';draft=$Draft.IsPresent;body=[IO.File]::ReadAllText((Get-Item -LiteralPath $BodyFile).FullName)}|ConvertTo-Json
 $p=Invoke-RestMethod -Method Post -Uri ($uri+'/pulls') -Headers $headers -ContentType application/json -Body $body
 if($p.head.ref-ne $branch -or $p.base.ref-ne 'main'){throw 'Created PR identity mismatch'}
}else{
 if($Number-lt 1){throw 'Need PR number'}
 $p=Invoke-RestMethod -Uri ($uri+'/pulls/'+$Number) -Headers $headers
 if($p.head.ref-ne $branch){throw 'PR identity mismatch'}
}
[pscustomobject]@{number=$p.number;url=$p.html_url;head=$p.head.ref;sha=$p.head.sha;state=$p.state;merged=$p.merged;draft=$p.draft}|ConvertTo-Json
