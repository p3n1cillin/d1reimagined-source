# D1 Reimagined — ordered runbook (historical recipes retained)

> **2026-10-06:** start with the [current handoff](HANDOFF-2026-10-06.md),
> [renderer checkpoint](checkpoint-2026-10-06-renderer.md) and
> [project README](../README.md). Repositories are now `d1reimagined` and
> `d1reimagined-data`; existing runtime/checkout folders remain supported. The
> attachment-layout candidate is deployed and completes JOIN157 without validation
> errors, but inspected outdoor shading still corrupts and visual acceptance is open.
> The private engineering record supplies exact runtime identities and paths.
> Older recipes below are history, not instructions to reset current work.

## Active work: 01.24 (updated 2026-10-05)

Start with the [October 5 checkpoint](checkpoint-2026-10-05-mission.md),
[October 4 checkpoint](checkpoint-2026-10-04-mission.md),
[October 3 checkpoint](checkpoint-2026-10-03-mission.md),
[October 2 run history](checkpoint-2026-10-02-mission.md), and
[acceptance checks](guardian-rises-acceptance.md). JOIN53–82 supersede the earlier black-scene
experiments: both resurrection cinematics, the outdoor Guardian, the first objective HUD,
and a recorded walk to the Wall have live evidence. JOIN97–102 clear the Breach's
249 live baselines. JOIN101/102 verify the automatic one-Ghost count after a manual
bubble reset and repeat helper attachment creation/registration. JOIN102 resolves
all twelve configured helper components and their links to the Ghost. Controlled
views toward the Ghost remain dark while the reverse corridor view is lit;
a persistent whole-view fade is not established. The walker now accepts pitched
camera directions and avoids movement probes when a valid pointer is unchanged.
Generation 1 is consumed without verifying a torch. Continue with component
activation/transform semantics and an unobstructed corridor viewpoint. Torch,
Fallen encounters and stable return-slice completion remain open. The mission is not
complete. Use intro activity **229**, checksum **0x8fe07299**, map `act_cosmo_m0_highway`;
activity 231 is level 25 and 230 reports no active activity. The JOIN12–20 lobster and
black-scene recipes below remain historical, not the next experiment to repeat.
JOIN87/88 reproduced the highway Ghost spawn with a fresh inactive bubble-14
generation after current 112, then a squad generation. For the recorded Wall route,
`scripts/d1_guardian_route.py --manifest <RUN>-processes.json --allow-breach-preload`
permits only its verified engine-3 preload while the same player remains in 112.
Queue the host teleport after reaching the actual entrance; protocol arrival from
the cars was not a playable Breach check in JOIN89. After current 24, require a
nonempty ready pool containing the Ghost squad/member/helper IDs before the fresh
inactive bubble-3 reset. The mission's delayed authored-member state then creates one
physical Ghost with both member and performance bindings; JOIN111 verifies this
without a manual member state. The old count-only Ghost action is removed to avoid
a second creation path. The filter/helper states remain manual candidates. Their generation
path reaches a default-payload update, not a verified light-on setter. See
[the light investigation](ghost-light-0124.md) and the current checkpoint.
JOIN112 repeats the automatic authored-member result and identifies the
generation callback's queried interface as a **damage component**. Its absence
on the Ghost does not establish a torch blocker; do not redirect the helper
to the Guardian because that damage interface is present there. Investigate
the attachment's actual lighting activation/transform path and the unsettled
return transition. JOIN113 tests the wrapper's bounded character-login retry
with the initial press withheld; it recovers selection and reaches matchmaking,
but the deliberately minimal host recipe later displays lobster. Use the
full current mission recipe for gameplay validation.
JOIN115/116 record actual gameplay for the revised devlogs and repeat the
one-Ghost bindings/manual movement and broken daylight Guardian appearance.
Both owned stacks are stopped. Devlog media stays private; use the
[video workflow](devlogs/README.md) rather than the superseded still-only edits.
Run the cycle with `scripts/Invoke-Final124Join.ps1`; add
`-SkipOpeningCinematic` after watching Mars once to skip that frontend
prologue on replay runs.
The join wrapper also saves a native pre-input sample before each of at most
three stored-character login retries, six seconds apart. An answered selection,
changed stage/layout or cinematic cannot pass its retry policy. Run
`pwsh -NoProfile -File scripts/Test-Final124CharacterLogin.ps1` to check the
policy without starting an emulator or sending input.
The owned keyboard profile is selected before
launch; restore XInput after stopping the run. The combined
`patches/rpcs3-destiny-spu-color-reads.patch` (01.24 copy only) retains clean
setup menus and normally proportioned inventory previews. Keep Read Color
Buffers enabled; disabling it distorted the Guardian. JOIN107 daylight emotes
show a transparent, refractive in-world Guardian. JOIN117 isolates missing
model arrangement indexes at progression gear-slot +0xc; JOIN118 supplies
them and shows solid daylight Titan armour; JOIN119 repeats that view with the
public fixture and no overlay. The standard 01.24 starter fixture now includes
the verified selections for all three classes. Check the latest
[appearance evidence and limits](guardian-appearance-0124.md) before accepting
other views: JOIN118/119 suffer Vulkan device loss around the character screen.
JOIN119 also confirms the armour thumbnails appear under Materials on the
inventory tab, while equipment icons and displayed level/light remain incomplete.
The later JOIN121–123 results below supersede this inventory-category fault. Social-space
and other appearance combinations remain unverified. JOIN120 reaches the Wall
and Breach with the public fixture, retaining five valid model roots after
handover and no recorded Vulkan fatal through those samples. The indoor emote
attempts leave the sampled view in first person, so indoor body rendering is
still unaccepted. Its owned stack is stopped and XInput restored. The
September 30 stage-19 waits were resolved by package-chain registration and
the updated qz object type hashes. Continue with
[the profile checkpoint](checkpoint-2026-10-01-profile.md), which records the
setup input sequence and the next service boundary. The later 01.00 mission
experiments in this file remain historical.

