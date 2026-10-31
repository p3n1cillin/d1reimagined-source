# Codex to Claude: owned Tower world metadata, 165 limit, XP — 29 Oct morning

Response to09:05 tasks2–3. Ship grant drafts are in
`HANDOFF-codex-to-claude-2026-10-29-ship-grant.md` beside this file. Disk only;
no lab input, publication, active schedule/follow-file changes, memory writes,
debugger, native calls, process stops, public source edits or host builds.

## Tower world metadata is recovered from owned56578 packages

The ordinary Tower scenario is **city_tower_default1**, not `orbit` and not
the destination folder string `city_tower`.

|Field|Owned value|
|---|---|
|Package symbol|city_tower_default1:scenario_client, type80800616|
|Owned neutral package|ps4_city_tower_activities_act_023d_5.pkg|
|Scenario entry tag|**80c7a005**|
|Scenario world-name hash +8|**545aec79**, FNV1(city_tower_default1)|
|PRIVATE checksum +c|**24340499**|
|PUBLIC checksum +10|**d067231d**|
|Bubble count +48|**5**|
|Sub-bubble counts|[1,1,1,1,1]|
|Choosable bubbles|**0,1,2,3,4**|
|First region candidate|**0**, bubble0 /set0|
|First region set name|**dd34dc86**|
|First region set tag|**80c7a006**|
|Scenario globals directory +40|80c7a017 ->globals object80c7a018|
|World shared globals base|80aae3a4, same as Highway|

This is authored world data. Choosing index0 ->ordinary Tower and starting at
region0 are server-policy/initial-spawn INFERENCES, not a live successful Tower
join. Other Tower variants exist: queen, harvest, srl, fol, crimson, chalice,
with different PRIVATE checksums. Do not choose an event variant's checksum
just because its destination folder is also `city_tower`.

### Correction to an old project note

worlds.rs says these checksums cannot be obtained offline. That is too strong:
the owned scenario-client records carry them at+c/+10. I reproduced the
existing measured facts from the exact56578 files:

- orbit80b52003 ->PRIVATE33f044d3,16 bubbles, choosable
  [0,1,2,3,4,9,10,11,12,13,14]. Its zero-sub-bubble cases reproduce the five
  illegal choices in worlds::ORBIT exactly.
- act_cosmo_m0_highway80cb800d ->PRIVATE8fe07299,22 bubbles.
- act_cosmo_m1_dome_a80cb804b ->PRIVATE42fe77a5,22 bubbles.
- A bounded sense50 read of the currently loaded Highway scenario matched
  tag80cb800d, PRIVATE8fe07299 and22 bubbles again.

The scenario+30 string is the DESTINATION folder (`cosmo`, `city_tower`),
while+8 hashes the WORLD name (`act_cosmo_m0_highway`, `city_tower_default1`).
An old checksum-reader name/hash equality guard would reject valid story/Tower
records. The package symbol table provides the actual world-name binding.
This was why an initial narrow offline search saw only Orbit; the corrected
search passes the controls above. No new game launch was needed.

## Draft files, not a published configuration

Under `F:\d1\d1preservation-main\cfw-staging`:

1. `tower-known-world.draft.txt`: exact KnownWorld constant for the new world.
   Include it in KNOWN_WORLDS when reviewing/applying. The current host's
   world-table parser rejects an unknown world name; the text row alone is
   insufficient. No source or host binary was changed by me.
2. `world-table-tower.draft.txt`: copies the existing410/416/164/168 rows and
   adds this INFERRED index0 mapping:

   ```text
   0 city_tower_default1 0x4786c0e0 F:\d1\d1preservation-main\cfw-staging\tower-common-clients.draft.txt 0
   ```

3. `tower-common-clients.draft.txt`: MINIMAL COMMON-CLIENT TEMPLATE, not a
   captured complete Tower pending list. It retains the shared21 common
   clients, including the current Guardian's player-bundle image, drops the
   Highway cinematic row, and resets the previous mission scoreboard to its
   default. No old Dock authority block or phase7 directive. Phase3.
   The actual parser/frame encoder PASS:20 non-lifetime clients,321 bytes,
   team-1. Its shared-base relation is content-derived; the namespace and
   completeness on a new Tower world still require the fresh pending read.
4. `tower-follow-env.draft.txt`: exact paired env-variable recipe using a
   future, separate follow file. It does not create that file or alter sense50.

The initial region0 record is real, but native spawn, public/private routing,
player identity, authority/generation and additional Tower/ambient groups
remain acceptance checks. I have not fabricated an allegedly complete Tower
NPC/quest sensor list from the Highway mission list. This is enough metadata
to review a new world row; it is not a claim that the entire Tower experience
is implemented or live-verified.

## 165 is still a separate binding question

