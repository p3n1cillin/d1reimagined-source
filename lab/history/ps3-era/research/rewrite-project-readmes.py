from pathlib import Path

root = Path(r'F:\d1\d1xenia')
data = Path(r'F:\d1\d1xenia-data')

main = '''# D1 Reimagined

An experimental Destiny 1 preservation and emulation project. The active target is
**PS3 BLES01857 01.24 (Destiny 2.3.1 / The Taken King)** in a patched RPCS3 build,
using local service stand-ins. The earlier Xbox 360/Xenia and PS3 01.00 work remains
research history. **The full game is not yet playable.**

Source and engineering notes: [d1reimagined](https://github.com/p3n1cillin/d1reimagined).
Private research inputs: [d1reimagined-data](https://github.com/p3n1cillin/d1reimagined-data).
The repositories were renamed on **2026-10-06**. Existing local checkout/runtime
folders may still have their earlier names; use explicit paths for imported runtimes.

## Current state — 2026-10-06

| Area | Verified scope | Remaining work |
| --- | --- | --- |
| Local sign-in and characters | 01.24 signs in, persists a stored character and setup answers, and joins the local activity host. | This uses local stand-ins; complete account/profile semantics and broader character coverage are incomplete. |
| Opening mission | Earlier native runs play the resurrection cinematics, hand control to the Guardian outside the Wall, show initial objectives and enter the Wall. Activity 229 uses scenario checksum `0x8fe07299`. | Reliable Breach slice handoff, objectives, rifle pickup, combat, mission completion and Tower arrival remain unverified. A world-entry state label alone proves none of these. |
| Guardian and inventory | Arrangement indexes and fixed equipment bucket ranges repair missing armour and misplaced item records. Bounded runs resolve five armour slots, Ghost and subclass icons, with normal Titan proportions in some views. | Later views still show texture corruption/distortion; level/light values and stable combined character, outdoor and mission rendering remain open. |
| Ghost | Authored member/performance bindings create one bound Ghost after the tested readiness/reset gate; helper components resolve. | The reset remains a protocol experiment. Visible torch behavior and automatic mission cues are unverified. |
| Vulkan stability | Actual-source CPU checks and bounded native comparisons support queue serialization and matching query scopes. GC and fence cleanup contracts have further CPU checks. | Descriptor-pool resets and depth/stencil layout errors remain under investigation. A longer run without device loss does not establish a graphics fix. |
| Automated evidence capture | Native game/audio recording, event deduplication, durable outbox, schema-1 packages, full decoding, hashes and automatic verified transfer have live acceptance. The capture suite passes 77 tests. | Rotation gaps and intermittent native source-identification failures remain explicit. Encounter/reward/multiplayer milestones require further reliable signals. |
| Broader content and multiplayer | Static package/scenario research and local protocol experiments provide a basis for later work. | Complete destinations, activities, AI, combat, progression, vendors, rewards, fireteams and multiplayer are not accepted. |

The [October 6 checkpoint](notes/checkpoint-2026-10-06-renderer.md) and
[Vulkan conformance record](notes/rpcs3-vulkan-conformance.md) hold the latest renderer
evidence. JOIN154 completes its bounded fixture but retains descriptor/layout errors.
JOIN155 completes without validation IDs or device loss, but diagnostic trace coverage
ends before earlier failures and the inspected outdoor scene is still distorted.
Those results do **not** accept the flicker/distortion repair. A new attachment-layout
candidate passes 31 CPU checks against 15 failures in the original control flow;
its native build and visual acceptance must be recorded separately.

The depth-only shader candidate stays disabled by default. Queue, query, GC and fence
audits have 48, 29, 14 and 71 passing CPU checks respectively; these counts describe
isolated contracts rather than whole-game acceptance. Follow
[Guardian Rises acceptance](notes/guardian-rises-acceptance.md) for the visible,
continuous checks still required. The [October 5 checkpoint](notes/checkpoint-2026-10-05-mission.md),
[appearance repair](notes/guardian-appearance-0124.md) and
[inventory repair](notes/inventory-items-0124.md) retain their bounded original evidence.

Priority is core gameplay, stability, fidelity, content coverage, multiplayer/backend
completeness, then polish. Next: validate the attachment-layout correction in the same
native scenario, correlate any remaining descriptor reset with actual submission and
completion, and obtain stable character/equipment, outdoor and mission views together.
Continue the stalled slice-transition investigation once the renderer permits it.

## Reproducing the active target

Use Windows, PowerShell 7, Python 3.14, the dependencies in `requirements.txt`, a
compatible Vulkan driver, and a privately prepared RPCS3/game runtime. Game files,
firmware, keys and patched emulator binaries are not supplied by this source checkout.
The companion data checkout is restricted research input, not a ready-to-play release.
Start with the [current runbook](notes/next-session-playbook.md) and
[01.24 profile checkpoint](notes/checkpoint-2026-10-01-profile.md).

```powershell
python -m pip install -r requirements.txt
$env:D1_LOCAL = '<absolute-private-runtime-path>'
$env:D1_DATA_REPO = '<absolute-d1reimagined-data-checkout>'

pwsh -NoProfile -File scripts/Invoke-Final124Join.ps1 `
    -Name FINAL124_UNIQUE_RUN -Local $env:D1_LOCAL -DataRepo $env:D1_DATA_REPO

# Stop only the stack owned by that exact run name.
pwsh -NoProfile -File scripts/Invoke-Final124Join.ps1 `
    -Name FINAL124_UNIQUE_RUN -Local $env:D1_LOCAL -DataRepo $env:D1_DATA_REPO -Stop
```

Every run needs a fresh name. The join wrapper leaves a successful stack running for
inspection; its `-Stop` closes the exactly owned client/services/proxy. Restore XInput
after scripted keyboard tests. `-SkipOpeningCinematic` skips a previously watched Mars
prologue at the verified frontend stage; mission revival cinematics still play.
The launchers prefer a sibling `d1reimagined-data` checkout and retain compatibility
with an existing `d1xenia-data` folder. Explicit `-DataRepo` / `D1_DATA_REPO` take precedence.

The current RPCS3 runtime is a private incremental build with additional local changes.
Individual patches and CPU audits are scoped increments, not a single self-contained
stock-RPCS3 installation recipe. Read their baseline and application notes before
applying them to a separate tree; preserve existing changes and caches. Fresh native
validation is required after a rebuild or GPU/toolchain change.

```powershell
python -m unittest discover -s server -p 'test_*.py'
python -m unittest discover -s scripts -p 'test_*.py'
python -m unittest discover -s tools/content_capture -p 'test_*.py'
```

Schema/build preflights and actual-source C++ audits are described in the runbook and
individual notes. CPU tests, static exports and successfully encoded replies do not
establish native gameplay. Use owned read-only observations and comparable input/
camera scenarios for live acceptance.

## Repository map

| Directory | Purpose |
| --- | --- |
| [server/](server/README.md) | Loopback DNS, STUN, SignOn/auth3, lobby, BAP and activity-host stand-ins, schema encoders, persistence and protocol tests. |
| `scripts/` | Run ownership, unattended input, preflights, guest-memory readers, traversal, data research and actual-source renderer audits. |
| `missions/` | Experimental mission/actor rules and [recorded traversal routes](missions/routes/README.md). |
| `patches/` | Scoped RPCS3 and historical Xenia source increments; their acceptance is documented individually. |
| `notes/` | Dated checkpoints, findings, acceptance criteria and historical test evidence. |
| [tools/content_capture/](tools/content_capture/README.md) | Asynchronous native source recording, event detection, verification, packaging, outbox and transfer. |
| [notes/devlogs/](notes/devlogs/README.md) | Evidence index and historical draft edits; current production happens on a separate machine. |

## Evidence and privacy

Normal testing automatically records relevant native game output and audio when a
private capture configuration is enabled. Meaningful observed changes produce source
packages; unchanged capabilities are deduplicated. Dimensions, distinct reported average/
base frame rates, recording gaps, truncated coverage and recovery provenance stay in
metadata. A configured recorder rate is not achieved gameplay FPS.

The development machine supplies original sources and factual engineering context.
It does not perform final editing, narration, captions, uploads or publication.
Automatic visual privacy review is unavailable, so footage requires downstream review;
audio/music/rights checks are not implied by decoding or delivery. See the
[capture contract](tools/content_capture/README.md).

Never add game images/assets, firmware, emulator builds, saves, credentials, private
certificates, raw schemas/logs, screenshots or media to this source repository. Keep
them in the ignored `local/` directory or an explicitly selected private runtime.
The private companion data repository has a different, restricted asset policy and
must not be made public or bundled into a release. Preserve private instructions and
machine provenance outside both project documentation and media releases.

## Historical work

PS3 01.00 reached local sign-in, orbit and a spawned Guardian in the first mission;
its memory/lighting work informed the 01.24 port. These earlier build-specific results
are preserved in dated notes and are not current 01.24 acceptance.

The Xbox 360/Xenia route is paused. It reached a distorted title screen and the
`canary` error; a content import patch did not establish a working online/gameplay
path. See [the original inventory](notes/inventory.md),
[network options](notes/network-options.md) and [test history](notes/test-log.md).
'''

