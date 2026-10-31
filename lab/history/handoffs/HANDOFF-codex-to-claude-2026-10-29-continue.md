# Codex to Claude: CONTINUE, activity 165, ship and XP — 29 Oct morning

For the "PS4 build 56578 retarget" session, delivered on disk as the human
requested. No further Computer Use/window changes. Claude retains all lab and
game input. This is static analysis plus bounded READ + QUERY-only reads of
owned client definitions; no game/control publication, active schedule edits,
native calls, memory writes, host builds, process stops or public commits.

## First: scoreboard draft is ready for your sense50 test

`F:\d1\d1preservation-main\cfw-staging\agr-postgame-scoreboard.draft.txt`

One kind15 row, synthetic 50 kills /15 precision /2 deaths; authority13,2,0;
NO phase7 directive. Owned object codec round trip IDENTICAL, full current
sensor-list parser/encoder PASS, 1163-byte frame. Fresh sense50 native player
identity matches the draft. Exact layout, provenance and two-step publication
sequence are in `HANDOFF-codex-to-claude-2026-10-29-scoreboard.md` beside this file.
Deliver while phase remains3, verify component AND UI counts1, then phase7.
An empty scoreboard rewards array does not grant inventory or XP.

## What the native departure route establishes

The normal fireteam-recall route can go through orbit AND retain the selected
chain activity165 for automatic launch. Those are successive steps, not mutually
exclusive destinations. An explicit return-to-orbit route clears that selection.
I have not yet proven the authored ACTIVITY REWARDS triangle widget's exact
binding to one of these routes, or that rows alone enable it. Please regard the
first successful sense50 button test as the final binding check.

The addresses below are ELF-relative for owned build56578, SHA672f0341…:

1. `abd740` processes the recall input and UI permission. It reads UI stages
   through788790 (these stage numbers are NOT lifetime phase numbers). At
   certain stages ab98e0 tests widget flags at `[982110()+11b]`. An allowed
   recall starts `955d40 -> 78bf70`, writing the fireteam countdown parameter
   at session params+eb8. This is local/session flow, not a postgame-rewards RPC.
2. `797ac0` consumes that countdown. On expiry, the local fireteam host sets
   the in-world handler byte+31 and calls649f40(1).
3. `797550` consumes byte+31. With lifetime phase7 it requests session0 goal
   1b (`setup:orbit`) with reason0. Its optional overwrite of the selected
   activity is controlled by75e430 ->545230. For current164 that predicate
   is FALSE: its investment definition byte+a9 is ff, and545230 returns
   `byte[a9] != ff`. Thus this branch does not replace selected165 with164.
4. Cleanup at7a5506 preserves the selection for reason0 (also5,6,119); the
   other branch invokes574ef0->574bb0, resetting it. In contrast,
   `955d60 ->78c130 ->797760` sets handler+32;797240 uses cleanup reason9,
   which clears it. The distinction matters when testing tap/hold actions.
5. Orbit updater7ac910 sees a valid selected activity on the local host,
   copies it through78b550 into the fireteam preference, then7acf00 copies
   the preference into session0 desired-activity params+9b8. It chooses a
   goal using the owned client definition: nonempty video hash at+18 ->25
   (`activity:watch_video`); empty811c9dc5 ->24 (`activity:in_world`).

Saved disassembly: private `continue-confirmed-native.txt`, plus
`continue-departure.txt`, `continue-selection.txt`, `recall-manager.txt`.
The enum name table is1945340;783940 resolves names. Goal24/25 here are hex,
i.e. world-controller rungs36/37. Do not confuse goal24 with rung24 prologue.

## Activity 165: owned facts and the unresolved world mapping

Both last night's snapshot and fresh sense50 loaded definitions confirm:

|Definition field|164|165|
|---|---|---|
|56578 activity hash|6e0dae89|6eaf0959|
|Investment chain, +98|type1, index165|type1, index0|
|Chain expression count|0|0|
|Type hash, +a4|648999f4 (Story)|4d78882e (STORY_CINEMATIC)|
|Investment launch config first dword|ffffffff|ffffffff|
|Investment byte+a9|ff|ff|
|Local client video-name hash +18|811c9dc5|811c9dc5|
|Local client activity-file tag +38|80a5e898|ffffffff|

The type names and index0's name "Tower" come from the local naming corpus;
the indices, hashes, chain records and raw fields above are owned56578 reads.
Index0's owned hash is8ae8da69. Do not silently replace it with another Tower
index just because a later manifest gives that other row a familiar name.

IMPORTANT CORRECTION to a tempting inference: STORY_CINEMATIC does not by
itself mean that165 takes the standalone watch-video path. Its actual local
video-name field is empty, so7acf00's displayed branch chooses goal24/in_world.
Positive control: local index163 (Destiny Credits in the naming corpus) has
nonempty video hash2ef5d468; ten other rows also have nonempty video hashes.
This is not a negative result from reading an all-zero/uninitialized table.

The ff launch-config tag means setup-matchmaking7aa6c0 takes7aa8f4 and moves
the activity session toward goal24 without starting the matchmaking seeker.
It does NOT establish that the preceding session-creation/config-refresh
steps emit no BAP request. The config-refresh task exists on rung28.

I cannot yet supply an authenticated165 world-table row. It has no activity
file in that local field, while164/166/168 do. Reusing Highway's world,
checksum, region113 and sensor list solely because the selection says165 is
not justified by the owned content. The prepared host table I inspected has
no165 row. This remains the concrete server-side gap if165 requests an AH join.
The static chain suggests165 is an intermediate step before index0/Tower;
its actual completion/transition behavior still needs the owner-driven test.

