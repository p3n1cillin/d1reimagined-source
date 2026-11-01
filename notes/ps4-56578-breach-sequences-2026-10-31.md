# PS4 56578: the Breach's Fallen sequences play, but no squad spawns (31 October 2026)

Runs sense82 and sense83 on the owned 56578 image, from `main` plus one staging fixture change.
Addresses are 56578 executable addresses. Owned inputs and run output stay outside the repository.
This follows a contributor's live-client finding that every Breach squad has no spawn points: each
template's inline point count (`template+0x40`) is 0, and its `+0x250` list is empty.

## The question

None of the Breach's Fallen squads (`sq_fallen_*` in group `0xFC0E45A3`) ever comes alive in our
runs. The host logs every squad's live-member count, and the first squads to come alive are always
the Divide's (`0x9243DFFB`). The Breach's entrance sequences (kind `0x5`: index 96
`seq_fallen_intro_vandal_drop`, 98/99 `seq_fallen_vault_left`/`right`, 100 `seq_breach_fallen_drop`)
are listed in our lists, but with no state image, so they never get a step. Would they spawn the
Fallen if they played?

## The test

`agr-b-graveyard.txt` (step 15, served right after the rifle pickup until the Divide) was given
sequence 96's state with the image that plays our other sequences: `0x808029A1` `ff ff ff 3f 01 00 …`
(start `0x3FFFFFFF`, step 1). Execute breakpoints (`tools/ps4_setup/d1-watchwrite.py --exec`) were
attached as step 14 fired.

## Result

- **The sequence plays.** The step change reaches the apply function `0x40CE50`, then the evaluator
  `0x40D000`. The definition's resource (`+0x68`) is valid: `0x80CB8298`, not -1. Duration (`+0x6C`)
  is 0.0 and mode (`+0x80`) is 0. The apply function then calls the play function `0x40C970`, from
  `0x40CF77`. It only does that when the evaluator returns 2, so sequence 96 started.
- **Nothing spawns.** No `0xFC0E45A3` squad came alive through the Wall. In sense83 the first squads
  to come alive were the Divide's 13, at 00:04, the same as without the sequence. Starting the
  sequence does not supply what the squads lack.
- **Step changes reach the evaluator only while the Breach objects are loaded.** In sense82 a step
  served by the control file after the Divide loaded never reached `0x40D000`.

## The evaluator, corrected

[14 October](ps4-56578-blockers-2026-10-14.md) described `0x40C8B0` as returning 2 (play), 1
(pending) or 0. In fact `0x40D000` returns 0 when the resource is -1. When the duration is above 0
and start + duration has passed, it also returns 0. Otherwise it calls `0x40C8B0`, a yes/no check.
On no it returns 0. On yes it returns 2 for mode 0, 1 for mode 2 and 0 for mode 1. The apply function
plays on 2 and marks the sequence pending (`+0x1D4`) on 1.

`0x40C8B0` walks a reference list on the definition: a count at `+0x70`, and 8-byte references from
the self-relative array at `+0x78`. A reference is `{u32 hash, u8 kind @4, u16 index @6}`, the same
triple the host's sensor lists use. The check answers yes as soon as one reference is valid
(`0x1076C0`: kind is not `0xFF` and index is not `0xFFFF`) and fails `0x40DCD0`. That function checks
by kind: kind 5 (another sequence) is handled directly, and kinds above `0x1F` go to a separate path.
Which references sequence 96 lists is not settled: the register snapshot at the end of the check
contradicted the play call that followed.

## Where a squad's spawn points come from

`0x179FB0` picks a squad's spawn points:
- `0x1058B0(squad)` returns `&A + 0x50`, where A is the squad's 0x7C-byte state `0x8080071E` (fetched
  by `0x2CFCD0(object, 0x7C)`). `A + 0x50` holds an object reference. If the reference is valid
  (`0x1076C0`) and resolves (`0x109760`), the list used is **that object's** `+0x250`. Otherwise it
  is the squad's own `+0x250`.
- `0x105840(squad)` returns the u32 at `A + 0x4C`, a hash used as the key. `0x17B6A0(list, &key,
  &flag, &1, &count)` picks from the list. When nothing is found, the key is set (not `0x811C9DC5`)
  and the squad's flag `+0x24` is non-zero, it tries again with the flag cleared.
- After that come the template's inline points (per the contributor's reading).

So whatever `A + 0x50` names must be laid out like a squad, with a list at `+0x250`. That fits the
contributor's crash: pointing it at `ps_fallen_1` faulted in the template getter `0x1055E0`. Position
sets (kind `0x2D`) are definitions with no replicated state ([private-host
investigation](private-host-investigation.md)). The Breach squads' own `+0x250` lists are empty, so
the open question is what fills a squad's runtime `+0x250` list. A static search for `+0x250` finds
mostly unrelated stack locals; the live client is the better instrument.