JOIN121–123 subsequently isolate and repair the inventory fault. The self
family lacked its individual item records, and the fixture packed every
item into the Materials range. The public final fixture now emits seven
item objects and uses verified fixed bucket starts. JOIN123 shows all five
armour icons, Ghost and subclass; Materials is empty. See
[inventory layouts and evidence](inventory-items-0124.md). Zero level/light,
unset weapon-slot indicators and subclass upgrades remain incomplete.
The RSX menu crash repeats after this fix, preceded by malformed live fragment
programs. Trace their runtime binding and lifetime: the existing persisted-cache
guard does not repair it. Do not infer a healthy emulator from unchanged native
world state after a fatal entry, and stop/finalize recordings before risky menu
transitions. JOIN123's interrupted recorder has no usable MP4 metadata.

JOIN124/125 repeat the repaired inventory and brief world/menu returns with
usable native video. The [leading-NOP literal correction](rpcs3-fragment-start-literals.md)
passes 12 actual-parser fixtures and is deployed, but JOIN125 still compiles
malformed live programs and rapidly creates programs. Its final pre-shutdown
sample has no fatal entry; that is not sustained-stability acceptance. A
read-only scan finds the float prefix at `0xc01f1c20` and `0xc01f3f20` amid
changing float data. Capture the bound program's original address/location,
metadata start offset and nearby bytes to establish whether it selects either
block, then trace the binding/writer. Keep the cache write guard and renderer
settings; no draw suppression is justified by this evidence.

Stop a retained run with its **exact original name**, for example
`Invoke-Final124Join.ps1 -Name FINAL124_JOIN125 -Local <runtime> -Stop`.
An unknown/shortened name now fails clearly rather than reporting success.
Shutdown detaches the debugger and closes the owned client before its backends
and proxy, so cleanup does not briefly leave an open game without its host.
Every PID still requires the saved executable and creation time. JOIN125's
reported Anteater is confirmed by the user to occur at the earlier manual
proxy-first shutdown around 17:59;
that timing does not prove all such errors are benign. Preserve the host/client
logs and classify a drop before cleanup separately.

JOIN106's generation callback fails at the first requested interface, even with
the verified helper attached. JOIN112 identifies that interface as damage,
so it is not a proven light gate. JOIN107/108's physical squad Ghost has no
authored-member or performance binding. `nux_ghost` is **kind 0x28 / 40
decimal**, index 125, not decimal 28. Its configured target is kind-2 member
95; the final member handler is `0x257aac`, not the 01.00 handler. JOIN109's
isolated state and JOIN111's automatic rule create one Ghost with both
bindings while desired counts stay zero. The automatic rule leaves its
primary cue unset. JOIN111's separate manual performance state uses
primary `+0 = 0x9b1d6a71`, `+4 = 0x811c9dc5`, generation `+8 = 1` and
reaches native dispatch, moving that same Ghost about 21 units. The
configured category comes from `0x1a2772c`, not the state. Continue with
its completion/timing and actual light controls. No
automatic performance or torch action is enabled. Do not repeat helper
generations as proof of a torch.

JOIN111 also verifies the live player's normalized gear records contain
the final Titan kit. JOIN117 identifies the missing part of that summary:
the arrangement field is absent and defaults to -1, leaving model art invalid.
Final conversion `0x46ed4c` resolves it through `0x2a6474` before world
appearance wrapper `0x720198`, builder `0x71f86c` and armour assembly
`0x71f46c`. Resolve the current player descriptor and use its reported
stride/count. The world character fields differ from the wire offsets.
Do not repeat the missing-field baseline as a shader-only experiment.

