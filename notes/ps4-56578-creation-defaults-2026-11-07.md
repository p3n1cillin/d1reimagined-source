# Build 56578: authored new-Guardian flags

Offline owned-content investigation; no emulator or server was started. This does not establish
that the orbit popups are fixed in a native run.

## The empty record was not the game's creation state

`d1-new-guardian-defaults.py` follows investment globals `0x80A58001 +0xE8` to
`0x80A63A74` (class `0x8080190D`). Its first array has nine 176-byte records, keyed by
class and race. Each record's `+0x30` array (element `0x8080185C`) supplies flag/value pairs.
Their storage kinds and subindices come from the same build's master flag table, reached
through root `+0x538`. Both array headers/classes and all nine keys are checked. The numeric
[export](../ps4/fixtures/56578/characters/new-guardian-defaults.json) records source digests.
The executable is checked against the repository's 56578 digest before extraction.

For the served Human Titan the profile defaults are **341=2, 673=2, 693=2, 706=2, 826=2**.
The remaining creation flags belong to the account, including the class and race flags. They
must not be copied into profile field [11] just because their subindices fit there.

The profile generator now clears the later save's completions and applies the selected
identity's authored defaults before the mission's changes: the rifle grant adds 615=2; the
outro adds 40=2 and changes 673 to 1. All five boot/grant/post-game profiles receive this
treatment. The existing client codec encodes them, and decoding the output again must recover
the edited image exactly. No binary fixtures are added or replaced.

`OUT/new-guardian.env` replaces the broad `D1_UNLOCK_FLAGS` list with the existing eight-flag
**lab access policy**, plus the authored account creation flags. It omits the extra Director
and level grants, including 2539 (Level 16), 2549 (Level 25), and 3587. Profile creation flags
are deliberately absent from this repeated 205/503 override, so it cannot restore creation's
673=2 over the outro's 673=1. The lab access core is not a retail entitlement baseline.
Level-1 gear and roster cards still come from `d1-level1-fixtures.py`.

## Which fields the popups read

The [earlier native bisection](ps4-56578-new-guardian-2026-11-03.md) distinguishes **seen
markers** from **eligibility**. It does not identify each individual marker:

| Popup | Supported field mapping | Still open |
| --- | --- | --- |
| Crucible unlocked | Profile field [11], sub 912–936, contains its seen marker. | Exact sub and the target-build eligibility expression. |
| Armsday | Profile field [11], sub 673–910, contains its seen marker. | Exact sub and eligibility. |
| Age of Triumph | Same field and range as Armsday. | Exact sub and eligibility; whether this should appear on a first 2017 sign-in. |
| Destiny 2 pre-order | Same field and range as Armsday. | Exact sub and eligibility; whether this should appear on a first 2017 sign-in. |
| Venus revealed | Persists across the completion-record bisection; the separately served unlock flags affect it. The archived API's Venus book visibility expression names hashes corresponding to 56578 rows 3587 and 2549. | Confirm that visibility join against the target-build UI consumer; do not equate the API expression with a native popup observation. |

Clearing every completion previously exposed seven screens. Marking a later save's announcement
flags seen would hide them but would not establish a new Guardian's state. The generator therefore
does not pre-dismiss announcements. `--empty-record` and `--keep` remain explicitly diagnostic
stand-ins for reproducing the old experiments.

## Validation

- Five Python tests pass, including corrupt-array/duplicate-key controls and an owned-input
  regeneration comparison. Without `D1_EBOOT` and `D1_REPO`, the owned comparison skips with a reason.
- All five generated profiles encode and re-decode identically. Boot has the five authored profile
  flags; grant/mid-mission add only 615; outro adds only 40 and changes 673 to 1.
- The old oracle's `oodle controls` command could not run its 01.29 pinned-estate control because
  its compiled-in estate path is absent. It is not cited as a passing control. The root digest
  matches the existing `content-tables` export, the creation/flag array controls pass, and independent
  object/message codec controls pass on 56578. These remain offline results.

Exact individual popup consumers, retail account entitlements and native validation remain open.
