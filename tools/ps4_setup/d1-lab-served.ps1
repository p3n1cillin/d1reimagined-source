param(
    [Parameter(Mandatory)][string]$Name,
    [int]$Minutes = 90,
    # The world the Activity Host publishes (global_activity_state+0x688).
    [string]$World = 'pvp_moonbase',
    # The world's sensor-client list (d1-winguest.py sensor-clients --all --bundle).
    [string]$SensorClients = (& "$PSScriptRoot\D1Paths.ps1" Staging 'sensor-clients-pvp_moonbase-bundle7-team0.txt'),
    # The served profile (completion flags live here).
    [string]$Profile = (& "$PSScriptRoot\D1Paths.ps1" Workspace 'capture-rig\builds\later-56578\profile-geared-385-director-c.bin'),
    # Unlock-flag indices on the 503/205 pair list (<= 50). Never 308 or 3391: the Tower's own activity
    # (index 0) requires both CLEAR (notes/ps4-56578-tower-social-vendors-2026-10-30.md).
    [string]$UnlockFlags = '98,419,440,834,2668,5021,5998,6029,433,2539,2549,3558,3587',
    # The client patch (the -wizard variant lets the first-run display wizard run).
    [string]$ClientPatch = (& "$PSScriptRoot\D1Paths.ps1" Workspace 'patches\CUSA00219-56578-lab-orbit.xml'),
    # Extra NAME=VALUE server variables appended after the served set.
    [string[]]$Extra = @()
)
# The fully served 56578 lab session: geared Guardian, Director flags served (profile completions +
# D1_UNLOCK_FLAGS gates), and the Activity Host following the launched activity. No memory writes.
# Called through `powershell -File`, an array arrives as one comma-joined string.
# Split only at commas that start a new NAME=, so values may contain commas (e.g. bubble lists).
if ($Extra.Count -eq 1 -and $Extra[0] -match ',') { $Extra = $Extra[0] -split ',(?=[A-Z0-9_]+=)' }
. "$PSScriptRoot\D1Paths.ps1"
$B = Get-D1Path Workspace 'capture-rig\builds\later-56578'
$S = Get-D1Path Staging
# The pad drivers search the emulator log; old-session lines ("Entering state 'setup:orbit'") must
# not survive into this boot, or a driver presses keys while the client is still signing in.
Remove-Item (Get-D1Path ShadPS4 'user\log\shad_log.txt') -ErrorAction SilentlyContinue
$follow = "$S\$Name-launched-activity.txt"
Remove-Item $follow -ErrorAction SilentlyContinue
$envs = @(
    'D1_ANSWER_FAMILIES=0,2',
    "D1_ROSTER_DIR=$B\rosters-f0-geared",
    'D1_AH_SENSOR_PHASE=3', 'D1_AH_SENSOR_REPEAT_S=10',
    "D1_AH_SENSOR_CLIENTS=$SensorClients",
    "D1_ACTIVITY_FOLLOW_FILE=$follow", "D1_AH_ACTIVITY_FOLLOW_FILE=$follow",
    "D1_AH_ACTIVITY_TABLE=$B\activity-table.tsv",
    "D1_PROFILE_FILE=$Profile",
    "D1_UNLOCK_FLAGS=$UnlockFlags",
    "D1_INVENTORY=$B\inventory", 'D1_FAMILY42=1', 'D1_UNLOCKS=key',
    'D1_AH_TEAM0_ACTIVITIES=410,416', "D1_AH_WORLD_TABLE=$B\world-table.txt"
) + $Extra
& (Get-D1Path Tools 'Start-D1ContainedBoot.ps1') -Name $Name -LabIp 10.77.0.2 -Minutes $Minutes -OperatorInput -ActivityHost `
    -Activity $World -ClientPatch $ClientPatch `
    -ExtraServerEnv $envs *> "$S\$Name.console"
"done exit=$LASTEXITCODE"