Evidence saved privately: `investment-launch-config-readonly.json`,
`launch-config-readonly.json`, `local-activity-video-control-readonly.json`,
and reproducible read-only `probe-launch-readonly.py`. The +20 fields in the
investment and local-client records are DIFFERENT:545510 returns the
INVESTMENT definition's +20 relative target. Its first dword is ffffffff.
The local-client +20 target is an expression array, not that launch config.

## What to answer on the wire

There is no newly established "CONTINUE" investment message to invent.
Last night's postgame spinner produced no new request beyond701/702, and
the scoreboard is AH kind15 replication. Keep those separate from launch.

If session creation refreshes MM configuration, the implemented contract is:

- **BAP42 request /43 response**, ordinary protobuf inside the established
  class1/class2 envelopes. Not34/35 (those are the separate skill policy).
- Request f1=version13; f2=request kind4; f3=nonce; f11=key, with f11.1
  activity hash, f11.2 matchmaking-content hash, optional f11.3 skill hash.
- Reply f1 echoes kind, f2 echoes nonce, and f4 is PRESENT. Existing minimal
  body is `08 04 10 <nonce-as-varint> 22 02 0a 00`; the final four bytes are
  f4 containing an empty f4.1 tuning message. Use the request's real nonce
  and content key; do not manufacture them. A successful minimal config
  reply is not proof that the later world mapping is correct.

Current source: `ps4\d1-server\crates\d1-bap\src\matchmaking.rs` and
`d1-responder\src\main.rs:4051` under
`F:\d1\public-work\d1reimagined-sense`. These handlers already exist.

The responder writes D1_ACTIVITY_FOLLOW_FILE ONLY from a kind4 request's
activity key. The AH reads the same path via D1_AH_ACTIVITY_FOLLOW_FILE,
maps the hash with D1_AH_ACTIVITY_TABLE, and checks its world table on each
JoinRequest. Verify that165 actually updates the file; a cached config or
skipped request can leave164 there. I have not established an alternative
automatic follow channel for that case.

If a new AH join occurs, retain the existing BAP8/request ->100/push path
and current codecs. AH state.rs:949 echoes the client's complete JoinRequest
in accepted JoinResult, with the session/AH id and replication epoch; it sends
the required UDT4,0,1,11 initialization set and then the appropriate world/sensor
data. Use fresh session fields and client nonce. Do not reuse the previous
join image. No exact departure-message id was recovered in this trace.

Material host behavior: if followed index165 is selected but no world-table
row exists, state.rs changes activity_index and RETAINS the previous world's
name/checksum/bubbles/region/sensor payload. That can report165 while serving
164's world. Treat this as a diagnostic failure, not successful chain launch.
No new responder family or world row was published by me.

## Ship and XP

The local item-content index gives two owned56578 candidates:

|Item hash|Owned item index|Owned definition tag|Naming-corpus description|
|---|---:|---|---|
|8e7bf3ba|3517|80a6467f|Arcadia Class Jumpship, old Golden Age ship|
|8e7bf3bb|3518|80a64680|Arcadia Class Jumpship, Tower-serviced variant|

Naming corpus gives ship bucket/equipment slot10fc26e7. The first is the
plausible initial ship candidate; the second is the repaired variant. I have
NOT recovered an authentic mission loot-grant rule proving which one the
reference screen awards or when. Activity164's named manifest reward is
4e696e20 "XP Bonus", not evidence of a ship inventory grant;165 has no listed
rewards there. Names/description metadata are a newer local corpus and must
not override owned56578 indices.

An actual ship grant needs the family4/type3 inventory element (stable item
instance SOID, owned item index, normal item fields) and, if equipped, the
character profile's ship equipment slot, plus whatever roster refresh the
existing item/equipment path requires. A kind22 reward in the scoreboard row
is only a display entry: it neither creates an investment item nor equips it.
The draft intentionally leaves it empty. I have not fabricated an inventory
instance or ship-grant delta for this first scoreboard test.

Progression117 is cumulative character XP, not displayed level:0 is level1,
900 reaches level2; the next cost4100 means total5000 reaches level3. Thus900
is an explicitly SYNTHETIC minimum for a level2 demonstration, not a recovered
reference mission total. Keep118/124/249/250 at their justified values; do not
put the displayed level number2 into117.

XP reaches the client through the character profile progression delta. Native
collector573950 reads profile progression through274970 and seeds all three
level-XP samples at collector+10/+14/+18 to the current total. 573b10 rolls
the current sample forward;573ff0 builds reward data from observed changes.
To show mission XP gained, the progression increase must be observed during
the collector's interval, before it finalizes; seeding900 before its initial
snapshot can show level2 with zero gained XP. Last night's collector was
already state4 with zeros and no rewards, so a late display row alone cannot
retroactively establish that XP interval. Exact family-push timing for an
animated900-XP test remains to be verified by the owner. No separate XP RPC
was established;505 in this area is not a reward/continue request to send.

## Suggested sense50 observation after the row test

After you verify row resolution and try the displayed CONTINUE input, capture
selection `[base+26bc068]` (type,+2 activity), session0 desired activity, the
world-controller rung/reason, new BAP42 key, the follow file and new AH join.
These distinguish: a still-disabled input; recall preserving165; explicit
orbit clearing it; successful165 selection with stale164 world; and a real
transition. Please keep the first test focused on row population and binding.

All raw definitions, decoded images and probe outputs remain under
`lab/history\2026-10-29-postgame-codex`.
