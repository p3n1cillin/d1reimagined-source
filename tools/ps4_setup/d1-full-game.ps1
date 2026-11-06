# SPDX-License-Identifier: GPL-2.0-or-later
<#
.SYNOPSIS
One command for the full-game test: every server with every new subsystem switched on, then the
emulator, the way the lab launchers start them (Start-D1ContainedBoot.ps1 on the D1 Lab adapter).

.DESCRIPTION
Reads a launch profile (ps4/profiles/<name>.env; format in ps4/profiles/lab-base.env), resolves it,
and checks every input before anything starts: the eboot and manifest against the build registry,
the emulator, the client patch, every fixture and table a knob names, the store folder, and (with
the script bridge on) node, the script folder, the tiger checkout and its built BullShcript. Every
knob's name must be in the binary that reads it (so a rename or a stale build is caught); a `?KEY`
knob is dropped, and reported, when its code is not in the binary yet.

It builds the release servers when they are stale (missing, older than this checkout's sources, or
built from another checkout sharing CARGO_TARGET_DIR), copies them into the run folder, writes the
run's knobs and a manifest there, and starts Start-D1ContainedBoot.ps1 in the background. The
contained boot checks the network containment itself and refuses to start on any failure; this
script never changes firewall rules, adapters or hosts entries.

  d1-full-game.ps1 -DryRun                        # print everything, start nothing
  d1-full-game.ps1 -Name fg1                       # build if stale, start servers and emulator
  d1-full-game.ps1 -Profile baseline -Name base1   # the same session with the new subsystems off
  d1-full-game.ps1 -Only servers -Name fg2         # servers only (stop: create RUN\boot\stop)
  d1-full-game.ps1 -Only emulator -Name fg2e       # the emulator against servers already up
  d1-full-game.ps1 -Set D1_DRAW_FAST=0,D1_FPS=60   # per-knob overrides (later wins)

docs/full-game-test.md says what each subsystem should log to count as working.
#>
[CmdletBinding()]
param(
    # The run's name: the run folder is STAGING\full-game\<utc>-<Name>.
    [ValidatePattern('^[A-Za-z0-9_-]{1,64}$')][string]$Name = 'full-game',
    # A profile name in ps4/profiles (full-game, baseline) or a path to a .env file.
    [string]$Profile = 'full-game',
    # all = servers and emulator; servers = servers only; emulator = the emulator only, against
    # servers already listening (an earlier -Only servers run).
    [ValidateSet('all', 'servers', 'emulator')][string]$Only = 'all',
    # Overrides, KEY=VALUE (comma-joined or repeated). A key the profile lacks needs its section:
    # server:D1_X=1, sidecar:..., emulator:..., launch:...
    [string[]]$Set = @(),
    # Run cargo even when the servers look fresh (cargo itself skips unchanged crates).
    [switch]$Build,
    # Never build: refuse when the servers are stale.
    [switch]$NoBuild,
    # Resolve and check everything, print it, and start nothing (no files written, no build).
    [switch]$DryRun,
    # With -DryRun: print the manifest as JSON only.
    [switch]$Json,
    # The cargo target folder (default: this checkout's ps4/d1-server/target; CARGO_TARGET_DIR is ignored).
    [string]$TargetDir = ''
)
$ErrorActionPreference = 'Stop'

$Repo = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$Profiles = Join-Path $Repo 'ps4/profiles'
$Servers = @('d1-responder', 'd1-activity-host', 'shadnet-server', 'd1-oracle')
$Sections = @('launch', 'server', 'sidecar', 'emulator')
$OffValues = @('', '0')
$IsWin = ($PSVersionTable.PSEdition -eq 'Desktop') -or $IsWindows
$Exe = if ($IsWin) { '.exe' } else { '' }
$problems = [Collections.Generic.List[string]]::new()
$warnings = [Collections.Generic.List[string]]::new()
function Problem([string]$m) { $problems.Add($m) }
function Warn([string]$m) { $warnings.Add($m) }