For 01.24, add `-BapExtra '--peer-family;--character-gear;starter:01.24:titan'`
to the join wrapper. This equips Striker, Lightmail, Mark of the Risen and the Generalist
Shell using final-build item indexes. The original `starter:titan` shorthand belongs to
01.00. Use `hunter` or `warlock` after `starter:01.24:` for those classes. Equipment
metadata is verified against scoped package string libraries, with starter model
arrangements independently verified against the final client asset library.
JOIN118/119 visibly verify Titan daylight armour; additional mission views remain
to be checked. Movement waits for engine state **0**, not just stage 36.

Use the imported runtime explicitly on this PC:

```powershell
$runtime = '<D1_ROOT>\main-pc-import\d1xenia\local'
$data = '<D1_ROOT>\d1xenia-data'
scripts\Invoke-Final124Run.ps1 -Name FINAL124_NEXT -Local $runtime -DataRepo $data `
  -BootSeconds 360 -PressSeconds 150 -ContinueAfterBap -KeepRpcs3
python scripts\rpcs3_final124_bootflow.py --world `
  --process-manifest "$runtime/SIGNON_LOCAL/logs/FINAL124_NEXT-processes.json" `
  --out "$runtime/RESEARCH/ps3_updates/FINAL124_NEXT-bootflow.json"
```

Use a fresh run name. The default manifest includes every required package patch
chain (837 entries). `-ContinueAfterBap` observes beyond the first connection;
`-KeepRpcs3` retains a successful stack for input, screenshots and direct reads.
Cross presses alone do not complete the cursor-based Brightness/Subtitles
choices. Follow the observed inputs in the checkpoint and verify the screen
before sending them. `Invoke-Final124Login.ps1` and `Test-ScreenReady.ps1` still
contain historical assumptions about a BETA BUILD watermark; do not use that
watermark as a 01.24 progress gate.

Before input, resolve the emulator's identity from this run's manifest:

```powershell
. scripts\Final124RunHelpers.ps1
$owned = Read-D1ProcessManifest "$runtime/SIGNON_LOCAL/logs/FINAL124_NEXT-processes.json"
$identity = $owned.processes | Where-Object tag -EQ 'rpcs3'
$emulator = Get-D1OwnedProcess $identity
if (-not $emulator) { throw 'Owned emulator has exited.' }
$env:D1_LOCAL = $runtime
$env:D1_RPCS3 = "$runtime/RPCS3_FINAL124_SMOKE001"
scripts\Invoke-RpcsStep.ps1 -ProcessId $emulator.Id -WaitSeconds 0
```

For cleanup, preserve screenshots and direct samples, then stop only the owned
processes and capture the final log:

```powershell
foreach ($process in ($owned.processes | Sort-Object { $_.tag -ne 'gdbd' })) {
  Stop-D1OwnedProcess $process
}
Copy-D1SharedFileSnapshot `
  -Source "$runtime/RPCS3_FINAL124_SMOKE001/log/RPCS3.log" `
  -Destination "$runtime/SIGNON_LOCAL/logs/FINAL124_NEXT-rpcs3-final.log"
```

Index the run with `summarize_final_smoke.py`; keep raw evidence under private
`local/`. The wrapper's protocol verdict confirms traffic, not orbit or a
completed login. Samples are sequential and not atomic. `--process-manifest`
checks executable and creation time and uses read-only Windows memory reads without
pausing; the legacy GDB path still pauses/resumes. Only stage 19's `+0x30/+0x34`
layout is interpreted as substates.

### 01.24 mission join recipe and private catalog

**Current JOIN115/116 recipe:** start in revival slice 113, use the corrected
1,455-entry private baseline superset, and retain group-switch handover.
Choose a fresh run name. Any outer launcher stdout/stderr/identity files must
use a different prefix until the public launcher has reserved the run name.

```powershell
$runtime = '<D1_ROOT>\main-pc-import\d1xenia\local'
$run = 'FINAL124_NEXT' # choose a previously unused name
$control = "$runtime/SIGNON_LOCAL/$run-control.jsonl"
New-Item -ItemType File -Path $control -ErrorAction Stop | Out-Null
$proxyOptions = '--ah-bubble-states;-1,0=0,3=0,14=0;--ah-zone;113:0x811c9dc5;--ah-world-state;113:1;--ah-roster;0;--ah-bubble-reset;14:1;--ah-sim-player;0x4786c0e0:5:0x0a3d4a2c8fbea6ba:1;--ah-game-mode;0x4786c0e0:3:3;--ah-baselines;@' + "$runtime/SIGNON_LOCAL/baselines_highway_0124_all.txt" + ';--ah-heartbeat;1;--ah-sim-clock;30;--ah-group-switch;--ah-mission;' + (Resolve-Path missions/a_guardian_rises.json).Path + ';--ah-control;' + $control
& scripts/Invoke-Final124Join.ps1 -Name $run -Local $runtime `
  -DataRepo '<D1_ROOT>\d1xenia-data' `
  -BapExtra '--peer-family;--character-gear;starter:01.24:titan' `
  -ProxyExtra $proxyOptions -AfterMatchSeconds 1200 -SkipOpeningCinematic
```