## Spawn points are not what stops the Breach (run sense93, 1 November)

- **What `+0x250` is.** The squad initialiser `0x177990` sets `+0x250` to the squad's own reference
  (it reads back as the squad's sensor name). It builds the spawn-point nodes from the template's
  inline points (count at template `+0x40`, 0x90-byte entries from the self-relative offset at
  `+0x48`) and keeps the node list's head at `+0x3D8`. That is list `+0x188`, which the picker
  `0x17B6A0` walks, matching each node's key at `+0x84` against `A + 0x4C`. Nodes are built only
  there (`0x175FA0` and `0x177680` have no other callers).
- **`lab/tower-route/squad-templates-readonly.py`** lists every live squad's template point count,
  node head and `+0x250` reference. With the Breach loaded (step 15), all 30 Breach squads
  (`0xFC0E45A3`) have no template points and no nodes. After the Divide loaded, 9 of its 12 squads
  (`0x9243DFFB`) had none either; only 14, 15 and 16 have one point each. All 12 had members.
- **The picker, watched** (`d1-watchwrite.py --exec 0x17A011 --exec 0x17A0BA`, from step 15
  through the Divide):
  - It was called 132 times: 49 by the Divide's squads, 23 by `0x062DD95D` (the hangar cohort) and
    60 by `0x8D3F1166`. They all had the `A + 0x4C`/`+0x50` we serve (`0x811C9DC5`, an empty
    reference), most with empty lists of their own, and they spawned anyway. An empty list is not
    fatal.
  - No Breach squad called the picker at all.
- So the Breach squads are never asked to make members, and the reason is in our lists. The
  unattended route (`lab/tower-route/ending-run.ps1`) serves the Breach squads with no state image,
  so their desired count is 0. Squad states were worked out on 26 October ([run
  `fallen2`](ps4-56578-blockers-2026-10-26.md)): the hash fields at `0x811C9DC5` plus a desired count
  make the Breach Fallen spawn and attack. The route leaves them out so that its replay can walk
  through.
- **Run sense94 confirms it with the watch.** `tools/ps4_setup/d1-squad-states.py` gave squads 21,
  26, 28 and 30 one member each (generation 2). The list was published through the activity host's
  control line right after step 15. Within seconds each of the four ran the update's spawn path
  (`0x102D40` → `0x102D69`), called the picker, and had a live member 25-70 m from the Guardian
  inside the Breach. They had no template points and empty lists.
- **Conclusion.** The Breach does not lack spawn points or a sequence state; it lacks served squad
  states at the right moments. Those are what the original server's encounter scripting sent. Note
  that a control line's path must be a Windows path (`F:/...`): the host can't open an MSYS path
  (`/f/...`).

## The Breach fight in the unattended route (runs sense96 and sense97, 1 November)

`lab/tower-route/ending-run.ps1 -BreachFallen` passes `--breach-fallen` to `d1-agr-replay.py`. After
the rifle's retrack, the replay publishes `ps4/fixtures/56578/mission/agr-b-graveyard-fallen.txt`
(the graveyard list with states for squads 21, 26, 28 and 30), then fights them before walking on.

- **sense96, fighting off the route.** The Fallen spawned. Squad 28's Dreg came to the Guardian
  (14 m), took hits (vitality 3 → 2 → dead) and died; the first Fallen killed by the unattended
  route.
  - The other three stood still at about (422, -813), (441, -814) and (452, -807). That is on the
    route itself: the walk to the chest passes (446.9, -814.8) by waypoint 13.
  - `seek.py` toward them failed at once. It walks the mission nav graph first, which does not
    cover the Breach, and `nav.py` returned exit 3.
  - `combat2.py`'s strafing then drifted the Guardian off the route, and the chest walk stalled
    against a wall at waypoint 13.
- **sense97, fighting along the route.** Each round now walks the route from the waypoint nearest
  the Guardian to the one nearest the target (`target.py`, `lab/tower-route/where.py`), then fights
  for 30 s:
  - Round 0 shot the Dreg that came close.
  - Round 1 walked to (420, -814) and fought.
  - Round 2 fought the last one from (443, -808).
  - Rounds 3 and 4 found nobody within 400 m.
  - All four squads were empty in memory 3.5 minutes after the publish. The chest walk then
    resumed from the waypoint nearest the Guardian.

## Next

- The Wall's later squads at their own moments, the same way.
- What the drop actors need: `dropper1` and `crawler1`–`8` (kind `0x28`). Performances animate
  members of squads that already exist, so they wait on the squads.
