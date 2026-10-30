# PlayStation 4 preparation helpers

The active Rust runtime source now ships under [ps4/](../../ps4/README.md).
Use explicit paths to that tree; its source fixes are already applied.

`d1-byte-scan.rs` provides nine bounded planning/scanning controls and three
unit tests. `Export-D1RepackSlack.ps1` reuses the separately installed licensed
PFS reader to export logical nonpayload ranges. All outputs stay private;
neither tool accepts a native executable. `d1-synthetic-gate.rs` generates
the public gate fixture through the existing tracer encoder, with two
positive/rejection controls and no native input. These Rust sources also
ship in their matching crates in `ps4/`.

Original Windows/WSL preparation helpers and scoped patches for the preservation
runtime. External tool installations, console inputs and game data are not
vendored here. Select your own installation with `D1_ROOT` (or the per-user
`paths.json`) and each helper's path parameters; `python d1paths.py` shows what
the defaults resolve to. See "Local paths" in [ps4/README.md](../../ps4/README.md).

The helpers support exact-input checks, content-only validation, planning and bounded server-only checks. Those results do not prove executable availability, native game boot, console isolation, Windows guest transport or gameplay. Never substitute a different executable to satisfy an identity gate, weaken containment, or label an incomplete readiness check as passed.

The package-owner and canonical-preparation patches must be applied only to their expected source revisions. Preserve unrelated changes and retain existing upstream licences. External reader modifications require their corresponding source and notices when distributed; this repository supplies no reader executable. See THIRD_PARTY_NOTICES.md.

## 56578 tutorial replay to the Divide

`d1-agr-replay.py` replaces the session-only replay copies. It uses the existing
held-key pad, verified read-only position tracker and source walker. Run only one
movement driver. The boot launcher owns cinematic input; the replay waits for
`in gameplay (112)` in that run's `boot.log`, for up to 900 seconds by default.
Every run needs its own directory, current contained-host log and fresh tracking.
The idle guard must remain active; stick movement is required for inactivity.

Before leaving the title screen, the launcher waits for port 4110 to be listening
under the contained activity-host executable started by this run. A stale listener
or an unrelated process cannot satisfy that check. This avoids starting the client
activity while the cold host is still constructing its codecs.

The launcher waits up to `-WorldTimeoutSeconds 900` for the native `activity:in_world`
milestone before sending revival skips. It exits with code 2 on that timeout or
on missing completed teleportation. A timeout does not stop the contained lab;
inspect its owned processes and native log before recovering. Raise the replay's
`--boot-timeout` too if the combined boot and world loading need more time.

First promote a **previously native-verified** full schedule and its final Divide
list in private staging. The list must already be the final `%96|900` step:

```text
python -I tools/ps4_setup/d1-agr-steps.py --promote-steps <verified-run>/steps.txt --divide-list <verified-run>/divide-gen2-objective17.txt --out-dir <private-staging>
```

This preserves all earlier timings, triggers and incident payloads and writes
stable `agr-steps-divide.txt` and `agr-divide-objective17.txt`. Different existing
outputs are refused. This is promotion, not a new native test. Build the objective
list with the existing Rust `d1-oracle sensor objective` command before testing it;
the promoter treats its contents as opaque. No generated list belongs in Git.
The launcher's default now selects this schedule. Keep `-Steps` explicit when
testing another candidate.

Launch `d1-agr-run.ps1 -Name <unique-name>` through the existing contained lab
workflow, redirecting its output to `<run>/boot.log`. Then, with that run's host
log path (its parent ends in `-<unique-name>`), run:

```text
python -u -I tools/ps4_setup/d1-agr-replay.py --run-dir <run> --host-log <contained-run>/activity-host.log --garage-clients <private-prop-3-only-list>
```

The replay stops on incomplete routes. At the rifle it uses tolerance 1.0 and
allows one retry, but continues only after the host reports trigger
`0x992bb471/0x1d/35`. A route ending near the rifle is insufficient. The garage
control uses exclusive creation and requires host acceptance; the route still
needs native inspection. The final Divide handover comes from the scheduled
slice-96 step, not a manual Divide publish.

`--resume-after-rifle` is an explicit supervised recovery from that checkpoint;
verify position, tracking and standing stance first. It refuses a run without
the rifle event. A failed crouched leg stops crouched; inspect before resuming.
Logs get new attempt names, and an exclusive `replay.lock` prevents two copies of
this replay. On abnormal termination, check owned processes before removing a
stale lock. Other input tools and the launcher do not share this lock.

The underlying route chain reached the Divide in run 17. The consolidated script
and its new failure gates have portable tests; a fresh end-to-end native replay
is still required. Reaching the Divide does not establish its crossing, combat,
correct ammunition, health, clock or staged flyover.

## Optional 56578 current-unit tracking

After the rifle movement control, `d1-agr-replay.py` now switches its remaining
walks to this mode automatically if the fresh retrack retains fewer than 30
copies. This covers the observed 11-copy chest stall. At 30 or more it keeps the
heuristic tracker. Native mode validates position and facing before route input;
failure stops the replay without retrying the sparse heuristic set. This fallback
has portable controls; a fresh lab replay is still required for acceptance.

`d1-walk.py --native-unit` reads the local player's current unit through the
guarded native player/object tables. It verifies the object's self handle and
agreement of three transform copies, and resolves the link again on each sample.
It raises an error on unavailable/inconsistent data or a changed unit handle.
Inspect the scene and start a new route after a death/respawn; there is no stale
position fallback. This does not detect every death before the unit link changes.

```text
python -u -I tools/ps4_setup/d1-walk.py --native-unit --memory-facing --route <private-route.json> --turn-sign 1 --turn-rate 2.26 --max-seconds 120
```

No scanned `--tracked` file is required. `--scan` and `--retrack` are refused in
this mode. With `--memory-facing`, heading comes from two agreeing native unit
forward vectors, rather than vectors adjacent to heuristic position copies.
The existing pad driver still sends all movement; the new reader does not attach
a debugger, call guest functions or write guest memory. The existing mode remains
the default.

The 28 October native controls verified that a forward/backward pulse moved all
three transform copies together, and right/left look input changed both forward
vectors together. Six synthetic tests cover object identity, pointer arithmetic,
unit-link races, invalid geometry, invalid transforms and heading disagreement.
These controls establish the reader, not a complete mission replay or route
geometry. Rendering/camera copies and unit transforms may differ; validate the
chosen route and tolerances before promoting this mode to an unattended replay.

The Rust `d1-oracle character weapon-stats` command can now populate a peer's
weapon investment table from its equipped owned definition. See
[56578 weapon stats](../../docs/PS4_WEAPON_STATS.md) for the native magazine result,
controlled candidate workflow and remaining limits.