The guarded route/reset/teleport sequence above is still required for the
current Breach experiment. Neither this launch recipe nor a finished recording
establishes automatic first-mission completion. The smaller zone-112 recipe
below is historical reference, not the current gameplay test.

Use PowerShell's script invocation (`&`) for the semicolon-separated extra options.
Passing a value starting with `--` through `pwsh -File` can be parsed as another
parameter rather than as the value. Add `-CaptureTimeline` to preserve private
screenshots alongside the state samples.

```powershell
$proxyOptions = '--ah-bubble-states;-1,14=0,3=0;--ah-zone;112:0x811c9dc5;--ah-world-state;112:1;--ah-roster;0;--ah-sim-player;0x4786c0e0:5:0x0a3d4a2c8fbea6ba:1;--ah-game-mode;0x4786c0e0:3:3;--ah-baselines;@' + "$runtime/SIGNON_LOCAL/baselines_highway_0124_z112.txt" + ';--ah-heartbeat;1;--ah-sim-clock;30;--ah-mission;' + (Resolve-Path missions/a_guardian_rises.json).Path
& scripts/Invoke-Final124Join.ps1 -Name FINAL124_MISSION_NEXT -Local $runtime `
  -DataRepo $data -BapExtra '--peer-family' -ProxyExtra $proxyOptions `
  -AfterMatchSeconds 180 -CaptureTimeline
& scripts/Invoke-Final124Join.ps1 -Name FINAL124_MISSION_NEXT -Local $runtime -Stop
```

The 01.24 highway baseline file has **158** records, not the historical 138.
Current schemas come from `$data/ps3_updates/types_01.24.json` and
`services_01.24.json`. Use `starter:01.24:titan` for final gear (Generalist Shell
862, Striker 710); the original shorthand has the 01.00 indexes.

JOIN16's historical control recipe changes `--ah-bubble-states` to `0`, supplies the
then-2,311-record `baselines_highway_0124_all.txt` superset, adds
`--ah-follow-client-zone`, and omits `--ah-mission`. JOIN17 tests the follow fix
that ignores a cleared target {index=0, hash=0}. Read the profile checkpoint before
making this the default: the stable bootflow has not yet produced a playable scene.

The [October 4 checkpoint](checkpoint-2026-10-04-mission.md) supersedes this
early black-world limit: revival and outdoor walking work. Regenerate current
baseline exports with the updated `d1_pkg.py`; old catalog lists omit the helper
object's state-B bit and include unsafe AI definition kinds `0x29`, `0x2a` and
`0x2d`. The corrected private mission superset has 1,455 entries in the same 31
groups, retaining all 249 records observed at Breach entry. Do not reuse the old
2,311-entry list for a fresh handover test. The exact selected group keys remain
private, and later newly loaded records still need coverage checks.

JOIN20 narrows the advertised states to `-1,0=0,3=0,14=0`, retains final Titan
gear/follow/superset, and still reaches stage 36/engine 6. It has much lower
memory use and frames/input remain responsive beyond JOIN19's earlier freeze;
the scene stays black. Prefer this controlled recipe for the next spawn test.
`rpcs3_final124_frames.py` reads frame credits and FIFO status. Its explicit
mapping for these runs is `--fifo-mapping 0x04000000 0x34000000 0x01000000`;
samples outside that mapping are not hangs. `rpcs3_final124_records.py` takes
the observed r3 argument at native watch `1df644` as `--sim`, and checks the
real occupancy bitmap. Do not use the old contiguous-count helper for 01.24.

The short automatic walking check is state-gated and owns no new emulator:

```powershell
& scripts/Invoke-Final124Movement.ps1 -Name FINAL124_MISSION_NEXT -Local $runtime
```

```powershell
python scripts/dump_mission_catalog.py `
  "$runtime/01_WORKING/ps3_destiny/PS3_GAME/USRDIR/packages" `
  "$runtime/RESEARCH/ps3_updates/final/packages" `
  --oodle '<D1_ROOT>/main-pc-import/d1xenia/oo2core_3_win64_x86-64/oo2core_3_win64.dll' `
  --all-named-tags --investment-items `
  --out "$runtime/RESEARCH/ps3_updates/mission-catalog-0124"
python scripts/rpcs3_final124_activities.py `
  --process-manifest "$runtime/SIGNON_LOCAL/logs/FINAL124_MISSION_NEXT-processes.json" `
  --out "$runtime/RESEARCH/ps3_updates/mission-catalog-0124/activities.json"