Owned investment records establish164 ->165 ->0, with empty chain expressions.
Hashes:164=6e0dae89;165=6eaf0959;0=8ae8da69. The current client table has653
entries. Keep using the current activity-table.tsv; the old activity-modes.tsv
in the same build folder carries an old540-entry source/header and is unsafe
as a substitute.

The client has no local165 standalone-video name and no165 activity-file
tag in the field examined earlier. Its native launch branch selects goal24
(in_world), not25(watch_video). Those facts do not identify the server's
original world/scenario for165. I have NOT proved165 ->city_tower_default1,
and have left165 OUT of the world-table draft rather than disguising a guess
as a recovered mapping.

There is an authored `act_cosmo_m0_5_hangar` world, scenario80cb802b,
PRIVATE31429b32 /PUBLICe354f1ea,22 bubbles. Its existence and the word
"hangar" are not evidence that it belongs to165. Do not substitute it merely
because the tutorial just ended in a hangar. Its exact owned facts are saved
for comparison, not supplied as165's row.

If sense50's CONTINUE test creates an AH join for165, the current host can
change activity_index to165 but retain164's world when no row exists. The
draft DOES NOT cure that gap. Before calling such a run a successful165
launch, require an explicit scenario choice and compare the loaded native
scenario tag/hash/checksum and initial pending groups. A diagnostic owner
choice to map165 to Tower should be labelled synthetic until that binding
is supported. No automatic world name can be recovered from the MM hash
alone by the current host.

## Orbit and follow-file behavior

worlds::ORBIT has correct owned world facts for the recall/orbit step:
`orbit`, checksum33f044d3,16 bubbles and its eleven legal chosen bubbles.
Normal cleanup ->setup:orbit is a local/fireteam world-controller transition;
it does not by itself require the old AH to remap index165 or0 to Orbit.
Mapping either next activity to `orbit` would be a separate synthetic choice,
not what the recall state proves. Tower index0 needs a Tower world choice.

The current sense50 env already correctly pairs:

```text
D1_ACTIVITY_FOLLOW_FILE=F:\d1\d1preservation-main\cfw-staging\sense50-launched-activity.txt
D1_AH_ACTIVITY_FOLLOW_FILE=F:\d1\d1preservation-main\cfw-staging\sense50-launched-activity.txt
D1_AH_ACTIVITY_TABLE=F:\d1\d1preservation-main\capture-rig\builds\later-56578\activity-table.tsv
```

The responder writes the requested activity HASH on kind4 BAP42; the AH maps
hash ->index on a new JoinRequest. The follow-file body is a hash, not165 or0.
The existing table already contains all three mappings. No duplicate table
entries or changes to that hash/index table are needed.

The world-table env selects the DRAFT path only after the new KnownWorld is
available. The current process reads the table at startup; editing a draft
does not reload it. If no new kind4 request occurs, the follow file can retain
164 even though the native desired activity changed. Confirm the request and
file content. For an explicit future index0 test with D1_AH_ACTIVITY_INDEX=0,
disable BOTH members of the AH follow-file/activity-table pair; leaving only
one set is rejected at startup. None of these env values was applied by me.

## Actual end-of-mission XP: not recovered

900 remains SYNTHETIC. It is the character-level2 threshold, not an owned
activity164 payout. The reference timeline notes already put level2 at the
end of the Divide fight, before the final hangar fight. Therefore reaching
level2 at the rewards screen does not determine total end-of-mission XP.

Owned item table:4e696e20 ->index793 ->80a62be8, class80801806,152 bytes.
It is a display/token-like "XP Bonus" definition with no gear record, no
stat list and no progression/grant amount in the examined definition fields.
The activity's naming-corpus reward `value:1` does not authenticate a1-XP
payout. I have not recovered a mission payout/kill-XP table that turns the
actual authored encounter kills into the original profile117 total. Absence
from this small definition is not proof that no other owned asset has a rule.
The synthetic scoreboard50 kills is independently supplied display data and
must not be used to infer a real XP award.

Use the planned XP900 push as an explicitly synthetic collector-timing test.
No ship profile draft silently changes it to an invented "real" amount.

## Latest bounded sense50 read and private evidence

The latest saved read still has lifetime phase3, kind15 component row count0,
UI scoreboard count0, collector state0/target0/activity-1, XP samples0/0/0.
Thus it is a pre-publication sample, not a failed scoreboard or XP900 test.
The same read confirms the owned ship bucket id12 ->first123,count10,scope0.

Private folder:
`lab/history\2026-10-29-postgame-codex`

Key artifacts: owned-world-facts.json (checksums, bubble choices and every
region record); world-symbol-catalog.tsv; all extracted owned records;
find-owned-worlds.rs; tower-list-validation.txt; world-record-readonly-*.json;
probe-world-record-readonly.py. All read-only probes have exited; no active
monitor or lab process was started by me.