server = '''# D1 Reimagined — local Destiny service stand-ins

These tools support the active **PS3 BLES01857 01.24 / RPCS3** research target.
Last status review: **2026-10-06**. The source repository is
[d1reimagined](https://github.com/p3n1cillin/d1reimagined); schemas and restricted
research inputs are in [d1reimagined-data](https://github.com/p3n1cillin/d1reimagined-data).

The current local flow signs in, persists a stored character/setup answers, enters the
local activity-host flow and has reached the opening mission in bounded native tests.
The services are experimental stand-ins. Schema-valid default replies, host reports
and a world-entry label do not establish playable missions or a complete backend.
Reliable slice handoff, Ghost illumination, combat, rewards, progression, complete
inventory semantics and multiplayer remain open. See the
[current project status](../README.md), [renderer checkpoint](../notes/checkpoint-2026-10-06-renderer.md)
and [mission acceptance](../notes/guardian-rises-acceptance.md).

Listeners bind only to loopback addresses. The DNS sinkhole answers locally and never
forwards upstream. Private CA material, session keys, stores, schemas and captured
requests/logs belong in `local/SIGNON_LOCAL` or the runtime selected by `D1_LOCAL`.
RPCS3's DNS configuration must select the local sinkhole. Local RPCN is an additional
sign-in dependency; preserve an existing shared instance when stopping owned tests.

| Component | Current role |
| --- | --- |
| `dns_sinkhole.py`, `hosts.py` | Local DNS and per-service loopback mappings. |
| `stun_stub.py` | Demonware NAT/IP-discovery replies used by the local startup path. |
| `signon_probe.py`, `signon_proto.py`, `auth3_proto.py` | SignOn/auth3 test handshake and encoded local session responses. |
| `sslv3.py`, `local_ca.py`, `game_ca_patch.py` | Legacy client TLS compatibility and private test trust setup; runtime patch identities retain their existing names for compatibility. |
| `lobby_probe.py` | Local Demonware lobby/session protocol experiments used by the join harness; complete online services are not implemented. |
| `bap_server.py` | BAP handshake/encrypted requests, schema-selected responses, local character store, equipment records and activity-host experiments. Many replies are defaults or mocks. |
| `activity_host_proxy.py` | Local replicated-state/baseline feed, owned heartbeat and mission-control experiments; client acceptance must be observed independently. |
| `investment.py` | Schema-driven bitstream serialization used by the selected build. |
| `prepare_rpcs3_copy.py` | Preparation of a separate private emulator copy and test configuration. |
| `run_log.py`, `session_listener.py` | Private event analysis and connection capture. |

## Run and verify

Use the PowerShell 7 launchers rather than treating a listener banner as a successful
run. Pass explicit paths for imported runtimes and a fresh name for every experiment:

```powershell
pwsh -NoProfile -File scripts/Invoke-Final124Join.ps1 `
    -Name FINAL124_UNIQUE_RUN -Local <private-runtime> -DataRepo <d1reimagined-data-checkout>
pwsh -NoProfile -File scripts/Invoke-Final124Join.ps1 `
    -Name FINAL124_UNIQUE_RUN -Local <private-runtime> -DataRepo <d1reimagined-data-checkout> -Stop
python -m unittest discover -s server -p 'test_*.py' -v
```

Commands above run from the source repository root. `D1_DATA_REPO` is an alternative
to `-DataRepo`; sibling checkout lookup prefers the new name and accepts the legacy
folder name. The [runbook](../notes/next-session-playbook.md) covers build/schema
preflight, current host arguments, read-only observations and controlled experiments.
The 01.24 intro defaults are activity 229 / checksum `0x8fe07299`; older 01.00 examples
and address tables remain historical references, not interchangeable configurations.

The [state-preserving heartbeat investigation](../notes/rpcs3-world-heartbeat.md)
records a loaded-but-inactive target slice and the missing settled handoff. Avoid
inventing transport acknowledgements or promoting a mock state transition to native
acceptance. Renderer/device-loss issues can precede that handoff and are tracked
separately. Capture runs asynchronously when configured; its failures do not fail a
game launch, and all automatic media remains private review input.
'''