```

Run the live activity export after the activity cache initializes. The package
catalog is static data across planets and activities, including cinematics and
shared groups; it does not establish playable mission coverage. Latest-patch
blocks missing from an update folder are resolved from the disc folder only when
length and SHA-1 match. Raw tags, captures and catalog JSON stay private.

The d1p3 executable is now deployed from the main PC's build. It logs after
scheduler admission; its hash and evidence are in the handoff/profile checkpoint.
The prior statement that it remains an uncompiled source patch is historical.

The cloud sessions of 2026-09-27 could not run the game (no `local\` there). They prepared the
experiments below and the offline tools they need. In order of cost: the offline checks take
minutes and need no emulator; each mission run takes 8-10 minutes. Background and predictions:
[`public-research.md`](public-research.md). Run everything from the repository root in `pwsh`.

## Final PS3 build export (any time, no game run needed)

`scripts\Invoke-FinalBuildExport.ps1` does the following:
1. Downloads the EBOOTs of the PS3 updates 01.13 and 01.24. It reads only those files out of
   Sony's update PKGs, so this is about 60 MB.
2. Decrypts them with RPCS3 (`rpcs3.exe --decrypt`).
3. Exports every runtime type (`d1_types.py`).
4. Writes the comparison reports 01.00 -> 01.13, 01.00 -> 01.24 and 01.13 -> 01.24
   (`d1_builddiff.py`).

```powershell
scripts\Invoke-FinalBuildExport.ps1                 # add -Versions all for every update
scripts\Invoke-FinalBuildExport.ps1 -Packages       # also the full final package set (~12 GB)
```

It needs the decrypted 01.00 EBOOT at `local\RESEARCH\ps3_decrypt\EBOOT.elf`, and for the
package steps `local\oo2core_3_win64.dll` (or `-Oodle <path>`). Push these files from
`local\RESEARCH\ps3_updates` to the data repo:
- the `.md` reports;
- `types_*.json` and `handles_*.json`;
- `updates.json` and `update-list.txt`;
- with `-Packages`, the `final_*.txt` lists.

The ELFs and packages stay private.

Done once (2026-09-28): the findings are in [`final-build.md`](final-build.md).
The data repo's `types_*.json`, `diff_*.md` and handle maps have been regenerated
with the corrected type scanner and one-to-one matcher. The 01.24 service
table is exported separately as `d1xenia-data/ps3_updates/services_01.24.json`
(94 services), with a metadata-only update package inventory beside it.

For the first 01.24 boot, build the RPCS3 copy with the 01.24 patch sites and its
calculated PPU hash (`PPU-ac58999f254e8d9dc817426c8c84b068ee8caaea`):

```powershell
& $D1Python server\prepare_rpcs3_copy.py <source> <dest> --game-ca <ca.pem> --unsigned-manifest `
    --build 01.24
```

