# Live-server message captures (5 November 2026)

A member of the project's Discord shared 49 captures of Destiny 1 played on Bungie's live servers between 27 October and 3 November 2026. There is one per mission:
- A Guardian Rises, Restoration and the rest of the Earth, Moon, Venus and Mars story;
- The Taken King and Rise of Iron missions;
- strikes and patrols;
- Crota's End;
- Wolves' Gambit, Cerberus Vae III and Dust Palace.

They were recorded on an Xbox 360 (title 415608F8, executable build `51539.16.07.26.1808`, the final last-gen build) by a console plugin that logs each message before encryption.

**They stay private.** The sign-on request in each one carries the player's Xbox Live token and account identifiers. The captures, and anything exported from them, live in the private engineering folder, never in this repository. Only tools that read them, and findings, go here.

## Format

They are classic pcap files with link type 147 (`USER0`). Each record is one plaintext message behind a 40-byte `SRCP` header:

| Field | Meaning |
|---|---|
| +0 | `SRCP` |
| +4 | version 1 |
| +6 | header length 0x28 |
| +8 | timestamp |
| +16 | direction: 0 client→server, 1 server→client, 2 plugin note |
| +24 | IPv4 address |
| +28 | port |
| +40 | the message |

**What a capture holds, in order:**
1. The sign-on (`POST /SignOn?platform=xenon&build=…` to Bungie's sign-on service).
2. The content manifest.
3. A BAP session to the services host. It carries the same message types our server answers, 0x0a-0x2b, 0x79, 0x7b, 0xab, 0xfa and 0x12e.
4. One activity-host session per activity, each opened with the same "DESTINY" hello (0x1e/0x1f, 0x19/0x1a) and the 0x06/0x07 manager exchange our server implements.
5. The plugin notes each session's state, and ends with its packet counts.

The frames are the plain frames our server builds before sealing them. The activity host's notifications are type 9 (`BC_NOT`: flag, id, udt, length, body); the client's messages to it are type 8 (`AH_NOT`).

`tools/ps4_setup/d1_srcp.py` reads them. `tools/ps4_setup/d1-srcp.py` summarises a capture, decodes its incidents, or exports every activity-host message, with its hex body, to a private JSONL file.

## What decodes

The 56578 client's own codec (`d1-oracle activity-host decode`) decodes the last-gen bodies:

| udt | Result |
|---|---|
| 11 (world state) | decodes |
| 2 | decodes |
| 18 (incidents) | decodes |
| 1 (`global_activity_state`) | guest fault: its layout differs between the builds |
| 5 (`sensor_auth_update`) | `d1-oracle sensor decode-batch` walks the frames (below) |
| 23, 24 | not pinned in `d1-oracle`'s call sites yet |

**Incident types are numbered per build.** Three pairs are matched so far:

| Meaning | 51539 (360) | 56578 (PS4) | Offset |
|---|---|---|---|
| a cinematic stopped | 734 | 891 | +157 |
| a cinematic started | 2120 | 2681 | +561 |
| the player crossed a trigger | 2665 | 3403 | +738 |

The trigger report's body has the same layout on both builds: at bit 303, the group hash, the kind and the index (`trigger_report` in `d1-activity-host`). The 360 client's 2665 reports name AGR's own trigger group, `0x992BB471/0x1D/42`, as run agr-breach13's 3403 did on PS4. The offsets grow with the id, as they would if both builds number one sorted table and the later build has more entries. So the mapping is probably monotonic, but each id still needs its own anchor, or the two tables.

## A Guardian Rises, as the live host ran it

This is the first capture, about 20 minutes long. The host session opens about six minutes in (after the title, character select and orbit) and lasts about 11 minutes. In it:
- **The host sent:** 569 udt-5 updates, 65 world states (11), 53 each of udt 1 and 2, 20 incidents (18), and 6 each of udt 23 and 24.
- **The client sent:** 1,375 udt 6, 178 udt 15, 173 incidents (18), 57 udt 36, 54 udt 21, 32 udt 37, 13 udt 17, 11 udt 47 and 7 udt 32.

**The host's incidents, in order** (times from the session's opening):

| Time | Incident type |
|---|---|
| +2 s (as the session opens) | 2426 |
| +169 s onward | 2938, fifteen times over the mission (with 6 each of udt 23 and 24 from the first ones) |
| +263 s | 808, once |
| +494 s | 1170, once |
| +584 s | a five-type chain beginning 789 |

