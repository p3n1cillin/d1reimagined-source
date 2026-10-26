# 01.24 mission continuation — 2026-10-02

Target: BLES01857 01.24, Destiny 2.3.1. Continued from Claude's `d723faf`,
including the JOIN76/77 recorded routes and JOIN79 squad experiment. The two supplied
specifications are summarized in [the acceptance checks](guardian-rises-acceptance.md).
Their proposed event/API names and public tuning values are not original wire evidence.

## Verified starting point

Both resurrection cinematics run, the 113 -> 112 handover can spawn the Guardian outside
the Wall, and objectives 3 and 7 display. Recorded-route walking reached the Wall in
JOIN82. A manually changed simulation flag made the highway Ghost squad spawn its member
in JOIN79; protocol-based spawning remains to be checked. Weapons, combat, completion,
results and Tower arrival are not yet verified.

Keep the SPU colour-buffer fix and Read Color Buffers enabled. The previous broad setting
change removed menu corruption but distorted character lighting. The current executable
also contains Claude's fragment-program fixes. Do not replace it with an older backup.

## Repairs made during this continuation

- **Teleport cleanup:** both the client-report and timer completion paths now retain
  state 3 for the existing ten-second settle interval, then drop the request with a new
  world version. Previously only one group-switch path scheduled cleanup, so another
  queued teleport could stay blocked forever. Three encrypted-session regressions cover
  already-current, follow-mode and timer-only completion.
- **Objective ordering:** reactive mission steps can declare a lane and phase. A newly
  matched phase cancels older pending changes in that lane. This prevents the previous
  objective's delayed active-state update from reappearing after a fast next trigger.
  Sibling changes, other lanes and unscoped diagnostic rules retain their behavior.
- **Object names:** the entity-group reader follows each record's signed relative name
  pointer. Scanning printable strings had included binary fragments and shifted labels.
  In the Breach, the true Ghost squad is kind 1/index 94 and its member is kind 2/index 95;
  Ghost-light triggers are 0x1d/120 and 121. The old purported light trigger at 107 is a
  Fallen spawn trigger. Mission references and generated object notes are corrected.
- **Simulation experiment:** `--ah-bubble-reset SLOT[:GEN],...` sends inactive roster
  slots with changed generations while keeping the player's active slot intact. It is
  disabled by default. Disassembly links this reset/baseline path to the two simulation
  masks checked by squads; it is not proof of retail ownership semantics. See
  [the protocol note](simulation-bubble-reset.md).

## Test state and evidence

JOIN84 was already running when this continuation began. Its 113 -> 112 teleport reached
state 3 but did not report current 112, and screenshots remained black with a loading
glyph. Its exact owned stack was stopped after saving the logs. No movement was claimed.

JOIN85 tests the repaired handover, retained world-state heartbeat (`1:world`), and
inactive bubble-14 generation 1. It omits incident echo. Screenshots, process identities,
state samples, native watch output and control lines stay in the private runtime under
`SIGNON_LOCAL/logs/FINAL124_JOIN85*` and `RESEARCH/ps3_updates/FINAL124_JOIN85*`.

Repeated incidents near the JOIN81/82 Breach failure do not by themselves prove a missing
acknowledgement. JOIN83's echo experiment did not reproduce incident 2665 and produced
growing event chains. Trace the exact leave reason before changing production behavior.
The bounded native error-poster watch is `355d54 100`; its r3/r4/r5 show category, reason
and context. Keep echo opt-in and off in the next control run.

## Preservation inventory

The completed private catalog contains 315 package chains, 921 named tags, 183 scenarios,
1,339 entity groups, 338 activity definitions, 6,557 items, and 1,001 scoped localization
libraries, with zero extraction errors. The package entry index covers 1,157,737 entries
and 71,084 blocks. These are static records across planets/activities, not playable
mission coverage. Raw packages/tags remain private. All 1,339 private group metadata
files were refreshed atomically: 40,084 names corrected, zero errors, and unchanged
group identities, kinds, indexes and baselines.

## Next checks