routes = '''# D1 Reimagined — experimental traversal routes

`enter_the_wall.json` and `breach.json` contain recorded player positions for the
**PS3 01.24** opening-mission research target. Coordinates are `[x, y, z]`, with
`z` up. `scripts/d1_traverse.py` records/replays them using the exact owned emulator
manifest. Run from the source repository root and select the appropriate private
runtime before use:

```powershell
python scripts/d1_traverse.py --help
python scripts/d1_traverse.py --manifest <run>-processes.json record missions/routes/<new-name>.json
```

Status reviewed **2026-10-06**: earlier native tests reach initial player control,
enter the Wall and show Breach views; pitched camera handling has been improved.
These breadcrumbs are traversal experiments, not a completed mission script.
Reliable slice handoff, Ghost lighting, weapon pickup, combat, objectives and mission
completion remain unverified. Rendering corruption can invalidate a visual traversal
comparison. A replay or position change alone is not progression acceptance.

Preserve original routes and record a separate file when testing a changed scenario.
Check native camera, movement and slice state against the
[runbook](../../notes/next-session-playbook.md) and
[mission acceptance criteria](../../notes/guardian-rises-acceptance.md).
'''

devlogs = '''# D1 Reimagined — development evidence and historical drafts

Status reviewed **2026-10-06**. The development machine supplies original native
game/audio footage, screenshots and factual engineering context through the
[automatic capture/outbox system](../../tools/content_capture/README.md).
Normal operation requires no manual recording, naming, metadata, copying or retry
steps after private configuration. The separate production machine decides how to
group evidence and performs editing, narration, captions, rendering and publication.

Engineering results drive the evidence. Prefer comparable before/after tests and
continuous input-to-result demonstrations when they establish a real outcome.
Unchanged observations are deduplicated; a CPU test, bound actor or successful mock
reply does not prove visible gameplay. Keep timestamps, maturity, missing coverage,
remaining issues and privacy/audio/rights limitations explicit. Dev Blog identifiers
remain null until assigned downstream.

The current [October 6 checkpoint](../checkpoint-2026-10-06-renderer.md) records GPU
fault/conformance work and live capture verification. Guardian flicker/distortion,
combined body/world/mission rendering, Ghost illumination, combat and complete
mission progression remain open. A clean bounded validator run alone is not a fix.
The [mission acceptance checks](../guardian-rises-acceptance.md) define what footage
would need to demonstrate.

## Retained historical material

The October 5 Episode 01 drafts cover authored Ghost member/performance bindings,
a separate manual movement experiment and unfinished gameplay/rendering. Existing
review cuts use actual recorded gameplay in every section; they include reused
b-roll and synthetic draft narration and are not continuous complete playthroughs.
Their source/timecode manifests and media remain private.

The [Episode 02 inventory draft](02-2026-10-05-inventory.md) retains the tested
equipment-record/bucket repair and real before/after source footage. It is not a
renderer-stability claim or a finished release. `scripts/render_devlog.py` and its
tests remain historical tooling; they are not the current development workflow.
Previously drafted upload copy is not authorization to publish.

## Current handoff contract

Closed recordings are fully decoded and indexed with actual dimensions, distinct
reported average/base rates, audio availability, hashes and coverage gaps. Selected
sources and optional screenshot candidates enter schema-1 packages in a durable
outbox. The worker verifies every transferred payload before the atomic `.ready`
transition; completed packages are immutable and corrections require a new identity.
Earlier delivered footage can be referenced without resending it.

Native capture targets the game output. No automated visual secret or rights review
is available, so automatic footage is flagged sensitive and requiring downstream
review. Do not add private logs, machine/account identities, credentials or extracted
game assets to release material. Decoding and NAS delivery do not approve publication.
The capture README documents integration points, supported signals, retries and
the intermittent source-identification failure still under investigation.

Source: [d1reimagined](https://github.com/p3n1cillin/d1reimagined).
'''

