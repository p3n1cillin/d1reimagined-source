# SPDX-License-Identifier: GPL-2.0-or-later
# D1Paths.ps1 - the PowerShell twin of d1paths.py: where the local D1 lab files live.
# Resolution (first match wins): environment variable (D1_ROOT, D1_TOOLS, ...), then the same name
# lowercase without D1_ ("root", "tools", ...) in %APPDATA%\d1reimagined\paths.json (Windows) or
# ${XDG_CONFIG_HOME:-~/.config}/d1reimagined/paths.json (D1_PATHS_CONFIG overrides), then the
# default below. Nothing is resolved until asked, so a script whose parameters are all given never
# needs D1_ROOT.
#
#   Root         D1_ROOT         required unless every location used is set directly
#   Tools        D1_TOOLS        Root\tools
#   ShadPS4      D1_SHADPS4      Tools\shadPS4\0.19.0
#   LibOrbisPkg  D1_LIBORBISPKG  Tools\liborbispkg\0.2.245
#   Workspace    D1_WORKSPACE    Root\d1preservation-main
#   Staging      D1_STAGING      Workspace\cfw-staging
#   PkgDir       D1_PKG_DIR      Root\01.29\d1shad
#
# In a script body:        . "$PSScriptRoot/D1Paths.ps1"; Get-D1Path Workspace
# In a param() default:    [string]$Repo = (& "$PSScriptRoot/D1Paths.ps1" Workspace)
#   (param defaults run before the body, so they call this file instead of dot-sourcing it.)

$script:D1PathLocations = [ordered]@{
    Root        = @{ Key = 'root';        Parent = $null;       Below = $null }
    Tools       = @{ Key = 'tools';       Parent = 'Root';      Below = 'tools' }
    ShadPS4     = @{ Key = 'shadps4';     Parent = 'Tools';     Below = 'shadPS4\0.19.0' }
    LibOrbisPkg = @{ Key = 'liborbispkg'; Parent = 'Tools';     Below = 'liborbispkg\0.2.245' }
    Workspace   = @{ Key = 'workspace';   Parent = 'Root';      Below = 'd1preservation-main' }
    Staging     = @{ Key = 'staging';     Parent = 'Workspace'; Below = 'cfw-staging' }
    PkgDir      = @{ Key = 'pkg_dir';     Parent = 'Root';      Below = '01.29\d1shad' }
}

function Get-D1PathConfigFile {
    if ($env:D1_PATHS_CONFIG) { return $env:D1_PATHS_CONFIG }
    $isWin = ($PSVersionTable.PSEdition -eq 'Desktop') -or $IsWindows
    if ($isWin) {
        $base = if ($env:APPDATA) { $env:APPDATA } else { Join-Path $HOME 'AppData\Roaming' }
    } else {
        $base = if ($env:XDG_CONFIG_HOME) { $env:XDG_CONFIG_HOME } else { Join-Path $HOME '.config' }
    }
    Join-Path (Join-Path $base 'd1reimagined') 'paths.json'
}

function ConvertTo-D1CleanPath([string]$Value) {
    $Value = [Environment]::ExpandEnvironmentVariables($Value)
    if ($Value -eq '~' -or $Value -match '^~[\\/]') { $Value = $HOME + $Value.Substring(1) }
    $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Value)
}

function Resolve-D1Path {
    [CmdletBinding()]
    param([Parameter(Mandatory)][string]$Name)
    $entry = $null
    foreach ($k in $script:D1PathLocations.Keys) { if ($k -eq $Name) { $Name = $k; $entry = $script:D1PathLocations[$k] } }
    if (-not $entry) { throw "Unknown D1 location '$Name' (known: $($script:D1PathLocations.Keys -join ', '))." }
    $envName = 'D1_' + $entry.Key.ToUpperInvariant()
    $value = [Environment]::GetEnvironmentVariable($envName)
    if ($value) { return [pscustomobject]@{ Name = $Name; Variable = $envName; Source = 'env'; Path = (ConvertTo-D1CleanPath $value) } }
    $file = Get-D1PathConfigFile
    if (Test-Path -LiteralPath $file -PathType Leaf) {
        try { $config = Get-Content -LiteralPath $file -Raw | ConvertFrom-Json -ErrorAction Stop }
        catch { throw "D1Paths: cannot read ${file}: $($_.Exception.Message)" }
        $value = $config.($entry.Key)
        if ($value) { return [pscustomobject]@{ Name = $Name; Variable = $envName; Source = 'config'; Path = (ConvertTo-D1CleanPath $value) } }
    }
    if (-not $entry.Parent) {
        $others = @($script:D1PathLocations.Values | Where-Object { $_.Parent } | ForEach-Object { 'D1_' + $_.Key.ToUpperInvariant() }) -join ', '
        throw ("D1Paths: the D1 root folder is not set. Either set the environment variable D1_ROOT " +
            "(e.g. D1_ROOT=D:\d1), or create $file containing {`"root`": `"D:/d1`"} " +
            "(see tools/ps4_setup/paths.example.json). Individual locations can be set the same way: $others.")
    }
    $parent = (Resolve-D1Path $entry.Parent).Path
    [pscustomobject]@{ Name = $Name; Variable = $envName; Source = 'default'; Path = (Join-Path $parent $entry.Below) }
}

function Get-D1Path {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory, Position = 0)]
        [ValidateSet('Root', 'Tools', 'ShadPS4', 'LibOrbisPkg', 'Workspace', 'Staging', 'PkgDir')]
        [string]$Name,
        # Optional path below the location, e.g. Get-D1Path Tools 'd1-pad.ps1'.
        [Parameter(Position = 1)][string]$ChildPath
    )
    $p = (Resolve-D1Path $Name).Path
    if ($ChildPath) { Join-Path $p $ChildPath } else { $p }
}

# Called (not dot-sourced) with a name: print that location, e.g. & D1Paths.ps1 Staging 'x.txt'.
# Called with no arguments: list every location and its source.
if ($MyInvocation.InvocationName -ne '.') {
    if ($args.Count) { Get-D1Path @args }
    else {
        "config file: $(Get-D1PathConfigFile)"
        foreach ($k in $script:D1PathLocations.Keys) {
            try { $r = Resolve-D1Path $k; '{0,-12} {1,-15} {2,-8} {3}' -f $r.Name, $r.Variable, $r.Source, $r.Path }
            catch { '{0,-12} {1,-15} unresolved ({2})' -f $k, ('D1_' + $script:D1PathLocations[$k].Key.ToUpperInvariant()), $_.Exception.Message }
        }
    }
}