# -- locations ------------------------------------------------------------------------------------
. (Join-Path $PSScriptRoot 'D1Paths.ps1')
$loc = [ordered]@{ REPO = $Repo; PROFILES = $Profiles }
foreach ($pair in @(@('ROOT', 'Root'), @('TOOLS', 'Tools'), @('WORKSPACE', 'Workspace'), @('STAGING', 'Staging'),
        @('FIXTURES', 'Fixtures'), @('SHADPS4', 'ShadPS4'))) {
    try { $loc[$pair[0]] = Get-D1Path $pair[1] }
    catch { $loc[$pair[0]] = "<unresolved $($pair[1])>"; Problem "location $($pair[1]): $($_.Exception.Message)" }
}
$stamp = [DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss')
$loc['RUN'] = Join-Path (Join-Path $loc.STAGING 'full-game') "$stamp-$Name"
# One fixed folder for the servers a run uses. Windows Firewall keys its answer to a program's path,
# so a new folder per run asked again on every run (6 Nov 2026). Only one lab runs at a time (the
# launcher refuses over a running one), and each manifest records the SHA-256 of what it ran.
$loc['BIN'] = Join-Path (Join-Path $loc.STAGING 'full-game') 'bin'

# -- the profile ----------------------------------------------------------------------------------
function Resolve-ProfilePath([string]$p, [string]$base) {
    if (Test-Path -LiteralPath $p -PathType Leaf) { return (Resolve-Path -LiteralPath $p).Path }
    $cand = @((Join-Path $base $p), (Join-Path $base "$p.env"))
    foreach ($c in $cand) { if (Test-Path -LiteralPath $c -PathType Leaf) { return (Resolve-Path -LiteralPath $c).Path } }
    throw "profile not found: $p (looked in $base)"
}
$knobs = [ordered]@{}   # KEY -> entry
$chain = [Collections.Generic.List[string]]::new()
function Read-Profile([string]$path, [string[]]$stack) {
    if ($stack -contains $path) { throw "profile include cycle: $(($stack + $path) -join ' -> ')" }
    $chain.Add($path)
    $section = $null; $comment = ''; $n = 0
    foreach ($raw in Get-Content -LiteralPath $path) {
        $n++
        $line = $raw.Trim()
        if (-not $line) { $comment = ''; continue }
        if ($line.StartsWith('#')) { $comment = $line.TrimStart('#').Trim(); continue }
        if ($line -match '^@include\s+(\S+)$') {
            Read-Profile (Resolve-ProfilePath $Matches[1] (Split-Path $path)) ($stack + $path); $comment = ''; continue
        }
        if ($line -match '^\[(\w+)\]$') {
            if ($Sections -notcontains $Matches[1]) { throw "${path}:${n}: unknown section [$($Matches[1])]" }
            $section = $Matches[1]; $comment = ''; continue
        }
        if ($line -notmatch '^(\?)?([A-Za-z_][A-Za-z0-9_]*)=(.*)$') { throw "${path}:${n}: not KEY=VALUE: $raw" }
        $opt = [bool]$Matches[1]; $key = $Matches[2]; $value = $Matches[3]
        if (-not $section) { throw "${path}:${n}: $key is outside a section" }
        if ($section -ne 'launch' -and $key -notmatch '^D1_[A-Z0-9_]+$') { throw "${path}:${n}: $key is not a D1_ knob" }
        if ($knobs.Contains($key) -and $knobs[$key].section -ne $section) {
            throw "${path}:${n}: $key is in [$section] here but [$($knobs[$key].section)] in $($knobs[$key].file)"
        }
        $knobs[$key] = [ordered]@{ section = $section; key = $key; raw = $value; optional = $opt
            source = $comment; file = (Split-Path $path -Leaf); line = $n; override = $false }
        $comment = ''
    }
}
try { $profilePath = Resolve-ProfilePath $Profile $Profiles; Read-Profile $profilePath @() }
catch { Write-Error "d1-full-game: $($_.Exception.Message)"; exit 1 }

# -Set: through `pwsh -File` an array arrives as one comma-joined string; split only at commas that
# start a new KEY= (values may hold commas), as d1-lab-served.ps1 does.
$sets = @(foreach ($s in $Set) { $s -split ',(?=(?:(?:launch|server|sidecar|emulator):)?[A-Za-z_][A-Za-z0-9_]*=)' })
foreach ($s in $sets | Where-Object { $_ }) {
    if ($s -notmatch '^(?:(launch|server|sidecar|emulator):)?([A-Za-z_][A-Za-z0-9_]*)=(.*)$') { Write-Error "d1-full-game: -Set $s is not [section:]KEY=VALUE"; exit 1 }
    $sec = $Matches[1]; $key = $Matches[2]; $value = $Matches[3]
    if ($knobs.Contains($key)) {
        if ($sec -and $sec -ne $knobs[$key].section) { Write-Error "d1-full-game: -Set $key is in [$($knobs[$key].section)], not [$sec]"; exit 1 }
        $knobs[$key].raw = $value; $knobs[$key].override = $true; $knobs[$key].optional = $false
    } elseif ($sec) {
        if ($sec -ne 'launch' -and $key -notmatch '^D1_[A-Z0-9_]+$') { Write-Error "d1-full-game: -Set $key is not a D1_ knob"; exit 1 }
        $knobs[$key] = [ordered]@{ section = $sec; key = $key; raw = $value; optional = $false; source = '-Set'; file = '-Set'; line = 0; override = $true }
    } else { Write-Error "d1-full-game: -Set $key is not in the profile; name its section (server:$key=...)"; exit 1 }
}

# -- placeholders ---------------------------------------------------------------------------------
$launch = [ordered]@{}
function Expand([string]$v, [string]$what) {
    [regex]::Replace($v, '\$\{(env:)?([A-Za-z0-9_]+)\}', {
        param($m)
        $n = $m.Groups[2].Value
        if ($m.Groups[1].Success) { $e = [Environment]::GetEnvironmentVariable($n); if ($null -eq $e) { '' } else { $e } }
        elseif ($launch.Contains($n)) { $launch[$n] }
        elseif ($loc.Contains($n)) { $loc[$n] }
        else { Problem "${what}: unknown placeholder `${$n}"; "<unknown $n>" }
    })
}
function Tidy([string]$v) {
    # Paths built from ${LOCATION}/a/b read better with one separator; leave lists and commands alone.
    if ($IsWin -and $v -match '^[A-Za-z]:\\' -and $v -notmatch '[\s,;]') { $v -replace '/', '\' } else { $v }
}
foreach ($k in @($knobs.Keys)) {
    $e = $knobs[$k]
    if ($e.section -eq 'launch') { $launch[$k] = Tidy (Expand $e.raw $k); $e.value = $launch[$k] }
}
foreach ($k in @($knobs.Keys)) {
    $e = $knobs[$k]
    if ($e.section -ne 'launch') { $e.value = Tidy (Expand $e.raw $k) }
}
function L([string]$k, [string]$default = '') { if ($launch.Contains($k)) { $launch[$k] } else { $default } }
$LabIp = L 'LAB_IP' '10.77.0.2'
$Minutes = [int](L 'MINUTES' '240')

# The sidecar's node: the profile's, else node on PATH.
$node = L 'NODE'
if ($node -and -not (Test-Path -LiteralPath $node -PathType Leaf)) {
    $onPath = Get-Command node -ErrorAction SilentlyContinue
    if ($onPath) {
        Warn "NODE $node is absent; using node on PATH ($($onPath.Source))"
        foreach ($k in @($knobs.Keys)) { if ($knobs[$k].value -and $knobs[$k].value.Contains($node)) { $knobs[$k].value = $knobs[$k].value.Replace($node, $onPath.Source) } }
        $node = $onPath.Source; $launch['NODE'] = $node
    }
}

# -- the binaries ---------------------------------------------------------------------------------
# This checkout's own target folder by default: one CARGO_TARGET_DIR shared between worktrees has
# left one checkout's binaries under another's name and corrupted builds.
if (-not $TargetDir) {
    $TargetDir = Join-Path $Repo 'ps4/d1-server/target'
    if ($env:CARGO_TARGET_DIR -and (Resolve-Path -LiteralPath $env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue).Path -ne (Resolve-Path -LiteralPath $TargetDir -ErrorAction SilentlyContinue).Path) {
        Warn "CARGO_TARGET_DIR ($env:CARGO_TARGET_DIR) is ignored: building into this checkout's $TargetDir (pass -TargetDir to choose)"
    }
}
$release = Join-Path $TargetDir 'release'
$crateRoot = (Join-Path $Repo 'ps4/d1-server/crates').ToLowerInvariant().Replace('\', '/')
function Get-DepFiles([string]$depInfo) {
    # cargo's dep-info: "<target>: <dep> <dep> ...", spaces in paths escaped as "\ ".
    $text = Get-Content -LiteralPath $depInfo -Raw
    $i = $text.IndexOf(': ')
    if ($i -lt 0) { return @() }
    $body = $text.Substring($i + 2).Replace("\`r`n", ' ').Replace("\`n", ' ')
    @([regex]::Split($body.Trim(), '(?<!\\)\s+') | Where-Object { $_ } | ForEach-Object { $_.Replace('\ ', ' ') })
}
function Get-ServerState([string]$bin) {
    $path = Join-Path $release "$bin$Exe"
    $s = [ordered]@{ name = $bin; path = $path; exists = (Test-Path -LiteralPath $path -PathType Leaf); built = $null
        builtFrom = $null; stale = @(); sha256 = $null }
    if (-not $s.exists) { $s.stale = @('not built'); return $s }
    $exeTime = (Get-Item -LiteralPath $path).LastWriteTimeUtc
    $s.built = $exeTime.ToString('o')
    $d = Join-Path $release "$bin.d"
    if (-not (Test-Path -LiteralPath $d)) { $s.stale = @('no dep-info beside it'); return $s }
    $deps = Get-DepFiles $d
    $ours = @($deps | Where-Object { $_.ToLowerInvariant().Replace('\', '/').StartsWith($crateRoot) })
    $local = @($deps | Where-Object { $_ -match '[\\/]ps4[\\/]d1-server[\\/]crates[\\/]' })
    if ($local.Count) { $s.builtFrom = ($local[0] -split '[\\/]ps4[\\/]d1-server[\\/]crates[\\/]')[0] }
    if (-not $ours.Count) { $s.stale += "built from $($s.builtFrom), not this checkout"; return $s }
    $newer = @($ours | Where-Object { (Test-Path -LiteralPath $_) -and (Get-Item -LiteralPath $_).LastWriteTimeUtc -gt $exeTime })
    $gone = @($ours | Where-Object { -not (Test-Path -LiteralPath $_) })
    # The manifests of the workspace crates this binary is built from (cargo itself skips a binary
    # whose crates did not change, so another crate's Cargo.toml or the lock file says nothing).
    $crates = @($ours | ForEach-Object { ($_ -split '[\\/]ps4[\\/]d1-server[\\/]crates[\\/]')[1] -split '[\\/]' | Select-Object -First 1 } | Sort-Object -Unique)
    $manifests = @($crates | ForEach-Object { Get-Item -LiteralPath (Join-Path $Repo "ps4/d1-server/crates/$_/Cargo.toml") -ErrorAction SilentlyContinue })
    $newer += @($manifests | Where-Object { $_.LastWriteTimeUtc -gt $exeTime } | ForEach-Object { $_.FullName })
    if ($newer.Count) { $s.stale += "$($newer.Count) source file(s) newer, e.g. $($newer[0] -replace [regex]::Escape($Repo + [IO.Path]::DirectorySeparatorChar), '')" }
    if ($gone.Count) { $s.stale += "$($gone.Count) source file(s) it was built from are gone" }
    $s
}
$serverState = [ordered]@{}
foreach ($b in $Servers) { $serverState[$b] = Get-ServerState $b }
$stale = @($Servers | Where-Object { $serverState[$_].stale.Count })
$wantBuild = ($Only -ne 'emulator') -and ($Build -or $stale.Count)
$buildCmd = "cargo build --release $(($Servers | ForEach-Object { "-p $_" }) -join ' ')"
if ($wantBuild -and $NoBuild) { Problem "the servers are stale ($(($stale | ForEach-Object { "${_}: $($serverState[$_].stale -join '; ')" }) -join ' | ')) and -NoBuild was given" }

# -- for real: refuse over a live lab, then build (the knob checks below read the built servers) ---
$didBuild = $false
$log = Join-Path $loc.RUN 'launcher.log'
function Say([string]$m) { $m; if (-not $DryRun) { Add-Content -LiteralPath $log -Value "$([DateTime]::UtcNow.ToString('o')) $m" } }
if (-not $DryRun) {
    if (-not $IsWin) { Write-Error 'd1-full-game: the lab runs on Windows only (use -DryRun elsewhere)'; exit 1 }
    $running = @(Get-Process shadPS4, d1-activity-host, d1-responder, shadnet-server -ErrorAction SilentlyContinue)
    if ($Only -ne 'emulator' -and $running.Count) { Write-Error "d1-full-game: a lab run is active (process ids $($running.Id -join ', ')); end it first"; exit 2 }
    if ($Only -eq 'emulator' -and @($running | Where-Object ProcessName -eq 'shadPS4').Count) { Write-Error 'd1-full-game: an emulator is already running'; exit 2 }
    if ($problems.Count) { $problems | ForEach-Object { Write-Error "d1-full-game: $_" -ErrorAction Continue }; exit 2 }
    New-Item -ItemType Directory -Force $loc.BIN | Out-Null
    Say "d1-full-game $Name ($Only), profile $profilePath"
    if ($wantBuild) {
        if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { Say 'cargo is not on PATH'; exit 3 }
        $env:CARGO_TARGET_DIR = $TargetDir
        # unicorn-engine-sys runs bindgen, which needs libclang: use the lab's LLVM when unset.
        if (-not $env:LIBCLANG_PATH) {
            $clang = @(Get-ChildItem -Path (Join-Path $loc.TOOLS 'llvm*\bin\libclang.dll'), (Join-Path $loc.TOOLS 'LLVM*\bin\libclang.dll') -ErrorAction SilentlyContinue | Sort-Object FullName -Descending)
            if ($clang.Count) { $env:LIBCLANG_PATH = $clang[0].DirectoryName; Say "LIBCLANG_PATH=$($env:LIBCLANG_PATH)" }
        }
        for ($try = 1; $try -le 2; $try++) {
            Say "build ($try): $buildCmd at BelowNormal priority, log $($loc.RUN)\build-$try.log"
            $cargoArgs = @('build', '--release') + @($Servers | ForEach-Object { '-p', $_ })
            $p = Start-Process cargo -ArgumentList $cargoArgs -WorkingDirectory (Join-Path $Repo 'ps4/d1-server') -PassThru -NoNewWindow `
                -RedirectStandardOutput (Join-Path $loc.RUN "build-$try.out.log") -RedirectStandardError (Join-Path $loc.RUN "build-$try.log")
            # rustc and the build scripts start after this and inherit the class.
            try { $p.PriorityClass = 'BelowNormal' } catch { }
            $null = $p.Handle; $p.WaitForExit()
            if ($p.ExitCode -ne 0) { Say "build failed (exit $($p.ExitCode)); see build-$try.log"; exit 3 }
            foreach ($b in $Servers) { $serverState[$b] = Get-ServerState $b }
            $other = @($Servers | Where-Object { $serverState[$_].stale -match 'not this checkout' })
            if (-not $other.Count) { break }
            # A target folder shared with another checkout: cargo found this checkout's crates fresh
            # and left the other checkout's binary in place. Touch each binary's root to relink it here.
            foreach ($b in $other) {
                $main = Join-Path $Repo "ps4/d1-server/crates/$b/src/main.rs"
                Say "  $b was built from $($serverState[$b].builtFrom): touching $main"
                (Get-Item -LiteralPath $main).LastWriteTimeUtc = [DateTime]::UtcNow
            }
        }
        # A file whose time changed but whose content did not (a git checkout rewrites mtimes) is
        # fresh to cargo, which then leaves the binary alone, so the time check still calls it stale
        # (run fg1, 6 Nov 2026). Cargo's clean exit is the authority: bring the binary's time forward.
        foreach ($b in @($Servers | Where-Object { $serverState[$_].stale.Count -and
                -not @($serverState[$_].stale | Where-Object { $_ -notmatch 'newer' }).Count })) {
            Say "  $b is fresh to cargo; only file times were newer: touching the binary"
            (Get-Item -LiteralPath $serverState[$b].path).LastWriteTimeUtc = [DateTime]::UtcNow
            $serverState[$b] = Get-ServerState $b
        }
        $still = @($Servers | Where-Object { $serverState[$_].stale.Count })
        if ($still.Count) { Say "still stale after building: $(($still | ForEach-Object { "${_}: $($serverState[$_].stale -join '; ')" }) -join ' | ')"; exit 3 }
        $wantBuild = $false; $didBuild = $true
        Say 'build done; the servers are fresh'
    }
}

# -- is each knob in the code that reads it? -------------------------------------------------------
$emuExe = Join-Path $loc.SHADPS4 "shadPS4$Exe"
$textCache = @{}
function Get-Text([string]$path) {
    if (-not $textCache.ContainsKey($path)) {
        $textCache[$path] = if (Test-Path -LiteralPath $path -PathType Leaf) {
            [Text.Encoding]::GetEncoding(28591).GetString([IO.File]::ReadAllBytes($path)) } else { $null }
    }
    $textCache[$path]
}
function Readers([string]$section) {
    switch ($section) {
        'server' { @($Servers | ForEach-Object { $serverState[$_].path }) }
        'sidecar' { @(Get-ChildItem -LiteralPath (Join-Path $Repo 'tools/script-host/src') -Filter *.mjs -ErrorAction SilentlyContinue | ForEach-Object { $_.FullName }) }
        'emulator' { @($emuExe) }
        default { @() }
    }
}
function Test-Named([string]$text, [string]$k) {
    # Ordinal search (a regex over a 60 MB emulator binary is far too slow).
    $i = 0
    while (($i = $text.IndexOf($k, $i, [StringComparison]::Ordinal)) -ge 0) {
        $after = $i + $k.Length
        if ($after -ge $text.Length -or $text[$after] -ne '_') { return $true }
        $i = $after
    }
    $false
}
function Test-Knobs {
    foreach ($k in @($knobs.Keys)) {
        $e = $knobs[$k]
        if ($e.section -eq 'launch') { $e.state = 'launch'; continue }
        # A dry run with a build to come can't judge the servers by the stale (or absent) binaries.
        $pending = $e.section -eq 'server' -and $wantBuild
        $readers = if ($pending) { @() } else { @(Readers $e.section | Where-Object { $null -ne (Get-Text $_) }) }
        # The name, not a longer knob it begins (D1_SOCIAL is not D1_SOCIAL_INACTIVITY_MS). Literals
        # sit packed in a binary's data, so only a following `_` is taken as a longer name.
        $found = if ($readers.Count) { [bool]@($readers | Where-Object { Test-Named (Get-Text $_) $k }).Count } else { $null }
        $e.inCode = $found
        if ($e.value -eq '') { $e.state = 'unset'; continue }
        if ($null -eq $found) { $e.state = if ($pending) { 'build' } else { 'unchecked' }; continue }
        if ($found) { $e.state = 'on'; continue }
        if ($e.optional) { $e.state = 'dropped'; continue }
        $e.state = 'missing'
    }
}
Test-Knobs
function On([string]$k) { $knobs.Contains($k) -and $knobs[$k].state -in @('on', 'unchecked', 'build') -and $knobs[$k].value -notin $OffValues }
if ($wantBuild) { Warn "server knobs (state 'build') are checked against the servers once they are built; a ?knob whose code is absent is dropped then" }
foreach ($sec in 'emulator', 'sidecar') {
    if (@($knobs.Values | Where-Object { $_.section -eq $sec -and $_.state -eq 'unchecked' }).Count) { Warn "[$sec] knobs are unchecked: $((Readers $sec) -join ', ') not found" }
}

# -- inputs ---------------------------------------------------------------------------------------
$inputs = [Collections.Generic.List[object]]::new()
function Need([string]$what, [string]$path, [bool]$ok, [string]$detail = '', [switch]$Soft) {
    $inputs.Add([ordered]@{ what = $what; path = $path; ok = $ok; detail = $detail })
    if (-not $ok) { if ($Soft) { Warn "${what}: $detail $path" } else { Problem "missing ${what}: $path$(if ($detail) { " ($detail)" })" } }
}
$buildId = $null
$registryPath = Join-Path $loc.WORKSPACE 'capture-rig/builds.json'
if (Test-Path -LiteralPath $registryPath) {
    $registry = Get-Content -LiteralPath $registryPath -Raw | ConvertFrom-Json
    $buildId = if ($env:D1_BUILD) { $env:D1_BUILD } else { $registry.default }
    $entry = $registry.builds.$buildId
    if ($null -eq $entry) { Problem "the build registry has no build '$buildId'" }
    else {
        $eboot = Join-Path $loc.WORKSPACE $entry.launch
        $ok = Test-Path -LiteralPath $eboot -PathType Leaf
        $pin = if ($ok) { $i = Get-Item -LiteralPath $eboot; $i.Length -eq $entry.launch_bytes -and (Get-FileHash -LiteralPath $eboot).Hash.ToLowerInvariant() -eq $entry.launch_sha256 } else { $false }
        Need 'eboot' $eboot ($ok -and $pin) $(if (-not $ok) { "build $buildId" } elseif (-not $pin) { "not build $buildId's pinned image" } else { "build $buildId ($($entry.build_label)), pin ok" })
        $sfo = Join-Path (Split-Path $eboot) 'sce_sys/param.sfo'
        Need 'content (param.sfo beside the eboot)' $sfo (Test-Path -LiteralPath $sfo -PathType Leaf)
        $man = Join-Path $loc.WORKSPACE $entry.manifest
        $mok = Test-Path -LiteralPath $man -PathType Leaf
        $mpin = $mok -and (-not $entry.manifest_sha256 -or (Get-FileHash -LiteralPath $man).Hash.ToLowerInvariant() -eq $entry.manifest_sha256)
        Need 'served manifest' $man ($mok -and $mpin) $(if ($mok -and -not $mpin) { 'not the pinned manifest' } else { '' })
    }
} else { Need 'build registry' $registryPath $false }
Need 'emulator' $emuExe (Test-Path -LiteralPath $emuExe -PathType Leaf)
Need 'contained boot' (Join-Path $PSScriptRoot 'Start-D1ContainedBoot.ps1') (Test-Path -LiteralPath (Join-Path $PSScriptRoot 'Start-D1ContainedBoot.ps1'))
if (L 'CLIENT_PATCH') { Need 'client patch' (L 'CLIENT_PATCH') (Test-Path -LiteralPath (L 'CLIENT_PATCH') -PathType Leaf) }
# With -Only emulator the servers are an earlier run's; with a build planned they are made below.
if ($Only -ne 'emulator' -and -not $wantBuild) {
    foreach ($b in $Servers) { Need "server $b" $serverState[$b].path $serverState[$b].exists }
}

# Paths the knobs name. Under RUN or STAGING they are outputs (created as needed); else inputs.
$pathRoots = @('REPO', 'ROOT', 'TOOLS', 'WORKSPACE', 'FIXTURES', 'PROFILES', 'SHADPS4') | ForEach-Object { $loc[$_] } | Where-Object { $_ -notmatch '^<' }
foreach ($k in @($knobs.Keys)) {
    $e = $knobs[$k]
    if ($e.section -eq 'launch' -or $e.state -in @('unset', 'dropped') -or $k -eq 'D1_AH_SCRIPTS_SIDECAR') { continue }
    $v = $e.value
    if ($e.raw -match '^\$\{(RUN|BIN|STAGING)\}') {
        if ($k -eq 'D1_STORE_DIR') {
            $records = @(Get-ChildItem -LiteralPath $v -Filter 'account-*.json' -ErrorAction SilentlyContinue)
            $parent = Split-Path $v
            Need 'player store folder (D1_STORE_DIR)' $v ((Test-Path -LiteralPath $v -PathType Container) -or ($parent -and (Test-Path -LiteralPath $parent -PathType Container))) $(
                if (Test-Path -LiteralPath $v) { "exists, $($records.Count) account record(s)" }
                elseif ($parent -and (Test-Path -LiteralPath $parent)) { 'new: the store creates it and seeds it from the fixtures at the first sign-on' }
                else { "its parent $parent does not exist" })
        }
        continue
    }
    if ($v -match '^[A-Za-z]:[\\/]|^/' -or @($pathRoots | Where-Object { $v.StartsWith($_) }).Count) {
        $sideKnob = $e.section -eq 'sidecar' -and -not (On 'D1_AH_SCRIPTS')
        Need $k $v (Test-Path -LiteralPath $v) $(if ($sideKnob) { 'not needed: D1_AH_SCRIPTS is off or not in this host' } else { '' }) -Soft:$sideKnob
    }
}

# The script sidecar's prerequisites (only needed with the bridge on).
$sidecar = [ordered]@{ needed = (On 'D1_AH_SCRIPTS') }
if ($knobs.Contains('D1_AH_SCRIPTS_SIDECAR') -and $knobs['D1_AH_SCRIPTS_SIDECAR'].value) {
    $cmd = $knobs['D1_AH_SCRIPTS_SIDECAR'].value
    $sidecar.command = $cmd
    $parts = @($cmd -split '\s+' | Where-Object { $_ })
    $soft = -not $sidecar.needed
    if ($parts.Count -ge 1) {
        $nodeOk = (Test-Path -LiteralPath $parts[0] -PathType Leaf) -or [bool](Get-Command $parts[0] -ErrorAction SilentlyContinue)
        $ver = if ($nodeOk) { try { (& $parts[0] --version 2>$null) -join '' } catch { '' } } else { '' }
        $sidecar.node = $ver
        $major = if ($ver -match '^v(\d+)') { [int]$Matches[1] } else { 0 }
        Need 'node 22+ for the script sidecar' $parts[0] ($nodeOk -and $major -ge 22) $(if ($ver) { $ver } else { 'not found' }) -Soft:$soft
    }
    foreach ($p in $parts | Select-Object -Skip 1) { Need 'script sidecar' $p (Test-Path -LiteralPath $p -PathType Leaf) -Soft:$soft }
    if ($knobs.Contains('D1_TIGER_DIR') -and $knobs['D1_TIGER_DIR'].value) {
        $bs = Join-Path $knobs['D1_TIGER_DIR'].value 'packages/bullshcript/dist/versions/destiny1/v52553_16_09_28_1551_evil.js'
        Need 'built BullShcript (npm install; npm run build -w @blamnetwork/bullshcript in the tiger checkout)' $bs (Test-Path -LiteralPath $bs -PathType Leaf) -Soft:$soft
    }
    if ($knobs.Contains('D1_SCRIPTS_DIR') -and $knobs['D1_SCRIPTS_DIR'].value -and (Test-Path -LiteralPath $knobs['D1_SCRIPTS_DIR'].value)) {
        $n = @(Get-ChildItem -LiteralPath $knobs['D1_SCRIPTS_DIR'].value -Recurse -File -ErrorAction SilentlyContinue).Count
        Need 'mission scripts (files in D1_SCRIPTS_DIR)' $knobs['D1_SCRIPTS_DIR'].value ($n -gt 0) "$n file(s)" -Soft:$soft
    }
}

# Knobs that need each other.
if ((On 'D1_AH_PUBLIC_HOST') -and -not (On 'D1_AH_DISTINCT_HOST_IDS')) { Problem 'D1_AH_PUBLIC_HOST=1 needs D1_AH_DISTINCT_HOST_IDS=1 (#316)' }
if (On 'D1_AH_SCRIPTS') {
    foreach ($k in 'D1_AH_SCRIPTS_SIDECAR', 'D1_AH_SCRIPTS_ACTIVITIES') { if (-not (On $k)) { Problem "D1_AH_SCRIPTS=1 needs $k (#319)" } }
    # Stream K's private content bundle export: the sidecar's rows come from it. The sidecar reads
    # it from the environment it inherits from the activity host (tools/script-host/src/sidecar.mjs,
    # bundlesPathFor: <D1_BUNDLES_DIR>/<activity>.json); without it every row is a stand-in, which
    # the bridge never sends (integration review, finding 16: the knob is not inert).
    if (-not ($knobs.Contains('D1_BUNDLES_DIR') -and $knobs['D1_BUNDLES_DIR'].value)) {
        Problem 'D1_AH_SCRIPTS=1 needs D1_BUNDLES_DIR, the private content bundle export (stream K): set it in the environment, or -Set D1_BUNDLES_DIR=<folder>; or turn the bridge off with -Set D1_AH_SCRIPTS='
    } elseif (-not (Test-Path -LiteralPath $knobs['D1_BUNDLES_DIR'].value -PathType Container)) {
        Problem "D1_BUNDLES_DIR is not a folder: $($knobs['D1_BUNDLES_DIR'].value)"
    }
}
# A knob its reader does not name: a rename, or a stale build. Only the parts this mode starts count;
# with a build still to come (a dry run), the stale servers can't settle it.
$started = switch ($Only) { 'servers' { @('server', 'sidecar') } 'emulator' { @('emulator') } default { @('server', 'sidecar', 'emulator') } }
foreach ($k in @($knobs.Keys)) {
    $e = $knobs[$k]
    if ($e.state -ne 'missing') { continue }
    $m = "$k is not in the $($e.section) code that should read it ($((Readers $e.section | ForEach-Object { Split-Path $_ -Leaf }) -join ', ')): renamed, or a stale build?"
    if ($started -notcontains $e.section) { Warn "$m (not started with -Only $Only)" }
    elseif ($e.section -eq 'sidecar') { Warn "$m (passed on anyway: the sidecar may not read it yet)" }
    elseif ($e.section -eq 'server' -and $wantBuild) { Warn "$m (checked against the current, stale build; the build is checked again)" }
    else { Problem $m }
}

# Variables inherited from this shell that the profile does not set reach the servers and emulator.
$pathVars = @('D1_ROOT', 'D1_TOOLS', 'D1_SHADPS4', 'D1_LIBORBISPKG', 'D1_WORKSPACE', 'D1_STAGING', 'D1_PKG_DIR', 'D1_FIXTURES', 'D1_PATHS_CONFIG')
$inherited = @(Get-ChildItem env: | Where-Object { $_.Name -like 'D1_*' -and -not $knobs.Contains($_.Name) -and $pathVars -notcontains $_.Name } | ForEach-Object { "$($_.Name)=$($_.Value)" })
foreach ($i in $inherited) { Warn "inherited from this shell, not in the profile: $i" }

# -- git ------------------------------------------------------------------------------------------
$rev = try { (git -C $Repo rev-parse --short HEAD 2>$null) } catch { $null }
$dirty = try { [bool](git -C $Repo status --porcelain 2>$null) } catch { $null }

# -- the command ----------------------------------------------------------------------------------
function Q([string]$s) { if ($s -match '[\s"]') { '"' + $s.Replace('"', '\"') + '"' } else { $s } }
$boot = Join-Path $PSScriptRoot 'Start-D1ContainedBoot.ps1'
$bootArgs = @('-NoProfile', '-File', $boot, '-Name', $Name, '-LabIp', $LabIp, '-Minutes', "$Minutes", '-OutDir', (Join-Path $loc.RUN 'boot'))
if ($Only -ne 'emulator') {
    $bootArgs += @('-ActivityHost', '-ServerBin', $loc.BIN, '-ServerEnvFile', (Join-Path $loc.RUN 'server.env'))
    if (L 'ACTIVITY') { $bootArgs += @('-Activity', (L 'ACTIVITY')) }
}
if (L 'CLIENT_PATCH') { $bootArgs += @('-ClientPatch', (L 'CLIENT_PATCH')) }
if ((L 'OPERATOR_INPUT' '1') -eq '1') { $bootArgs += '-OperatorInput' }
if ($Only -eq 'servers') { $bootArgs += '-NoEmulator' }
if ($Only -eq 'emulator') { $bootArgs += '-NoServers' }
$bootLine = 'pwsh ' + (($bootArgs | ForEach-Object { Q $_ }) -join ' ')

function Lines([string]$section) {
    @(foreach ($k in $knobs.Keys) { $e = $knobs[$k]; if ($e.section -eq $section -and $e.state -notin @('dropped')) { "$k=$($e.value)" } })
}

$manifest = [ordered]@{
    schema = 1; tool = 'tools/ps4_setup/d1-full-game.ps1'; name = $Name; only = $Only; dryRun = [bool]$DryRun
    createdUtc = [DateTime]::UtcNow.ToString('o'); profile = $profilePath; chain = @($chain); overrides = @($sets | Where-Object { $_ })
    checkout = [ordered]@{ path = $Repo; revision = $rev; dirty = $dirty }
    locations = $loc; run = $loc.RUN; build = $buildId; labIp = $LabIp; minutes = $Minutes
    knobs = @(foreach ($k in $knobs.Keys) { $e = $knobs[$k]
        [ordered]@{ key = $k; section = $e.section; value = $e.value; state = $e.state; optional = $e.optional
            inCode = $(if ($e.Contains('inCode')) { $e.inCode } else { $null }); source = $e.source; from = "$($e.file):$($e.line)"; override = $e.override } })
    servers = @($Servers | ForEach-Object { $serverState[$_] }); targetDir = $TargetDir
    buildPlanned = [bool]$wantBuild; built = $didBuild; buildCommand = $(if ($wantBuild -or $didBuild) { $buildCmd } else { $null })
    emulator = $emuExe; sidecar = $sidecar; inputs = @($inputs)
    command = $bootLine; problems = @($problems); warnings = @($warnings); ready = ($problems.Count -eq 0)
}

function Show {
    "d1-full-game: profile $(Split-Path $profilePath -Leaf) ($(@($chain | ForEach-Object { Split-Path $_ -Leaf }) -join ' <- ')), run '$Name', only $Only"
    "  checkout  $Repo @ $rev$(if ($dirty) { ' (uncommitted changes)' })"
    "  run dir   $($loc.RUN)$(if ($DryRun) { ' (dry run: not created)' })"
    foreach ($section in $Sections) {
        $rows = @($knobs.Keys | Where-Object { $knobs[$_].section -eq $section })
        if (-not $rows.Count) { continue }
        ''
        "[$section]" + $(switch ($section) { 'server' { '  -> d1-responder, d1-activity-host, shadnet-server' } 'sidecar' { '  -> the activity host passes them to tools/script-host' } 'emulator' { "  -> $(Split-Path $emuExe -Leaf)" } default { '' } })
        foreach ($k in $rows) {
            $e = $knobs[$k]
            $state = if ($e.state -eq 'launch') { '' } else { $e.state }
            $v = if ($e.value.Length -gt 70) { $e.value.Substring(0, 67) + '...' } else { $e.value }
            $src = if ($e.override) { '(-Set)' } elseif ($e.source) { "# $($e.source)" } else { '' }
            if ($src.Length -gt 90) { $src = $src.Substring(0, 87) + '...' }
            '  {0,-9} {1,-28} = {2}' -f $state, $k, $v
            if ($src -and $e.state -ne 'launch') { '            {0}' -f $src }
        }
    }
    ''
    "servers (cargo target $release)"
    foreach ($b in $Servers) {
        $s = $serverState[$b]
        $what = if (-not $s.exists) { 'NOT BUILT' } elseif ($s.stale.Count) { "STALE: $($s.stale -join '; ')" } else { 'fresh' }
        '  {0,-18} {1}{2}' -f "$b$Exe", $what, $(if ($s.built) { "  (built $($s.built.Substring(0, 16)))" } else { '' })
    }
    if ($Only -eq 'emulator') { '  (not used: -Only emulator runs against servers already up)' }
    elseif ($wantBuild) { "  -> $(if ($DryRun) { 'would build' } else { 'building' }): $buildCmd  (BelowNormal priority)" }
    ''
    'inputs'
    foreach ($i in $inputs) { '  {0,-8} {1}: {2}{3}' -f $(if ($i.ok) { 'ok' } else { 'MISSING' }), $i.what, $i.path, $(if ($i.detail) { "  [$($i.detail)]" } else { '' }) }
    if ($warnings.Count) { ''; 'warnings'; foreach ($w in $warnings) { "  $w" } }
    ''
    "$(if ($DryRun) { 'would run' } else { 'running' })"
    "  $bootLine"
    ''
    if ($problems.Count) { "NOT READY: $($problems.Count) problem(s)"; foreach ($p in $problems) { "  - $p" } } else { 'READY' }
}

if ($DryRun) {
    if ($Json) { $manifest | ConvertTo-Json -Depth 8 } else { Show }
    exit 0
}

# -- for real -------------------------------------------------------------------------------------
if ($problems.Count) { Show; exit 2 }
if ($Only -ne 'emulator') {
    foreach ($b in $Servers) {
        $dest = Join-Path $loc.BIN "$b.exe"
        Copy-Item -LiteralPath $serverState[$b].path -Destination $dest
        $serverState[$b].sha256 = (Get-FileHash -LiteralPath $dest).Hash.ToLowerInvariant()
        Say "  $b.exe -> $($loc.BIN) (sha256 $($serverState[$b].sha256.Substring(0, 12)))"
    }
}
$manifest.servers = @($Servers | ForEach-Object { $serverState[$_] })
$manifest.knobs = @(foreach ($k in $knobs.Keys) { $e = $knobs[$k]
    [ordered]@{ key = $k; section = $e.section; value = $e.value; state = $e.state; optional = $e.optional; inCode = $e.inCode
        source = $e.source; from = "$($e.file):$($e.line)"; override = $e.override } })
$manifest.emulatorSha256 = (Get-FileHash -LiteralPath $emuExe).Hash.ToLowerInvariant()
$manifest.dryRun = $false
@("# server and sidecar knobs for $Name (d1-full-game.ps1); an empty value unsets the knob") + (Lines 'server') + (Lines 'sidecar') |
    Set-Content -LiteralPath (Join-Path $loc.RUN 'server.env')
@("# emulator knobs for $Name") + (Lines 'emulator') | Set-Content -LiteralPath (Join-Path $loc.RUN 'emulator.env')
$manifest | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $loc.RUN 'manifest.json') -Encoding utf8
Show | Set-Content -LiteralPath (Join-Path $loc.RUN 'plan.txt')

# The emulator inherits this process's environment through the contained boot.
foreach ($k in @($knobs.Keys)) {
    $e = $knobs[$k]
    if ($e.section -ne 'emulator' -or $e.state -eq 'dropped') { continue }
    if ($e.value -eq '') { Remove-Item "env:$k" -ErrorAction SilentlyContinue } else { Set-Item "env:$k" $e.value }
}
$bootProc = Start-Process pwsh -WindowStyle Hidden -PassThru -ArgumentList (($bootArgs | ForEach-Object { Q $_ }) -join ' ') `
    -RedirectStandardOutput (Join-Path $loc.RUN 'boot.console') -RedirectStandardError (Join-Path $loc.RUN 'boot.err')
Say "contained boot started (pid $($bootProc.Id)): $bootLine"
Say "run folder: $($loc.RUN) (manifest.json, server.env, emulator.env, boot\ with every server and emulator log)"
if ($Only -eq 'servers') { Say "servers only: create $($loc.RUN)\boot\stop to stop them (or wait $Minutes minutes)" }
