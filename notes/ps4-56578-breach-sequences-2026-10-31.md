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

## Serving every Breach squad at once stalled the host (run sense98)

A probe served states for all 30 Breach squads at step 15 (one member each; `d1-squad-states.py`),
to see where each one spawns:
- Building that list took the host 32 s. The control line is built with the client's own encoder in
  the oracle, which costs seconds per squad state, and it ran on the host's tick. The four-state
  list took about 13 s in sense94.
- In those 32 s the host logged nothing, not even heartbeats. The client dropped the activity-host
  connection and relaunched A Guardian Rises from the start (11:58:09).
- Building on a thread alone was not enough (run sense99). The oracle worker runs its jobs in
  turn, so the tick's own encodes queued behind the 31-second build, and the client dropped the
  host again.
- The host now builds `clients` control lines on a thread (`ActivityInstance::poll_control`) and on
  a second oracle worker (`codec::control_oracle`, `SchemaCodec::build_sensor_aside`). Run sense100:
  the same list took 32 s to build, the client stayed connected, and all 30 squads spawned.

Where they spawned (one member each, read 5-60 s after the publish; Guardian near the rifle):

| Area | Squads | Positions |
| --- | --- | --- |
| The Breach hall, by the rifle | 21, 22, 24, 26, 28, 30, 32, 34, 36, 38 | x 423-467, y -787 to -816, z 17 |
| East of the hall | 40, 42, 56, 58, 62 | x 488-535, y -792 to -817, z 16-18 |
| The tripmine corridor (`mine-*` legs) | 44, 47, 49, 51, 53, 55, 63 | x 466-534, y -715 to -739, z 14-16 |
| West, by the junction and the garage | 64, 68, 72, 74, 76 | x 391-428, y -680 to -716, z 8-10 |
| Between the hall and the corridor | 77, 86 | (449, -778, 16), (454, -751, 20) |

Squad 94 had a member with no body position yet. None of them moved much in the minute
watched, except the hall's Dregs, which closed on the Guardian.

## The Breach by name (Blam Network's tiger, 1 November)

Blam Network's [tiger](https://github.com/Blam-Network/tiger) libraries (MIT) read build 56578
directly. `tools/ps4_setup/d1-bundle-rows.mjs` (using its `gibbon` reader) lists every row of an
activity bundle in index order, and that order is the sensor index the host serves. Group
`0xFC0E45A3` is the bundle `enc_breach_m0` (tag `0x80CB8167`, 244 rows, 141 slotted). Its 30 squads:

| Index | Squad | Index | Squad |
| --- | --- | --- | --- |
| 21 | `sq_fallen_first_vandal` | 51 | `sq_fallen_dropdown_g` |
| 22 | `sq_fallen_bouncedown_a` | 53 | `sq_fallen_wall_l_g` |
| 24 | `sq_fallen_bouncedown_c` | 55 | `sq_fallen_h` |
| 26 | `sq_fallen_wall_l_a` | 56 | `sq_fallen_1c` |
| 28 | `sq_fallen_floor_a` | 58 | `sq_fallen_2` |
| 30 | `sq_fallen_floor_b` | 62 | `sq_fallen_bridge` |
| 32 | `sq_fallen_wall_r_b` | 63 | `sq_fallen_sniper` |
| 34 | `sq_fallen_wall_l_b` | 64 | `sq_fallen_garage_obj` |
| 36 | `sq_fallen_dropdown_c` | 68 | `sq_fallen_garage_melee` |
| 38 | `sq_fallen_wall_l_c` | 72 | `sq_fallen_garage_vandals_2` |
| 40 | `sq_fallen_floor_c` | 74 | `sq_fallen_garage_vandals_3` |
| 42 | `sq_fallen_dropdown_e` | 76 | `sq_fallen_garage_exit` |
| 44 | `sq_fallen_dropdown_f` | 77 | `sq_fallen_nest1` |
| 47 | `sq_fallen_floor_f` | 86 | `sq_fallen_nest2` |
| 49 | `sq_fallen_wall_r_f` | 94 | `sq_nux_ghost` |

Player triggers are kind `0x1D`, rows 102-122. Examples: 106 `pt_fallen_vandal_spawn`, 107
`pt_fallen_3_spawn`, 108 `pt_fallen_1b_spawn`, 110 `pt_fallen_c_spawn`, 111 `pt_fallen_melee`, 112
`pt_fallen_arena_spawn`, 114-116 `pt_corr_spawn_b*`, 119 `pt_exit`, and 120
`pt_nux_ghost_light_up_start` (the trigger our schedule already arms).

