# 28 October 2026 — reaching the hangar and identifying the remaining encounter blocker

Candidate public update: The opening mission now progresses into the hangar and
presents the next combat objective. Some enemies still fail to respond, and
weapon damage remains under investigation. The mission is not yet complete.

Private production input. No publication, final-video edit or media release is
authorized by this report.

## Reporting window and provenance

Europe/London reporting day2026-10-28; engineering coverage2026-10-27T19:00:00Z
through2026-10-28T19:00:00Z. Prepared2026-10-28T19:07:16.316895+00:00. Later tests keep their real dates
and roll into the next eligible edition. This is the single consolidated report.
The earlier27Oct event and daily packages are referenced below, never modified
or resent. This report contains only three text/JSON payloads; no new media.

Inspected source checkout: 53a37e4baf64aad38fa6baf912d915be45d4074c, branchcodex/native-squad-targets, statusclean.
PR68 is recorded merged as981e0f6dc16ae2cb6ee95ad31322dfed3a03ab8b. Runtime
results span several earlier source revisions and private mission lists; this
checkout hash alone does not reproduce all tests. Runs use the owned PS4 build
56578 with the existing contained lab patches. Automatic capture-tool Git fields
describe its legacy checkout; explicit reporting_source fields identify this
engineering checkout. No stock-client/full-game acceptance is implied.

## What changed and what was actually observed

1. Weapon investment work fixed the native magazine to25 in earlier tests;
reserve quantities still vary across respawn. Diagnostic Defense1200/Attack200
tables were measured; these are not a reference-correct tutorial character.
Earlier scalar arithmetic controls exposed an offline emulator arithmetic trap,
and corrected results were retained separately. Reference Defense59 at level2
is still a validation target, not a claimed result.

2. Host refresh now runs under incoming load. Serving the world clock with a
33.333ms period makes each trigger arming report once. Claude's sense39 native
record counted exactly one incident each for triggers30,120,34,35. Earlier
zero-period runs repeatedly reported triggers; this problem is superseded in
the clock-enabled tested configuration.

3. The drop2 pilot exit program advances natively after activation. Claude's
sense40 control recorded a newly bound pilot entity and progress1→2 within
about15s. The later gate fight had fewer Skiff-turret deaths, but this was not
a controlled performance/combat-rate benchmark. A clean frame proving the
ship actually departed is still missing. Cargo/drop behaviour is not declared
complete merely from pilot queue progression.

4. The client reaches Dock13 naturally, changes authority to machine13, admits
its eight squads, and follows triggers40/41 to the authored Find the Ship and
Clear Out the Fallen objectives. The source journal records sense40 slice104
at13:54:04.59Z, trigger40 at13:54:04.99Z, and trigger41 at14:17:32Z. The route
passes north through the NUX doorway, rather than following a straight line
into a wall. Content-derived door/trigger geometry guided the later live walk.
The initial candidate coordinates were untested when delivered at14:15Z;
later successful traversal is Claude's separate live result.

5. A fail-closed explicit cohort gate requires a fresh positive admission and
then exact zero for all eight named squads in the current epoch. Timeout holds
the ending instead of counting as a clear. Private phase104→105/cinematic
drafts exist but were not established as a working ending. Cinematic cast slot
zero was corrected against native layout; a codec round trip does not prove
a completed in-game cinematic.

6. Sparse movement tracking now has a guarded native-unit fallback. A callable
native squad reader also resolves active squad members to current physical
body positions, with identity/serial/generation rechecks. It reports health as
unknown. A bounded25-sample timing check took roughly3.4–5.5ms per sample with
no unavailable sample; this is reader timing, not game performance or kills.

7. The earlier statement that the remaining squads had never spawned is not
established for the current run. At18:51:57Z a bounded sense43 snapshot found
physical bodies for squads5,6,8,11,12; squads7,9,10 had empty lists. Claude's
18:56 inspection separately reports all five remaining bodies upright and
stationary in three samples, with no engagement. Older sense40 observations
include squads8/10 reaching zero, so the failure is not proof that these
authored squads can never work. No smaller mission-clear cohort is justified.