for relative, content in [('README.md', main), ('server/README.md', server),
                          ('missions/routes/README.md', routes), ('notes/devlogs/README.md', devlogs)]:
    (root / relative).write_text(content, encoding='utf-8', newline='\n')

capture = root / 'tools/content_capture/README.md'
text = capture.read_text(encoding='utf-8')
text = text.replace('# Automatic development source capture', '# D1 Reimagined — automatic development source capture', 1)
lead = '''## Verified state — 2026-10-06

The capture suite passes **77 tests**. Live acceptance includes native game/audio
segments, interrupted-fragment recovery, durable event/outbox handling, full decoding,
verified SMB transfer and atomic ready transitions. Status-lock contention is repaired
and tested with the live worker during recording. Source-start identification still
fails intermittently in some runs; diagnostics now record shortcut results, source
timing and cleanup. Do not claim uninterrupted coverage or a complete zero-touch
solution for every failure. Recorder failure preserves sources and does not stop
engineering. New metadata uses inspected media properties, including distinct rounded
average/base rates, instead of assuming the configured frame rate was achieved.

Current detectors are described below. Mission completion, AI, rewards and multiplayer
require reliable acceptance signals before automatic milestone classification can be
added. Missing footage and unverified tests remain explicit. Normal capture/transfer
is automatic; final editorial and publication review happens on the production machine.
See the [project status](../../README.md) and
[October 6 evidence](../../notes/checkpoint-2026-10-06-renderer.md).

'''
text = text.replace('## Normal operation\n', lead + '## Normal operation\n', 1)
text = text.replace('The automatic worker runs this maintenance\nhourly while no owned game is active.', 'When enabled by the private retention policy, the worker runs this maintenance\nhourly while no owned game is active. Do not enable deletion without an agreed archive policy.')
capture.write_text(text, encoding='utf-8', newline='\n')

