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

## Next

What the drop actors need in order to spawn: `dropper1` and `crawler1`–`8` (kind `0x28`), or a
spawn-point list for the squads. A contributor found that pointing a squad's `A+0x50` at a non-squad
object crashes the template getter `0x1055E0`, so whatever it names probably needs a squad-like
definition.