8. Static power tracing found the Guardian's actual combat power40/40/40 and
NPC-family components1/1/1, distinct from displayed Light385/Attack200. The
owned power globals and selector/VM code matched bounded native reads at
14:41:10Z. An offline float32 replica of those native operations evaluates
the base damage multiplier to1.0 in both directions for those inputs. This
rules out a simple low multiplier on that measured path; it is not a live
bullet-hit measurement and does not rule out weapon-profile/perk modifiers.
No guessed buff or talent-grid unlock was published or tested as a fix.

## Validation — actual results, without combining unlike runs

- PASS, earlier focused host run:100 activity-host tests after strict gate work.
- PASS, earlier focused script runs:37 combined replay/walker/reader controls
  after cast-slot/fallback work; these are the recorded earlier counts.
- PASS, PR68 work:11 new synthetic squad-reader controls and6 existing native
  unit-reader controls. Native identity/code controls preceded discovery.
- PASS, selected code/global byte comparisons, current physical-binding and
  guarded reader timing observations described above.
- OBSERVED/ATTRIBUTED: Claude's clock, objective transition, pilot progress and
  upright-inert-enemy results; no independent video review by this report.
- KNOWN BROKEN: reliable encounter clearing, reference-like time to kill,
  remaining inert actors, correct tutorial stats/reserve and sprint arc pose.
- NOT RUN/NOT ESTABLISHED: full mission completion, correct outro playback,
  persistence/reconnect acceptance, full stock boot, a controlled combat
  balance comparison, or a new full media decode in this reporting pass.

Private reproducibility records include the static-combat evaluate-power-curves
and verify-damage-curves-readonly helpers, the native squad-reader synthetic
tests, private native snapshots, source notes and the dated Claude handoffs.
Raw memory, packet payloads and extracted content are not included here.

## Maturity and next acceptance

Clock-enabled trigger reporting and the explicit gate are working in the tested
scope. Dock traversal/objective progression and the pilot program are partially
working. Combat and ending remain experimental/known broken; the complete game
is far from done. Active membership is not proof of visible life or vulnerability.

Next engineering priority: compare inert members' AI/encounter state with a
member that fought, establish a native health/damage observation, and derive
the exact authored wake/behaviour transition. Acceptance: an identified member
engages, receives observed hits, dies and leaves its squad, while the explicit
eight-member gate clears only on the real encounter result. Then verify the
private phase/cinematic drafts with fresh authority and identity. Keep damage
profile and sprint-pose investigations separate from guessed stat changes.

## Evidence index, privacy and delivery

Referenced packages (27Oct, context only):
- <lab-pc>-20261027T163920-483d90b903ad47e69f966f0c5fb71f27
- <lab-pc>-20261027T190944-ff2b8032cd2f48d3b8b3b4f400293ba8

Their media is not resent and was not redecoded today. They establish the
earlier garage/Divide milestone, not today's hangar clearing or ending.
The private28Oct evidence index lists older post-cutoff fight/navigation clips
as candidates with file-modification timestamps, not verified capture times.
Those clips have not been selected, newly reviewed or transferred by this report.
No actual secrets, raw logs, game assets, screenshots, media, instruction file
or provider/desktop imagery are packaged. No new before/after video is claimed.
Audio, music, rights, privacy and editorial review remain with production before
any use of separate existing media; the report's private flag is not approval.

Delivery uses the established report command, durable outbox, verified NAS copy
and uploading→ready rename. A separate private receipt records each payload hash,
capacity, timings and remaining outbox state. The report's coverage is partial
with respect to unreviewed media, but consolidates the available engineering
facts without manufacturing visual proof. Active native combat is allowed to
finish at a safe checkpoint; this report sends no game input and stops nothing.