1. Confirm the repaired handover reaches current 112 and a visible Guardian.
2. Send a new `sq_ghost` generation and observe the handler and kind-2 member report.
   Read both reset masks; if they clear immediately, trace simulation start-mode +0xc.
3. Replay the recorded route to the Wall. Finish the requested 24/hash `0x68935472`
   switch, capture the exact disconnect reason, and check connection survival while moving.
4. Test the correctly identified Breach Ghost/light objects and then the rifle pickup.
   Inventory/progression grants must become persistent and idempotent before calling any
   phase complete.

## JOIN85-87 results (2026-10-03, Claude continuing)

- **JOIN85 was not a disconnect.** Its WinError 10054 at 19:04:47 has the same timestamp as
  `FINAL124_JOIN85-rpcs3-final.log`, i.e. the stack was stopped. The repaired 113 -> 112
  handover reached current 112 and the Enter the Wall objective.
- **The 112 arrival waits for the Urgent Message dialog (JOIN87, OBSERVED).** State 4 came at
  15:06:27; the client reported current 112 only at 15:09:51, two seconds after the
  dialog was dismissed (X). JOIN84's "state 3/4 but never current 112" is probably the same
  thing. Dismiss the dialogs before waiting for the arrival report.
- **Protocol-only Ghost squad spawn works (JOIN87, OBSERVED).**
  - The `--ah-bubble-reset 14:1` start value was processed in 113 (slot 14, inactive, generation 1),
    but the transition left both simulation flags at zero, so the `sq_ghost` generation sent
    at 15:10:28 never reached the handler.
  - Sending `{"bubble_reset":"14:2"}` in 112 (15:11:09) set +0x844 bit 14 with +0x850 clear.
  - A new `sq_ghost` generation (15:11:18) then ran `0x20cc38`/`0x245204`.
  - At 15:12:31 the client's type 6 reported the squad (1/28) **and `sq_ghost__ghost` (2/29)**.
    No memory writes were involved. The member report followed about 70 s after the reset
    (baseline readiness, `0x1d8b6c`).
- Script order that works on the highway:
  1. objective;
  2. dismiss the dialogs;
  3. bubble reset with a fresh generation *after* arriving;
  4. squad generation.
- **Wrapper:** JOIN86's Cross presses came before the title accepted input. With no
  sign-in after 8 minutes, the wrapper stopped before starting the activity host, and
  logging in by hand ended in currant. If that happens, rerun rather than continuing.
- **JOIN88 (fixed script, no help):**
  - The dialogs were dismissed until the objective showed; arrival in 112 came 28 s after state 4.
  - `bubble_reset 14:2` plus a `sq_ghost` generation brought the Ghost member report in
    21 s (reproduced).
  - The walk with denser waypoints at the climb (328.6,-823.5)/(327.5,-820.4) reached the Wall
    in 3.5 min (29/29 waypoints). JOIN87's sparser list overshot into the pit beside the
    climb (z about -2, no way out).
  - The client requested 24 by itself, the host teleport went out, the client reached
    state 4 at 15:39:54 and then left at 15:41:15 with **pumpkin, cleanup-stage reason 0x4d**
    (see `errors.md`; probably `private_activity_host_timed_out_initializing_ipahp`).
  - After the leave, the client sent `ahp req` with value 1 (twice). At join it had sent value 0.
- Next: find the 01.24 "ipahp" initialization (what the client waits for after a switch
  into a private bubble), starting from the reason-0x4d writer and the `ahp req` argument.

## October 3 follow-up: distinguish failure states

The old failure-name inference above is insufficient. Final-build leave-condition 32
passes through `0x35f50c -> 0x35dd14`, and table `0x12f9d9c` maps three session statuses
(9, 37, 38) to reason `0x4d`. Sample the status before cleanup to identify the producer.
The old status numbers also shifted: final-build status 35 maps to `0x67`, whereas
the older analysis used 36. Preserve the observed leave code without treating an old
name as the exact final-build condition.

JOIN88's successful walk used the older walker directly. The guarded runner's default
had still omitted the approach and narrow-climb points. It now retains those points
from the recorded route and uses JOIN88's 1.5-unit arrival tolerance. A live guarded
walk is a separate check; stopping when slice 24 is requested is intentional.