The offline `scripts/d1_ppu_hash.py` calculation reproduces the known 01.00 hash
exactly, then derives the 01.24 key from the saved ELF using RPCS3's
[`ppu_load_exec` hashing sequence](https://github.com/RPCS3/rpcs3/blob/master/rpcs3/Emu/Cell/PPUModule.cpp).
Compare the boot log's "PPU executable hash" with this key as an extra identity check.

Before that boot, check the data without launching anything:

```powershell
scripts\Start-FinalBuildSmoke.ps1 -DataRepo ..\d1xenia-data
```

For the first actual 01.24 run, install the title updates through 01.24 in an
isolated RPCS3 copy, then build the patched copy. The readiness check requires the disc's 247 package filenames and the
1,034 non-superseded update packages at their expected sizes, plus the exact
Sony 01.24 EBOOT (SHA-256 `cd612e9b448e28e05c06d17ecc715996722fe6eea31465080a3ba1b21cc67a8c`),
both enabled patches, loopback DNS and the corrected schemas:

```powershell
$rpcs3 = '<patched RPCS3 copy containing the installed updates>'
$usrdir = '<disc PS3_GAME\USRDIR folder>'
scripts\Start-FinalBuildSmoke.ps1 -DataRepo ..\d1xenia-data `
  -Rpcs3Dir $rpcs3 -GameUsrdir $usrdir
# When every check passes, add -Run to start loopback services and the game.
```

The smoke run supplies the 01.24 type and service maps to BAP and leaves login
manual for the first observation. It records logs under `local\SIGNON_LOCAL\logs`.
`D1_LOCAL` can point to the private runtime folder; `D1_PYTHON` can point to the
Python environment with `requirements.txt` installed. The launcher uses the
chosen RPCS3 copy and disc folder rather than its 01.00 defaults. If sign-on or
package registration fails, keep the DNS, sign-on, BAP and RPCS3 logs; the
01.24 client's exact content-manifest needs still require a live check. The
current reply names 160 English/language-neutral disc package IDs; the update
inventory adds 152 IDs absent from that set. Look for missing-package or
registration messages before deciding how to include them.

## Baselines without the live dump

The disc now gives the full object list for a zone (`preservation-plan.md`, "Done since"). For
the highway start plus The Breach:

```powershell
python scripts\d1_pkg.py baselines local\01_WORKING\ps3_destiny\PS3_GAME\USRDIR\packages --oodle <dll> `
  --keys 0x992bb471,0x3c2f5ed5,0x4786c0e0,0xecf574b5,0xfc0e45a3 > local\SIGNON_LOCAL\baselines_highway_disc.txt
```

Pass it as `--ah-baselines @local\SIGNON_LOCAL\baselines_highway_disc.txt` (in `-ExtraAh`). The
live watch still runs and simply finds nothing new.

## 0. Pull and check (2 minutes)

```powershell
git pull
python -m pip install -r requirements.txt        # adds capstone for d1_eboot.py disasm
python -m unittest discover -s server -p "test_*.py"
python -m unittest discover -s scripts -p "test_*.py"
python scripts\d1_mission_preflight.py --schema local\SIGNON_LOCAL\investment_schema_full.json `
  missions\a_guardian_rises.json missions\tower_npcs.json
```

`server\test_ah_session.py` now drives the stand-in end to end (handshake, join, heartbeats,
object steps) with a fake client, so a stand-in change that breaks the join shows up here first.
For 01.24, pass its corrected `types_01.24.json` export as `--schema` instead. The mission
preflight checks every scripted state before any game launch, and `--ah-mission` repeats the
check when the stand-in starts. Both scripts passed the 01.00 and 01.24 schemas on 2026-09-28;
01.24 still needs a live behavior check.

The run scripts default to the saved 01.00 schemas. On a future 01.24 run, supply the
corrected export to both stand-ins and the 01.24 package-derived service map to BAP:

```powershell
$types = (Resolve-Path ..\d1xenia-data\ps3_updates\types_01.24.json).Path
$services = (Resolve-Path ..\d1xenia-data\ps3_updates\services_01.24.json).Path
python scripts\d1_build_preflight.py --activity-schema $types `
  --investment-schema $types --services-schema $services `
  missions\a_guardian_rises.json missions\tower_npcs.json
# Include '--schema', $types, '--services-schema', $services in -ExtraBap.
# Include '--activity-schema', $types in -ExtraAh.
```

The first 01.24 boot should establish sign-on and activity join before attempting the mission
script; the static schema check cannot verify client acceptance. The EBOOT type export has
no service ID table of its own. The separate map covers all 94 package services, including
28 added after 01.00; responses for those new services still need client behavior checks.

## 1. Offline analysis (done in the cloud session; what is left)

The cloud session ran the EBOOT and package analysis on the uploaded data (orbit checkpoint,
"Static analysis of the uploaded EBOOT" and "PS3 packages read offline"). Left for the PC:

```powershell
# the disc's package names, so manifest package ids map to names (cloud has only ten packages)
Get-ChildItem local\01_WORKING\ps3_destiny\PS3_GAME\USRDIR\packages -Name > local\package-names.txt
python scripts\d1_pkg.py named local\01_WORKING\ps3_destiny\PS3_GAME\USRDIR\packages > local\named-tags.txt
python scripts\d1_addresses.py --ghidra local\d1_labels_ghidra.py        # optional: labels for Ghidra
```

Both text files are small and hold no game data; adding them to the data repo lets the next cloud
session map the 136 bubble definitions and 81 activity infos in the manifests to packages. Tag
contents need a user-owned `oo2core_3_win64.dll` (`d1_pkg.py extract ... --oodle <dll>`): every
PS3 tag block is compressed with the 0xB7 Oodle framing.

## 2. TEST367: the slice-set switch chain, host-pushed (expected: no switch)

Ran once (test log, 367): no switch, as expected. That run used an older watch list, though, so
the hooks below did not run. Repeat it as TEST367b with this list only if TEST368 leaves
questions. `Invoke-ObjectExperiment.ps1` now always rewrites the watch file and saves the list it
used as `<Name>-watch.txt`.

Static analysis (orbit checkpoint, "how a slice-set switch starts") says a mid-mission switch
starts from the player's position (`0x31dab0`) or from a pending request (`0x31bcd0`, fed by the
record at simulation +0x5080), and ends in engine request slot 2 (`0x11f91c`). These watches cover
the whole chain; the budgets are large so none runs out (TEST366 stopped at 10 hits):

```powershell
$w = @('25715c 5000 r5:0',                 # type-11 world state still handled in the world?
       '31dab0 3000 u r3+0x89:1 r3+0x90:4', # player-bubble follower: slice state / current object
       '31d358 500 r4:0 r5:0 r6:4',         # target written: state (2 arrival, 3 player bubble, 5 request), index, hash
       '31bcd0 500 r3:0 r4:4 r5:0',         # pending request raised
       '4e9470 3000 u r5:16 r6:20',         # record at sim+0x5080 vs the spawner's copy
       '11f91c 500 r4:0 r5:12')             # engine request posted (slot 2 = change slice set)
scripts\Invoke-ObjectExperiment.ps1 -Name TEST367 -Bubble 14 `
  -ExtraAh @('--ah-bubble-states','-1,14=0,3=0','--ah-zone-schedule','24@150') -Watch $w
Select-String 'D1P' local\SIGNON_LOCAL\logs\TEST367-rpcs3.log | Select-Object -Last 60
```

Read: whether `0x25715c` keeps hitting about once a second after 5:21 (type 11 is handled in
the world); `0x31d358` with r4 = 2 at arrival and no 3/5 later; no `0x11f91c` with r4 = 2 after
the join. The 150 s zone schedule should change nothing.

## 3. TEST368: walk into The Breach (expected: the client switches by itself)

Same watches, no zone schedule, and a person walking. `Invoke-ObjectExperiment.ps1` switches to
the keyboard pad, dismisses the urgent message and takes screenshots; during its `-Seconds`
window walk in the game window with the keyboard mapping (W forward, J/L turn, Space = the
Ghost's nav mode, as in `Invoke-Explore.ps1`) through the break in the Wall towards The Breach:

```powershell
scripts\Invoke-ObjectExperiment.ps1 -Name TEST368 -Bubble 14 -Seconds 600 `
  -ExtraAh @('--ah-bubble-states','-1,14=0,3=0') -Watch $w
python scripts\ah_log_summary.py local\SIGNON_LOCAL\logs\TEST368-ahl.log
Select-String 'D1P' local\SIGNON_LOCAL\logs\TEST368-rpcs3.log | Select-Object -Last 60
Select-String 'createSession' local\SIGNON_LOCAL\logs\TEST368-lobby.log
```

The stand-in log now shows the client's own slice-set reports (`client slice set: current
112:..., target 24:..., state N`, from client type 21), so the switch is visible in
`TEST368-ahl.log` even without hooks. If the client switches and is then removed from the world
(lobster or a similar error), repeat as TEST368b with `--ah-follow-client-zone` added to
`-ExtraAh`: the stand-in then updates its world-state group record to the zone set the client
reports, as a host tracking the fireteam would.

Expected from the static analysis: at the bubble edge `0x31d358` with r4 = 3 and r5 = 24 (The
Breach's base slice set), then `0x11f91c` with r4 = 2, and the Breach loading, all without any
host action. Bubble 3 must be registered (`3=0`), or its slice set is not in the scenario object
table and the follower ignores it. Also look at what the client sends around the edge
(`ah_log_summary.py`) and whether the lobby sees a new `createSession` (public bubble
matchmaking, `public-research.md`).

## 4. TEST369: show the first objective (names from the disc, `mission-objects-highway.md`)

The first objective "Enter the Wall" is `0x992bb471/0x20/3`
(`cosmo_m0_phase00_highway.obj_m0_phase00_m1._player_objective`, state 0x80802a1b); its
sequences are `0x992bb471/0x5/4` (incoming) and `/5` (completion), its trigger
`0x992bb471/0x1d/30` (`._player_trigger`). Step the objective's state byte (field 0, 3 bits)
1, 2, 3 in the highway start and watch the HUD. The handler (`0x3561d8`) treats 3 specially, and
shows an objective only to players in its +0x2c0 mask (`mission-director.md`), so every step
sets that mask to -1 (all players):

```powershell
scripts\Invoke-ObjectExperiment.ps1 -Name TEST369 -Bubble 14 -Seconds 240 -ExtraAh @(
  '--ah-object-at','60@0x992bb471:0x20:3:0x80802a1b={"0": 1, "0x2c0": -1}',
  '--ah-object-at','120@0x992bb471:0x20:3:0x80802a1b={"0": 2, "0x2c0": -1}',
  '--ah-object-at','180@0x992bb471:0x20:3:0x80802a1b={"0": 3, "0x2c0": -1}') -Watch @('3561d8 200 r4:16')
python scripts\ah_log_summary.py local\SIGNON_LOCAL\logs\TEST369-ahl.log
```

Look for the objective text or a waypoint in the screenshots, and for a new client message
(the trigger `0x1d/30` should report when the guardian walks into `tv_in_wall` /
`obj_m0_phase00_m1._trigger_volume` once the objective is active). If JSON quoting breaks,
put the JSON in a file and use `=@file.json`.

## 5. The intro cinematic: decoded, now part of TEST371

The cinematic handler needs a changed play counter (+0x4), playing = true (+0x8), −1 at +0x14,
and the player's machine id in the cast list (+0x9c) (`mission-director.md`). TEST360-363 set
none of these. The first step of TEST371's mission script starts `pf_104_res` that way.
`new_user_experience` (`0x4786c0e0/0x27/4`) remains a separate candidate for first-mission
gating.

## 6. TEST370: the see-through guardian (gear)

The guardian renders translucent because the stand-in equips nothing: all 20 equipment slots of
the character summaries are empty (`feature-gaps.md`, section 1). The item list is now decoded
from the disc, and `--character-gear starter:titan` equips the Titan's starter "wire armor",
subclass and ghost shell. Screenshot the character select screen and the guardian in the
mission:

```powershell
scripts\Invoke-ObjectExperiment.ps1 -Name TEST370 -Bubble 14 -Seconds 120 `
  -ExtraBap @('--character-gear','starter:titan')
python scripts\rpcs3_tiger_types.py schema local\SIGNON_LOCAL\investment_schema_full.json
```

The schema export is optional now: `scripts/d1_types.py` reads the same types, and the service
table, offline.

Read the result:
- **Solid armour:** the slot order is confirmed.
- **Armour but wrong pieces or a crash:** the slot order is off. Repeat with the armour in
  other slots.
- **Still translucent:**
  - try the 118-byte definitions (`starter:titan,1=199,2=193,3=195,4=201,5=197`);
  - then a weapon (`starter:titan,6=2008`);
  - check the RPCS3 log for "gear is still loading".
- **If the created character is not a Titan:** use `starter:hunter` or `starter:warlock`.

## 7. TEST371: the mission script (cinematic, objectives, dialogue, squads)

`missions/a_guardian_rises.json` plays the start of the first mission from the stand-in: the
revival cinematic, "Enter the Wall" with its dialogue, then the next objectives as the client
reports each trigger, and three Fallen squads once the client is in The Breach. Background and
field meanings: [`mission-director.md`](mission-director.md). Play along: after the objective
shows, walk to the Wall and on into The Breach (TEST368's walk).

```powershell
scripts\Invoke-ObjectExperiment.ps1 -Name TEST371 -Bubble 14 -Seconds 600 `
  -ExtraBap @('--character-gear','starter:titan') `
  -ExtraAh @('--ah-bubble-states','-1,14=0,3=0','--ah-sim-clock','30',
             '--ah-mission',(Resolve-Path missions\a_guardian_rises.json).Path) `
  -Watch @('4ed278 50 r4:48','4ecd08 20','b94414 100 r4:16','3561d8 100 r4:16','1f8d20 100 r4:16')
python scripts\ah_log_summary.py local\SIGNON_LOCAL\logs\TEST371-ahl.log
Select-String 'D1P|mission|--ah-on' local\SIGNON_LOCAL\logs\TEST371-rpcs3.log, local\SIGNON_LOCAL\logs\TEST371-ahl.log |
  Select-Object -Last 80
```

What to look for:
- **At +20 s, the revival cinematic.** `0x4ecd08` is hit when it starts. If the handler runs
  but `0x4ecd08` is never hit, one of its checks refused.
- **At +140 s, "Enter the Wall"** on the HUD with the Ghost's line (`0xb94414` hits). If the
  handler runs but nothing shows, repeat with objective state 2 or 3 in place of 1.
- **At the Wall,** the stand-in log shows "--ah-on rule ... matched type 6" and the next
  objective. If no rule matches, save the type-6 bodies from the log: they show how the client
  reports a trigger.
- **In The Breach,** Fallen appear 20 s after the client reports slice set 24 (`0x1f8d20` hits).
- The HUD/screen timeline from the screenshots, against the reference video.

The stand-in also writes a structured log per connection, `events.jsonl` in its capture
folder (`local\SIGNON_LOCAL\captures\bap\<time>\`). It holds every client frame and host
message with its full body, plus every log line. Push the activity connection's file with the
logs. It answers the trigger question offline:

```powershell
$ev = (Get-ChildItem local\SIGNON_LOCAL\captures\bap\*\events.jsonl | Sort-Object Length)[-1].FullName
python server\run_log.py summary $ev                    # message counts per type, rules fired
python server\run_log.py ah $ev --type 6 --distinct     # each distinct type-6 body once
python server\run_log.py bits $ev --type 6              # which header bits vary
```

## 8. TEST372: the Tower (after TEST371 shows whether squads spawn)

Pick the Tower in orbit and join with the Tower's values; the vendors and NPCs are placed by
`missions/tower_npcs.json`. The arguments and the bubble to try are in [`tower.md`](tower.md).
The world id comes from the client's service 105 in that run.

## 9. RPCS3 housekeeping (optional, 20 minutes)

The patches apply cleanly to upstream master `3fa07db78` (`rpcs3-upstream-draft.md`). A rebuild
on master picks up the PPU thread-start fixes and the boot shader-compilation change. The
login-cinematic freeze A/B (09:30 patch vs current, game files in the same place) needs about
10 boots per side to mean anything at a 40 % freeze rate.