Their Fukugen Daihon reconstruction of `enc_breach_m0.es` gives the waves. Each `sleep_until` on a
trigger places squads:

| Trigger | Squads placed |
| --- | --- |
| 108 `pt_fallen_1b_spawn` | 21 first vandal |
| 110 `pt_fallen_c_spawn` | 24 bouncedown c, 38 wall l c |
| 111 `pt_fallen_melee` | 42 dropdown e |
| 112 `pt_fallen_arena_spawn` | 56 1c, 58 2 (bridge rush) |
| 114 `pt_corr_spawn_b` | 44 dropdown f |
| 115 `pt_corr_spawn_b_melee` | 47 floor f, 49 wall r f |
| 116 `pt_corr_spawn_b_rear` | 51 dropdown g, 53 wall l g, then 55 h |
| 106 `pt_fallen_vandal_spawn` | 72, 74 garage vandals, 68 garage melee (and night falls) |
| 107 `pt_fallen_3_spawn` | 76 garage exit, once one of the five is left or on 119 `pt_exit` |

Nests 77 and 86 hold the perching Fallen that scatter on the Ghost's light-up (120).

What this changes:
- The route's squads 21, 26, 28 and 30 are the first vandal, wall l a, floor a and floor b.
- The "junction" wave of sense101/102 (64, 68, 76; 72 and 74 a level below) is the garage fight.
- In our runs the client reports only trigger 120, the one the schedule arms. Serving the Breach
  as written means arming its other triggers the same way and serving each trigger's squads when
  it fires.

## Serving the waves by their triggers (written, not yet run)

[`tools/ps4_setup/d1-breach-waves.py`](../tools/ps4_setup/d1-breach-waves.py) writes the Breach as
the table above places it, from the list served before the waves (`agr-b-graveyard.txt`):

- **`agr-bw-armed.txt`** arms the nine wave triggers and places the opening squads (26, 28 and 30).
- **`agr-bw-<trigger>.txt`**, one per trigger, places that trigger's squads and disarms it.
- **No refills:** squads whose wave has passed keep their generation with a desired count of 0, so
  their members stay but aren't replaced. sense101 showed that a count of 1 refills a squad behind
  the player.
- **`agr-bw-steps.txt`** holds the schedule lines (`^0xfc0e45a3/0x1d/<trigger>|300`) that go after
  the `agr-b-graveyard.txt` step. The host keeps each trigger report until a step uses it, so a
  trigger crossed early still counts.

It is opt-in: the default schedule still serves the route's lists from the replay.

**The limit:** the steps run in order, so a trigger the client never reports holds the later waves
and the Divide for its timeout.

`tests/test_breach_waves.py` checks the lists against the real base list.

## A second wave by the junction (runs sense101 and sense102)

`ending-run.ps1 -JunctionFallen` (`d1-agr-replay.py --junction-fallen`) serves a second list after
the `junction-west` leg and fights it along the garage route before the garage publish. The fight
is now one helper per wave.

