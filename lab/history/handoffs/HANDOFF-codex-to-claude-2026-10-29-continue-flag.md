# Codex ÃƒÂ¢Ã¢â‚¬Â Ã¢â‚¬â„¢ Claude: CONTINUE branch selector found (29 Oct afternoon)

Early result while I validate the unsent profile draft. Do not run a longer hold for this diagnosis: with the CURRENT gate a threshold-crossing hold sends **investment505 signout_character**, not normal recall.

`abd740` chooses ordinary64 only when `9545f0()` (leader predicate) is true AND `78e2a0()` is false. Live leader predicate is true. `78e2a0 -> 546f20` tests the unlock index at investment globals+192: **2838**, hash **2c39ab77**. It reads state **2** in the current plane, making that predicate true and selecting alternate65.

2838 is storage **kind3 / sub673**, i.e. profile **+53c0+673 = +5661**. The live profile byte is **2**, independently matching the plane. This flag must stop evaluating as true (plane must be non-2) for ordinary64. There is no need to change logical input route110/114: 114==37 explicitly passes the native input check and the observed edge/alternate latches prove the press is accepted.

Actual owned UI timing read: hold threshold **3000 ms** at config+374, ordinary leader recall countdown **7 seconds** at+378. Your held sample reached2937ms and did not cross the threshold. The threshold handler calls `78e2a0()` again: true ->552e10 ->505 signout; false ->955d60 explicit orbit. My earlier hand-back described the latter as though it covered all alternate holds; this corrects that overgeneralization. Neither is the intended ordinary64 countdown path.

505 is independently confirmed from sense54's live message table: hash1ce11bb6, request808017e5, response80801886. These match `signout_character`. No 505 was observed in your sub-threshold test.

I am validating a draft that changes this one completion byte and preserves the current wire profile fields. No publication, input, Computer Use or window switching performed. Full evidence/draft/acceptance steps will be appended here.

## Validated unsent drafts; exact proposed test

Primary diagnostic draft:

`F:\d1\d1preservation-main\cfw-staging\sense54-completion673-1-recall.draft.bin`

It changes **only profile byte+5661, 2Ã¢â€ â€™1**, making the direct `state==2` predicate false. The availability plane must be re-derived by the normal profile update; do not poke the plane. A value0 alternative is also available as `sense54-completion673-0-recall.draft.bin`, but use ONE controlled variant, not both in succession without reading the result. The distinction between0 and1 does not affect this equality test; I am not claiming an authored quest-state name or authentic completion award for either. Value1 is a deliberate false-state diagnostic; value0 restores an unset byte.

Both payloads: **166685 bits /20836 bytes**, owned object codec round-trip **IDENTICAL**. Each differs by exactly one byte from the authoritative canonical object. To check this against sense54 rather than assume the old fixture was still current, I captured its profile read-only and encoded/decoded it. The raw runtime object had324 non-wire bytes (first at+36c0); after codec normalization it matched your existing `profile-complete164-xp900.obj` **byte for byte**. I then changed only+5661 and verified each complete round trip. Identity, inventory/equipment, activity164, completed1, XP900 and all other wire fields are retained. Private manifest and validation log are under `2026-10-29-postgame-codex`.

Owner-operated acceptance sequence:

1. Use your existing family4/type1 profile push path with a fresh revision and the primary draft (or merge just completion sub673=1 into a newer authoritative profile). Do not replace an active schedule or startup baseline merely to push it. I have not published it.
2. Read back **profile+5661=1 AND cached availability plane[2838]!=2**. The plane address on this sense54 snapshot is `37f8bd3c8 +4 +2838 =37f8bdee2`. Do not assume that address survives a new run. If the profile changes but the plane stays2, investigate the normal invalidation/recompute before any button test.
3. Confirm selected165 remains and leader predicate remains true. Release triangle fully first so edge68 is0. Then **tap** triangle; a3-second hold is unnecessary for the ordinary path. No input was sent by Codex.
4. Look for the **persistent** recall parameter active bit / seven-second countdown, then handler+31 and normal cleanup reason0. `ordinary64` itself is set and consumed/cleared inside ONE `abd740` call, so a20-Hz sampler may miss it even on success. Countdown/departure state is the stronger acceptance evidence.
5. Expect selected165 to survive reason0 and be copied into desired activity by the orbit handler. Continue with the BAP42/follow-file/AH165/world checks in the earlier downstream hand-back. This flag draft does not supply an unproven165 world mapping.

A longer hold with the CURRENT flag2 would test sign-out, so it is not needed to solve this branch. Once flag2838 is false and the leader predicate passes, the branch is ordinary64. The alternative explicit-orbit endpoint is used only when `78e2a0` is false but ordinary leader recall was not selected (e.g. not leader); it is not the endpoint of this current flag2 path.

## Why route110=0 / route114=37 is accepted

At abd87d the client reads+114. `cmp ecx,0x25; je abd890` is an explicit bypass of the subsequent +110 equality test. Therefore37 does not need to change to0 and +110 does not need to be populated to reach the button-state machine. Your observed edge68/alternate65 latches are positive live evidence that input validation passed. Renaming these fields as current/previous route was only descriptive; their broader input-device semantics are not established by this trace.

## Keep the correction across later deltas

Your existing launch/active/completion/XP/ship fixtures inherit completion sub673=2. For a fresh run, retain whatever launch prerequisites you intentionally use, then make the end-of-mission/recall profile reflect the chosen false state for673. Every subsequent XP or ship-grant profile push must preserve that correction; replaying an older full profile with673=2 will re-arm sign-out-on-hold and suppress ordinary recall again. This explains why changing only activity completed1 did not clear the mode: completed+2c38 and completion sub673 are different fields.

## Provenance / remaining limits

This is owned-build56578 static analysis plus bounded READ+QUERY process reads and the two owner-operated input traces. The factual name `signout_character` is pinned by live message id505/hash1ce11bb6/request808017e5/response80801886; the unlock flag's own authored name has not been recovered. Earlier notes also record2838 as one of the launch-driving completion flags, consistent with it being inherited from the synthetic launch setup, but that does not by itself establish its original quest semantics.

The spinner-under-row remains a separate unresolved observation. This work proves the CONTINUE mode selector and provides an on-wire diagnostic correction; it does not claim that the spinner shares the same cause. No game input, Computer Use, window switching, process memory writes, native calls, host build/serve or source edits occurred.

## Read-only acceptance helper

```powershell
python lab/history\2026-10-29-postgame-codex\probe-recall-gate-readonly.py
```

This derives fresh pointers and prints the profile completion673 byte, derived plane[2838], leader predicate, selected activity and the predicted ordinary/alternate selector. It is a read-only check, not an input test. Before publication the expected control is byte2/plane2/ordinary=false; after the owner-operated delta require byte1/plane non-2/ordinary=true before tapping. The existing input sampler can then record the countdown and departure.

## End-of-turn live-state note,14:10 UK

A final acceptance-probe control found the required current client profile/content/session pointers **not populated**. I stopped live reads; I did not send any input or infer a state change from that. The earlier parked sense54 gate snapshots remain the evidence for byte2/plane2. The new standalone acceptance helper now prints NOT_READY and makes no branch prediction when those pointers are absent; it no longer depends on enumerating the activity table. Reuse the probe only after your chosen client reaches a fully populated profile/session state.

The drafts are explicitly sense54 snapshots. If you have restarted or changed inventory/profile state, merge **only completion sub673** into the newer authoritative profile instead of replaying a stale full sense54 profile. Preserve the separate XP target2 sequencing work.
