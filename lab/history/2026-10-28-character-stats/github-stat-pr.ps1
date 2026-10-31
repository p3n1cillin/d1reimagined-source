param(
    [ValidateSet('List','Create','Read')][string]$Mode='List',
    [string]$Title,
    [string]$BodyFile,
    [int]$Number=0
)
$ErrorActionPreference='Stop'
$credentialLines="protocol=https`nhost=github.com`n`n" | git credential fill 2>$null
if ($LASTEXITCODE) { throw 'GitHub credential lookup failed' }
$credentialFields=@{}
foreach ($line in $credentialLines) {
    $parts=$line -split '=',2
    if ($parts.Length -eq 2) { $credentialFields[$parts[0]]=$parts[1] }
}
if (!$credentialFields.password) { throw 'GitHub credential unavailable' }
$headers=@{Authorization=('Bearer '+$credentialFields.password);Accept='application/vnd.github+json';'X-GitHub-Api-Version'='2022-11-28';'User-Agent'='Codex-D1-stat-research'}
$uri='https://api.github.com/repos/p3n1cillin/d1reimagined-source'
$repo=Invoke-RestMethod -Uri $uri -Headers $headers
if ($repo.full_name -ne 'p3n1cillin/d1reimagined-source' -or !$repo.permissions.push) { throw 'Repository identity/push gate failed' }
$branch='codex/character-live-stats'
if ($Mode -eq 'List') {
    $rows=Invoke-RestMethod -Uri ($uri+'/pulls?state=open&head='+[Uri]::EscapeDataString('p3n1cillin:'+$branch)) -Headers $headers
    @($rows | ForEach-Object { [pscustomobject]@{number=$_.number;url=$_.html_url;head=$_.head.ref;draft=$_.draft;title=$_.title} }) | ConvertTo-Json -Depth 4
} elseif ($Mode -eq 'Read') {
    if ($Number -lt 1) { throw 'Need PR number' }
    $p=Invoke-RestMethod -Uri "$uri/pulls/$Number" -Headers $headers
    if ($p.head.ref -ne $branch) { throw 'Unexpected PR head' }
    [pscustomobject]@{number=$p.number;url=$p.html_url;head=$p.head.ref;sha=$p.head.sha;state=$p.state;merged=$p.merged;draft=$p.draft;mergeable=$p.mergeable} | ConvertTo-Json
} else {
    if (!$Title -or !(Test-Path -LiteralPath $BodyFile -PathType Leaf)) { throw 'Need concrete title and body file' }
    $existing=Invoke-RestMethod -Uri ($uri+'/pulls?state=open&head='+[Uri]::EscapeDataString('p3n1cillin:'+$branch)) -Headers $headers
    if ($existing.Count -gt 0) { throw 'A PR already exists for this branch' }
    $body=@{title=$Title;head=$branch;base='main';draft=$true;body=[IO.File]::ReadAllText((Get-Item -LiteralPath $BodyFile).FullName)} | ConvertTo-Json
    $p=Invoke-RestMethod -Method Post -Uri "$uri/pulls" -Headers $headers -ContentType application/json -Body $body
    if ($p.head.ref -ne $branch -or $p.base.ref -ne 'main' -or !$p.draft) { throw 'Created PR identity/draft mismatch' }
    $result=[pscustomobject]@{number=$p.number;url=$p.html_url;head=$p.head.ref;sha=$p.head.sha;draft=$p.draft}
    $result | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $PSScriptRoot 'source-pr-receipt.json')
    $result | ConvertTo-Json
}