- **sense101, with squads 64, 68, 72, 74 and 76** (the map's west group):
  - The Breach wave went down in six rounds. The legs after it walked normally to the junction.
  - At the junction, 64 and 68 went down. 72 and 74 stand a level below the garage route, and 76
    by its end. Fighting them, the Guardian died twice.
  - It respawned back in the tripmine corridor (about (461, -720)). The garage walk then headed
    straight at the route from there, ended on the lower level near 72 at (421, -711), and the
    replay stopped there.
  - The four Breach squads had members again by then, at their spawn spots. With a desired count
    of 1, a squad refills once its member dies. They were far behind the route.
- **Changes:**
  - The junction list now serves only 64, 68 and 76, the ones on or by the garage route.
  - When the Guardian ends a wave more than 15 m from the next route (a respawn), it re-walks the
    leg before the fight from its nearest waypoint first.
- **sense102, with both changes:**
  - The Breach wave fought through round 6; round 7 found nobody.
  - The junction wave took three rounds: one walk along the garage route and three fights. 64, 68
    and 76 went down with no deaths.
  - The garage walk then completed, and the run reached the Divide at 14:16, 35 minutes after
    launch, having fought both waves.
  - **But the Divide's squads never came.** The chain found none to fight, and the client sent 2
    squad updates after the Divide, against 677 in sense97.
  - The garage list was requested at 14:11:56. Built off the tick (#174), it was served at
    14:12:09, after mission steps 16 (14:11:33) and 17 (14:11:53, the Divide's activation list).
    It replaced step 17's frame.
  - In sense97 the synchronous publish landed before step 16.
  - Fixes: the host no longer serves a control frame that finishes after a mission step installed
    one ("... superseded by a mission step; not served"). The replay accepts that answer too, and
    waits up to 60 s for the host's answer.

## The junction wave's last squad, and the Divide on time (run sense104)

sense104 ran with both fixes from sense102 (no stale control frame, the garage list skipped once
the Divide's step has fired).

**The fan:**
- The Breach and junction waves were fought as in sense102, but the junction wave ended with squad
  76 still standing after 8 rounds, at about (405, -680) by the fan.
- The walk to the Divide then stopped at the fan rail under its fire. The resume and three nudges
  couldn't get past (16:03-16:20).
- Stopped by hand, three fight rounds on 64, 68, 72, 74 and 76 followed; the Guardian died once and
  respawned at (442, -714).
- A retrack and the fan-tunnel route then walked through, and the client reached slice set 96 at
  16:28:13.
- Fix: a walk that stalls now first fights what is left of the last wave (`skirmish`, up to three
  rounds, then a retrack), before it resumes and nudges.

**The Divide:**
- Its step (16) fired at 16:11:59 on its 2400 s timeout, 16 minutes before the Guardian got there.
  The timeout is now 5400 s.
- Even so, the Divide's squads were there this time: the garage list was skipped, so no late
  control frame replaced the activation list.
- The chain counted 12 live Divide squads and fought through the gate (squads 17-19 down).
- It went on to Dock 13 (trigger 39, slice set 104) and the hangar bay (trigger 41), whose eight
  squads were live.
- The hangar was clear at 16:47:15 (mission step 22: the cohort is exactly zero), and the outro
  steps followed (23 and 24, 16:47:56 and 16:47:59). That makes sense104 the first run that fought
  every wave served, the Breach's, the junction's, the Divide's and the hangar's, through to the
  outro.
- In sense102 the client sent 2 squad updates after the Divide.

## The Fallen's entrances: `enter()` is the member's command script (1 November)

In The Breach the wall Dreg vaults the pipes, the Vandal drops from the ceiling, Dregs drop from
above and one climbs a grate. Served as squads alone, every one of them stood up in place.

The script says what each one does. `enc_breach_m0.es` (Blam Network's reconstruction, already in
[CREDITS.md](../CREDITS.md)) defines `enter` itself:

```
def enter(actor, sequence, audio):
    __cs.clear()
    __cs.scripted_sequence(sequence)
    actor.execute(__cs)
    if audio:
        audio.start(actor)
```

The animation is the named member's command script, one `scripted_sequence` atom naming the
action. The sequence argument (`seq_fallen_vault_right` and the like) is only its audio.

In 56578 the command script sits in the member's A `0x80800868`, as `0x80800512` at `+0xB0`
(`d1-eboot descriptors --tag 0x80800868`):

| A offset | Field |
| --- | --- |
| `+0xB0` | cookie, 31 bits; a new value starts the script |
| `+0xB4` | pc |
| `+0xB8` | atom count, at most 32 |
| `+0xC0` | atoms, 64 bytes each: `+0` kind (1 = scripted_sequence), `+4` end condition (0 = its own duration), `+0x10` the action's FNV-1 hash |

Both the image and the squad image below round-trip through the client's codec
(`d1-oracle object --tag 0x80800868`: 587 bits, IDENTICAL; `--tag 0x8080071e`: 504 bits).

Two more things the native runs showed:
- The member's optional binding at `+0x50`/`+0x58` is not the sequence. Pointing it at
  `0xFC0E45A3/0x5/99` crashed the client at `0x1BDCC1`, the same unguarded lookup an all-zero
  binding hits. It stays absent.
- A squad whose state has no entry (`+0x2C` = 0, what `d1-squad-states.py` writes for count 0)
  places the client's own default, one body per member. A wall squad served that way plus its
  entered member gave two Dregs, one of them vaulting. One entry with count 0 (`+0x2C` = 1,
  `+0x30` = 0) leaves only the named actor. `d1-squad-states.py` now writes it for `SUB:GEN:=0`.

`d1-member-states.py SUB:GEN:ACTION` writes the member state with the command script.
`d1-breach-waves.py` now enters each wave's named members as the script does (`ENTRANCES`: 13
squads, 17 members) and gives their squads the explicit zero; `--no-entrances` writes the squads
alone. The three squads the script gives `@member_counts` place that many bodies (21: 1, h: 2,
garage exit: 3) instead of `--count`. It no longer places squads 26, 28 and 30 before the first trigger: the script only gives
them an objective (`enc_breach_m0.es:165-167`), and served that way they met the Guardian round
the corner before the first Vandal.

Seen in 56578 on shadPS4 (official kit), all with the published lists:
- squad 38 with member 39 `act_fallen_vault_right`: the wall Dreg vaults the pipes, alone once
  the squad carries the explicit zero;
- squad 24 with member 25 `act_fallen_intro_vandal_drop`: the Vandal drops from the ceiling;
- the bridge rush (56, 58), the mines corridor (44, 47, 49, 51, 53, 55) and the garage (68, 72,
  74): the drops, the grate climb, both vaults and the two garage Vandals play;
- the whole route by its triggers, twice, each report from 108 to 107 answered with its wave's
  list as it arrived:
  - first with hand-made lists that armed one trigger at a time. That run still placed 26/28/30
    first and activated `2__dreg_c` besides.
  - then from a fresh mission start with this generator's lists (before the `@member_counts` and
    explicit-zero spent commits): `agr-bw-armed.txt`, all nine triggers armed at once. All nine
    reported in route order with their entrances, nothing came before the first Vandal, and the
    client kept its AH connection for the whole run.

Still open:
- Squad tasks (`set_task`), which give the Fallen somewhere to go: placed with their entrance,
  they stand and shoot. The first Vandal's move to the bridge (`set_task("first_vandal")`, line
  327) is one of them.
- The mines' lasers (`o_fallen_tripwire_1`-`7`).
- Building a new client list took the host 15 to 70 s, longer for more states. Past about 20 s
  the client dropped the AH connection and reloaded the mission. Schedule steps are built when
  the host starts; a list published by control mid-run is built then. These runs used a local
  host change that is not on `main` (`D1_AH_SENSOR_CACHE`, built frames kept on disk), with the
  lists' frames built ahead in a dry host, so each list applied at once.

## The waves by their triggers, in the unattended route (runs sense105 and sense106)

`ending-run.ps1 -Waves` serves `agr-steps-divide-waves.txt`: the usual schedule with the nine wave
steps after `agr-b-graveyard.txt`, all nine triggers armed at once. The garage door is carried into
every list (`d1-breach-waves.py --overlay breach-prop3-only.txt`), so the replay publishes nothing
over the waves (`--waves`).

- **Startup:** the host builds every list at start, about 40 s for each wave list, so it was ready
  only after 16 minutes. `-Waves` gives it 30.
- **The triggers work.** Each trigger the Guardian reached was reported, and its wave was served
  and spawned. In sense105: 120 at 17:27:48, then 108, 110, 111 and 112 within six minutes. In
  sense106: 120, 108, 110 and 111.
- **One fight is not enough.** Each wave comes as its trigger is crossed, so the old single fight
  after the rifle meets only the first Vandal:
  - sense105 walked the 64-waypoint chest leg through three more waves without fighting, died at
    (468, -815), respawned at (415, -789) and lost the route;
  - in waves mode the chest leg is now walked ten waypoints at a time, with a fight after each
    stretch. A stretch that stops short (a stall, or a death the walker refuses to steer
    through) is retried after a retrack and a fight, from the nearest waypoint;
  - sense106 walked and fought all five stretches, then the tripmine stairs and the crouched
    stretch, and died in the last stretch of the corridor.
- **After a death the client reports no more wave triggers.** In sense106 the first death came at
  18:46-18:47:
  - 112 to 116 were then served only on their 300 s timeouts, though the Guardian walked through
    their places later;
  - a trigger's state (`0x80800463`) is the armed flag and an arming time, and every wave list
    armed with time 1;
  - the cause: a respawn clears the arming, and the same state served again doesn't restore it;
  - **tested in sense106:** at 19:17:01, after the deaths, wave 107's list was served by control
    with corridor triggers 114-116 armed at time 2. Walking the tripmine corridor, the client
    reported 114 (19:18:00), 115 (19:18:05) and 116 (19:18:36);
  - so the replay now re-arms after a death: it serves the current wave list (read from the host
    log) with arming time 1 + n (`--schedule-dir`).

## Next

- Squad tasks: how `set_task(name)` reaches the client (SquadAuth's task fields).
- Run `agr-bw-steps.txt` as schedule steps, all nine triggers armed at once, with the entrances.
- The nests' perching Fallen (`dropper1`, `crawler1`-`8`; squads 77 and 86) are placed at the
  encounter's start and scatter on trigger 120. Their performances animate members of squads that
  already exist, so they need those squads' states too.
