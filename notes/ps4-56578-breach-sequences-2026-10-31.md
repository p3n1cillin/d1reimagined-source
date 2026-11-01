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
  - Only the Divide's 12 squads called it (1-10 times each), with the `A + 0x4C`/`+0x50` we serve
    (`0x811C9DC5`, an empty reference) and their own empty lists. They spawned anyway, so an empty
    list is not fatal.
  - No Breach squad called the picker at all.
- So the Breach squads are never asked to make members. The question moves up a level: the
  per-frame squad update (`0x102B60` → `0x102C10`) makes members only while desired > existing +
  live, and the Breach lists serve their squads with no state image. A served desired count
  (`tools/ps4_setup/d1-squad-states.py`) needs a watch on that update and on the picker, to see
  where the Breach squads stop.

## Next

- Why the Breach squads never reach the spawn picker: watch `0x102C10` with a served desired count.
- What the drop actors need: `dropper1` and `crawler1`–`8` (kind `0x28`). Performances animate
  members of squads that already exist, so they wait on the squads.