old = (data / 'README.md').read_text(encoding='utf-8')
old = old.replace('# d1xenia research data', '# D1 Reimagined — private research data', 1)
old = old.replace('Private companion data for [d1xenia](https://github.com/p3n1cillin/d1xenia).',
                  'Private companion data for [d1reimagined](https://github.com/p3n1cillin/d1reimagined).\nRepository: [d1reimagined-data](https://github.com/p3n1cillin/d1reimagined-data).')
old = old.replace('The source copies remain in `d1xenia/local`;',
                  'The source copies remain in a private `local/` runtime (some existing folders retain the legacy name);')
old = old.replace('Source in `d1xenia/local`', 'Source relative to the private runtime')
section = '''## Role and current project status — 2026-10-06

This repository is a curated research snapshot, not a complete installation, playable
release or mirror of the current runtime. Its files were not regenerated by the
repository rename. The active project target is PS3 BLES01857 01.24 in patched RPCS3.
Current source, fixes, acceptance criteria and engineering checkpoints live in
`d1reimagined`; the export validation below describes the scope of this data snapshot.

Local sign-in, stored characters and initial mission/world entry have bounded native
evidence. Equipment records/buckets and Ghost authored bindings have partial acceptance.
Renderer stability, character/world distortion, settled mission slice handoff, Ghost
illumination, combat, progression and mission completion remain open. Static schemas,
service maps, package counts and serializer preflights are not gameplay acceptance.
See the [latest source checkpoint](https://github.com/p3n1cillin/d1reimagined/blob/master/notes/checkpoint-2026-10-06-renderer.md).

The repository was renamed from `d1xenia-data` on 2026-10-06. Its GitHub identity,
history and private visibility are retained. Fresh checkouts should use
`d1reimagined-data`; existing local folder names remain supported by the launchers.
Pass `-DataRepo` or `D1_DATA_REPO` explicitly for an imported layout. Current private
logs/media and schema exports are not automatically copied into this repository.

**Keep this repository private.** It contains extracted game binaries/assets and
legacy logs. Access does not authorize redistribution, public hosting, release bundles
or submission of those files as devlog source media. Preserve credentials, private
instructions, machine provenance and current capture material outside this repository.

## Snapshot contents and provenance

'''
before, after = old.split('| Directory |', 1)
old = before + section + '| Directory |' + after
old = old.replace('TEST370 screenshots and schema\nexports were not present when this update was prepared; a character/inventory\nBAP capture remains pending.',
                  'TEST370 screenshots and schema\nexports were not present when this snapshot was prepared. That historical absence\ndoes not describe the current source project\'s later character/inventory tests;\nthose runtime captures remain private and are not included here.')
(data / 'README.md').write_text(old, encoding='utf-8', newline='\n')
print('Updated all six tracked READMEs across the two repositories.')
