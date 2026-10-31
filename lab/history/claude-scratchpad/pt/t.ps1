param(
  [string]$Name = 'keep',
  [string]$Repo = (& "$PSScriptRoot/D1Paths.ps1" Workspace),
  [string]$Hosts = (& "$PSScriptRoot/D1Paths.ps1" Tools 'shadPS4\d1-hosts.txt')
)
"args before: $($args -join ',')"
. "$PSScriptRoot/D1Paths.ps1"
"name=$Name repo=$Repo hosts=$Hosts args after: $($args -join ',')"
Get-D1Path Staging
Get-D1Path ShadPS4 'user\log\shad_log.txt'