The next host session, 17.5 minutes into the capture, opens with 2426 again and ends with a four-type chain beginning 789.

**The live timeline** (times from the host session's opening):
- **+18.5 s to +117.7 s:** the opening cinematic, watched in full (2120, then 734, both naming `0x992BB471/6/1`).
- **+119.9 s to +167.2 s:** the Ghost's cinematic (`.../6/2`).
- **+168.4 s:** gameplay starts. The host sends three 2938 incidents, two world states, and udt 23 and 24.
- **From +171.7 s:** trigger reports in groups 0x992BB471, 0xECF574B5, 0x3C2F5ED5 and 0xFC0E45A3, each followed by the host's world-state (udt 11), udt 1 and udt 2 updates.

That is the reference for our activity host's steps: which trigger the real host answered, when, and with what. It can be printed for any mission in the set.

## The live host's sensors (udt 5)

`d1-oracle sensor decode-batch` reverses the framing our own encoder writes:
- gen and the authority table (a 65-bit changed mask, then the replicated table);
- a stamp and init;
- then each entry: its name reference, decoded through the client's codec, a 32-bit `body_bit_length`, and the body.

The bodies are kept as raw bits, so the list can be walked without knowing each value's type. It reads back our encoder's frames exactly (`decode_frame_reads_back_an_encoded_frame`), and all 617 of A Guardian Rises' udt-5 frames from the live host decode. `d1-srcp.py sensors` prints them.

What they show:
- **The host keeps time with them.** Between updates it sends an empty frame every 3 s, its stamp rising by 30 (a 10 Hz host clock).
- **Our AGR lists name the same records.** All 352 records in our served AGR lists (`ps4/fixtures/56578/mission/agr*.txt`) appear in the live host's stream under the same group, kind and index. Over the mission there were seven groups:
  - 0x4786C0E0 (world);
  - 0x992BB471 (the mission);
  - 0x3C2F5ED5, 0xFC0E45A3, 0x9243DFFB and 0x062DD95D (the areas);
  - 0xECF574B5 (the opening, done by +173 s).
- **The Tower follows the mission on the same host, from +804 s:**
  - 0x40AEBF90 (the Tower common group: its 30 player slots, `notes/tower.md`);
  - 0x8CC31436 (the plaza);
  - 0x19D43443 (the hangar);
  - 0xF2D88BD0 (the cast).

  Our Tower notes call our vendor and civilian choices a reconstruction, because "the original servers' choices … are not in the assets". This is the original server's choice, on 27 October 2026.

**Values.** `d1-oracle sensor decode-values` decodes an entry's value with the typed-value codec: the A value, from bit 2 after `fresh` and `presence`, with the tag our list gives that record. In the first comparison against our AGR lists (one identical, 125 different, 285 not yet decodable):
- **Squad members (kind 1)** carry a name reference on the live host (the group's own hash, kind 3, an index). Ours carry the empty default.
- **Triggers (kind 0x1D)** carry a changing 16-bit value at +4. Ours hold 1.
- **Cinematic records (kind 6)** step through 1 → (start) → 4. Ours hold 2.
- **The objective records (kind 0x20)** differ throughout.
- **The undecodable updates** have `fresh = 0`: partial updates against the value the client holds. They need the previous value and the delta rules (`0x2D0F63..`, the walker) before they can be read.

## Where the live client went (udt 21)

The client's own state (udt 21, `client_authoritative_data_update`) holds its current slice set at +0xB8: a 10-bit index and a name hash.
- **A Guardian Rises:** the live client entered slice set **113** first (hash 0xEAEC2335), our start region for 164. Then 112, 24, and **96** (hash 0x10FF7418), the Divide.
- **`cine_reveal` (165):** **25**, then **26** for its second cinematic. We start 165 at region 24; it stalled there on the loading screen (sense124, `ps4-56578-dismantle-2026-11-02.md`).
- **Restoration (166):** **56**, our start region for 166, then 104.

## The reveal cinematic (activity 165) as the live host ran it

The AGR capture's second host session, 17.5 to 19 minutes in, is `cine_reveal`. Its join is the usual one (udt 4, 0, 11, 1, 2), followed by incident 2426. About 10 s later one udt-5 frame:
- sets the authority mask to `0x10000000000000007`;
- serves **two cinematic records**, `0x2FE7DBE6/6/0` and `/6/1` (tag 0x8080254D);
- serves the world group's block, `0x4786C0E0` kinds 0xF, 0x22, 0x11, 0x10 and 0x27, and the player slots `0xC/5..20`. This is the same block our AGR highway list serves.

The cinematics then run on the host's word. Each record's value is `{u32 start, u32 state, u32 flag}`:

| Time from join | Record | Value |
|---|---|---|
| +10 s | both | state 1 (idle) |
| +12.7 s | `/6/0` | start = the host's stamp, state 2, flag 1 (playing). The client reports a cinematic started (2120) |
| +44.5 s | | the client reports it stopped (734) |
| +45.8 s | `/6/0` | state 4 (done) |
| +50 s | `/6/1` | state 2: the second cinematic plays |
| +96.7 s | `/6/1` | state 4. Then udt 23 and 24, and the client moves to the Tower |

We serve 165 an empty sensor list. The live host served both cinematic records, then stepped them through these states.

## Restoration's public start (activity 166)

Restoration's region 56 (bubble 7) is public. That is where our client goes to a black screen (`ps4-56578-public-sessions-2026-11-02.md`). The live capture has four host sessions. Session 1 opens at 7 s, session 2 at 31 s, session 3 at 57 s, and session 4 at 324 s. Each is requested by the client from the services host (0x10/0x11, the activity-host proxy assignment).

`join_result` echoes the client's join request in its first 0x100 bytes, so session details at +0x38 and +0xC8 are the client's. Its type byte is 3 for the first two sessions and 2 for the last two (2 for the Tower, too). From session 2 on it lists the previous sessions' host ids.

The PAH region record in udt 11 (`replicate_membership+0x2E48`, the record our host fills with the region id and our peer token):

| Session | Region | +0x04 | +0x08..0x0B | +0x4C (session) | +0xD0 (host) |
|---|---|---|---|---|---|
| AGR | 0x71 (113, private) | 0 | `01 01 00 00` | 0 | 0 |
| Restoration 1 | 0x38 (56) | **1** | `01 01 00 00` | 0 | 0 |
| Restoration 2 | 0x38 | 1 | `01 01 00 00` | `4e000000 820940a1` | **session 2's own host id** |
| Restoration 3 and 4 | 0x30 (48), back to 0x38 later | 1 | `01 01 00 00` | `4e000000 8009e170` | session 3's own host id |

- **The ambassador bytes are set for private regions too.** `+0x09 = 1`, `+0x0A = 0` (the local peer) in every session, AGR included.
- **What marks a public region is `+0x04 = 1`.** The public session's host then advertises itself, its session at +0x4C and its own host id at +0xD0. These are plan steps 2 and 5 in the public-sessions note, now with the live values.
- **Between the two sessions,** the client exchanges services messages 0x0a (2,409 bytes) and 0x2a/0x2b with the services host, at 20 s and 25-31 s. That is the matchmaking step 3 of the plan.

## Every capture's host sessions

`d1-srcp.py sessions CAPTURE...` lists, per activity-host session:
- the type the client's join request carries (private 3, public 2);
- the activity the host's join incident (2426) names, through the public manifest;
- the slice sets the client reported as current (udt 21), in order.

A mission typically opens with a short private session naming it, with no slice set yet, while the world loads. It then plays across one or more sessions, switching between private and public as the player moves between mission and patrol bubbles. Each session's first slice set is the start region an activity host has to serve.

| Capture | Host sessions: type, slice sets the client reported (in order) |
|---|---|
| A Guardian Rises | private 113, 112, 24, 96, 104, 105; private 25, 26; public 24 (Tower) |
| Restoration | private -; private 56; public 56; public 56, 104 |
| The Dark Within | private -; private 56; private 56, 48, 64; public 64 |
| The Warmind | private -; public 56; private 56, 48; private 48, 88, 72 |
| The Speaker | private 1 |
| The Last Array | private -; public 48; private 48, 64; private 64, 72 |
| The Dark Beyond | private 9, 8; public 8; private 8, 48, 24, 25 |
| The Worlds Grave | private - (The World's Grave); private 8, 104, 40, 88 |
| Sword of Crota | private - (The Sword of Crota); private 8; private 8, 16, 96 |
| Shrine of Oryx | private -; private 16, 96, 40, 104, 72 |
| Chamber of Night | private -; private 16; private 16, 48, 24, 25, 88 |
| A Strangers Call | private 161, 24 (A Stranger's Call); public 24, 160, 64; public 64, 65 |
| The Awoken | private 17, 1 |
| Ishtar Collective | private 24; public 24, 160; private 160, 80; private 80, 224, 168 |
| Eye of a Gate Lord | private 24; private 24, 72; private 72, 32; private 32, 224, 168, 104; private 1 (A Key Awaits); private - (Vestian Outpost) |
| The Archive | private 24; private 24, 160, 64; private 64, 120, 208 |
| Exclusion Zone | private 145, 144; private 144, 8, 24; public 24, 208 |
| The Gardens Spire | private - (The Garden's Spire); private 144; private 144, 8, 160; public 160; private 160, 112, 72, 224 |
| The Buried City | private -; private 144; private 144, 48; public 48, 136 |
| A Rising Tide | private -; private 144; private 144, 8, 16; private 16, 192, 96, 176 |
| The Black Garden | private -; private 144; private 144, 8, 24; private 24, 208, 32, 200, 104, 105; private 17 (A Hero's Return) |
| Scourge of Winter | private 24; private 24, 72, 184, 48, 88 |
| Fist of Crota | private 16, 152, 120 |
| Siege of the Warmind | private -; private 48; public 48; private 48, 88, 144, 136 |
| The Wakening | private -; public 8; private 8, 48, 24, 88, 80 |
| Scourge of Winter / HoW | private 24; private 24, 72, 184, 48, 88 |
| A Kell Rising | private 8, 48, 184 |
| Patrol / Scattered Pack | private 24 (Patrol Venus); private 24, 72 |
| The Silent Fang | private -; private 96, 24, 112 |
| The Ruling House | private 120; private 120, 152; private 152, 160, 32 |
| Wolves Gambit | private - (Wolves' Gambit); public 80, 224, 16, 128 |
| The Queens Ransom | private - (Queen's Ransom); private 224; private 224, 32, 96, 192 |
| The Coming War | private 25, 24, 8, 1 |
| Caydes Stash | private 16, 168, 8 (Cayde's Stash); private 17 (Hangar Stakes) |
| The Dreadnaught | private 129, 128, 72, 8 |
| Enemy of My Enemy | private 8, 96, 152 |
| Lost to Light | private 40, 88, 80, 24 |
| The Promethean Code | private -; private 88, 144, 136, 40, 128 |
| Last Rites | private 32 |
| Regicide | private 8, 152, 136, 128, 104, 105 |
| Dread Patrol | private 8, 152, 72, 128 |
| Crotas End | private 16, 32 (Crota's End) |
| The Devils Lair | private - (The Devils' Lair); private 160, 152, 16, 168 |
| The Summoning Pits | private -; private 16, 96, 40, 112 |
| Patrol / Silent Scream | private - (Patrol the Dreadnaught); private 8, 96; private 96, 88 |
| The Nexus | private -; private 120, 56, 40, 216 |
| Winters Run | private 24 (Winter's Run); private 24, 72, 184, 48, 8, 176 |
| Cerberus Vae III | private -; private 112, 72, 80; private 80, 208, 184 |
| Dust Palace | private -; private 48, 136, 40 |


## Next

- **Serve 165 as the live host did** (the two cinematic records, start region 25, the state steps), and retry it.
- **Restoration's public start, by the live values:** the region record's +0x04, then the second host and its advertisement, and the services messages 0x0a/0x2a the client matchmakes with.
- **Map the incident types between 51539 and 56578,** from pairs the two clients send at the same moments (cinematic start and stop), or from the builds' incident tables.
- **Delta (`fresh = 0`) sensor updates, then field by field:** what the live host's values mean, for the records where ours differ (the squads' names, the trigger values, the objectives).
- **The Tower: too little of it here.** Only A Guardian Rises (40 s) and Eye of a Gate Lord (36 s) reach it, not enough for its vendor, civilian and Xûr records. Group 0x8CC31436, which earlier notes called the Tower plaza, also appears in The Archive and The Nexus.
- **Pin udt 23 and 24 in `d1-oracle`.** `d1-oracle activity-host decode-batch` (stdin `UDT HEX` lines, JSON out) decodes a whole mission in seconds; one start costs about 3 s.
- **Compare each mission's live host timeline with what our activity host serves,** A Guardian Rises first: its incidents, world states and objective updates.
