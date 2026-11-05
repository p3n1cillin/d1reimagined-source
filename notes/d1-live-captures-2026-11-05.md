# Live-server message captures (5 November 2026)

A member of the project's Discord shared 49 captures of Destiny 1 played on Bungie's live servers between 27 October and 3 November 2026, then three more strikes (The Will of Crota, The Undying Mind and The Shadow Thief, 4-5 November). There is one per mission:
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
4. Activity-host sessions, each opened with the same "DESTINY" hello (0x1e/0x1f, 0x19/0x1a) and the 0x06/0x07 manager exchange our server implements. The 0x07 answer names the new host's id, and every message on that connection carries it. **The client holds several at once:** the mission's host stays connected all mission, and public instances join beside it ("Every capture's host sessions" below).
5. The plugin notes each session's state, and ends with its packet counts.

The frames are the plain frames our server builds before sealing them. The activity host's notifications are type 9 (`BC_NOT`: flag, id, udt, length, body); the client's messages to it are type 8 (`AH_NOT`).

`tools/ps4_setup/d1_srcp.py` reads them. `tools/ps4_setup/d1-srcp.py` summarises a capture per activity-host id, decodes its incidents, sensor updates and PAH region records, lists its host sessions, or exports every activity-host message, with its host id and hex body, to a private JSONL file. `--host` limits any command to one host.

## What decodes

The 56578 client's own codec (`d1-oracle activity-host decode`) decodes the last-gen bodies:

| udt | Result |
|---|---|
| 11 (`replicate_membership`) | decodes; it is replicated (only what changed), so `decode-batch` takes a stream column to decode it against the previous image |
| 2 | decodes |
| 18 (incidents) | decodes |
| 1 (`global_activity_state`) | guest fault: its layout differs between the builds |
| 5 (`sensor_auth_update`) | `d1-oracle sensor decode-batch` walks the frames (below) |
| 23, 24 | decode (pinned 5 November; "Entity-slot handover" below) |

**Incident types are numbered per build.** Three pairs are matched so far:

| Meaning | 51539 (360) | 56578 (PS4) | Offset |
|---|---|---|---|
| a cinematic stopped | 734 | 891 | +157 |
| a cinematic started | 2120 | 2681 | +561 |
| the player crossed a trigger | 2665 | 3403 | +738 |

The trigger report's body has the same layout on both builds: at bit 303, the group hash, the kind and the index (`trigger_report` in `d1-activity-host`). The 360 client's 2665 reports name AGR's own trigger group, `0x992BB471/0x1D/42`, as run agr-breach13's 3403 did on PS4. The offsets grow with the id, as they would if both builds number one sorted table and the later build has more entries. So the mapping is probably monotonic, but each id still needs its own anchor, or the two tables.

## A Guardian Rises, as the live host ran it

This is the first capture, about 20 minutes long. The host session opens about six minutes in (after the title, character select and orbit) and lasts about 11 minutes. In it:
- **The host sent:** 569 udt-5 updates, 65 memberships (11), 53 each of udt 1 and 2, 20 incidents (18), and 6 each of udt 23 and 24.
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
- **+168.4 s:** gameplay starts. The host sends three 2938 incidents, two memberships, and udt 23 and 24.
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

We served 165 an empty sensor list. The live host served both cinematic records, then stepped them through these states.

### Serving it (5 November)

`ps4/fixtures/56578/mission/world-table-cine-live.txt` serves 165 with:
- `sensor-clients-cine_reveal-live.txt`: the cinematic record, idle, plus the world block;
- start region 25.

`tools/ps4_setup/d1-cine-watch.py RUN` then steps the cinematic through the host's control file:
- `cine_reveal-play-0.txt` once the client reports slice set 25, or 15 s after the join;
- `cine_reveal-done.txt` on the client's incident 891.

**The first run to reach 165, cine-live8 (5 November, 08:59-10:17), gets into the world:**
- It played A Guardian Rises unattended from the checkpoint to the post-game, and CONTINUE moved it to 165 at 09:57. Codex's app had been closed. The client reported no network gap during play, only one of 4.9 s while loading into orbit.
- **Start region 25 loads.** The client transitioned to `PRV25.25`, then went through `activity:physics_join` to `activity:in_world`. With region 24 (sense124) it had sat on the loading screen.
- **The client's own list has one cinematic record**, `0x2FE7DBE6/6/0` (`sensor-clients-cine_reveal-native.txt`, read from its memory once the world was resident). The Xbox 360 host served two. Every record we served was delivered (pending 0), so the lists now leave `/6/1` out.
- **The cinematic did not start.** The screen stayed black, and no incident 2681 came, for each of these play states:
  - state 2 with start 0 and flag 0;
  - the same with the live flag 1;
  - each again with only `/6/0`.

  The live host's start was its own frame stamp (`0xC7`), which our host does not log, so that is the next thing to try. Something else the client waits on in a cinematic world is the alternative.
- **Two watcher bugs this run found:**
  - The host never logged "client is now in slice set 25" in 165, so the first cue never came. The watcher now falls back to 15 s after the join.
  - It died printing the host's emoji to a cp1252 console. It now writes UTF-8.

Earlier attempts did not reach 165:
- cine-live1: the D1 Video menu's saved 60 fps wrecked its walks (hence #264).
- cine-live2 and cine-live3: the game's own block detector crashed them in the post-game tour and the hangar.
- cine-live5: the client stalled ("4739 milliseconds since the last network update") and dropped the host ("baboon") at junction-west.

## Entity-slot handover (udt 23-26)

`d1-oracle` pins udt 23 and 24 for 56578 from the receive table: entries 9 and 10 load their descriptor globals `0x191A3C8` and `0x191A3D8` as udt 0's handler loads its own. Our protocol table names all four:

| udt | Direction | Name | Layout |
|---|---|---|---|
| 25 | client → host | `abandon_entity_slots` | u8 at +0x00, an entity-slot set (`0x808043A7`, 0x600 bytes) at +0x04, 3 bits at +0x604 (3) |
| 23 | host → client | `claim_authority_over_abandoned_entity_slots` | the same layout; 3 bits at +0x604 = 4 |
| 26 | client → host | `request_purge_entity_slots` | 3 bits at +0x00 (4), the slot set at +0x04 |
| 24 | host → client | `purge_abandoned_entity_slots` | 3 bits at +0x00 (4), u8 at +0x01, the slot set at +0x04 |

**How the live host ran it.** The client sends 25 as the player leaves bubbles (in A Guardian Rises at 524 s, 772 s, 934 s and 1042 s; in Restoration on leaving the public instance at 395 s), and then:
- **the host answers with 23**, echoing the client's slot set. It merges 25s that arrive together: at 772 s its one 23 carries the union of the two 25s' sets;
- **the client asks to purge (26)** with the same set, and **the host purges (24)** with the same set;
- **the host's u8s share one counter.** Across its 23s and 24s they run 1, 2, 3 … 11 over A Guardian Rises, with one repeat (two 23s at 934 s both carry 9);
- **the host also sends 23 unprompted.** In A Guardian Rises it did so at 580 s and 866 s, and cine_reveal's host did once, as the cinematic ended and before the move to the Tower (its own counter starting at 1 again). The client answered with 26 as usual.

**Incident 2938 comes with it.** In A Guardian Rises the host sent a 2938 within half a second of every 23/24 exchange: 524, 580, 772, 866 and 934 s, and on cine_reveal's host at its one handover. A few others came at 570, 618, 851 and 928 s, likely other slot or authority changes. Its body is 27 bytes: a counter that rises by one with each of the host's incidents, then a repeated pair of ids. The other bodies the host sent are 58 bytes for 2426 (the join), 38 for 808, 13 for 1170 and 79 for the closing chain beginning 789. Sending 2938 needs its 56578 id, which the three matched pairs only bracket ("What decodes").

**Our host answers none of this**: 25 and 26 fall through unanswered. So in our runs the client's abandoned slots are never claimed or purged. Whether that costs anything (slots running out on a long run, entities left behind after a transition) is untested.

## Restoration's public start (activity 166)

Restoration's region 56 (bubble 7) is public. That is where our client goes to a black screen (`ps4-56578-public-sessions-2026-11-02.md`).

**The client holds three activity hosts, not four in turn.** Read by host id (`d1-srcp.py summary` groups by it), the capture has:
- **the mission host, the PAH**, from 9 s to the end at 552 s. Its join incident names Restoration, and it receives every slice set (56, then 104);
- **a public instance the client starts**, from 33 s to the end, beside the PAH;
- **another player's public instance, which the client joins**, from 58 s to 340 s, twice.

Each host was requested from the services host by message 0x10/0x11 and announced by its own 0x07. The fourth "session" counted earlier was the second join to the third host.

`join_result` echoes the client's join request in its first 0x100 bytes. Its type byte (+0x38) is 3 on the hosts the client starts (the PAH and its own public instance) and 2 where it joins one (the other player's, and the Tower in A Guardian Rises). So 3 most likely means "start an instance" and 2 "join one", not private and public.

The public instance's join and membership are echoes of the client's join request, like the PAH's. They differ from the PAH's in two places:
- the client's fireteam id (join request +0xD8) is 0;
- join request +0xC8 names the PAH's host id.

**Entity indices are per host.** Each host's first `entity_slots_allocated` (udt 0) granted the client 500 indices by its peer slot. The PAH and the client's own public instance both granted 0-499; the other player's instance granted 500-999, the client being its second peer. So two hosts need no disjoint ranges for one client (the public-sessions note's first risk). Our host grants the whole pool at the join.

### The PAH's region records (udt 11, `replicate_membership+0x2E48`)

Only the PAH serves region records. The public instances served none. `d1-srcp.py regions` follows them, decoding each membership against the previous one:

| Time | Region record |
|---|---|
| 20.5 s | region 56: `+0x04 = 1` (public), `+0x08..0x0B = 01 01 00 00` (ambassadorship to peer 0), the client's transition token at `+0x38`, no session |
| 20.6-25.8 s | the client searches (below) |
| 30.7 s | `+0x19 = 1` |
| 33.0 s | the client joins its new public instance |
| 34.2 s | **the session at `+0x4C` (u32 0x4E, then 64 bytes) and the public instance's host id at `+0xD0`** |
| 52.2 s | a second record: region 48, public, no session yet |
| 60.0 s | region 48 advertises the other player's session and host, two seconds after the client joined it |
| 341 s | region 104 (private, `+0x04 = 0`) |

Notes:
- `+0x19` goes to 1 and back as the player enters and leaves a region (region 48: 0 → 1 at 83 s → 0 at 127 s). It is most likely "the peer is in this region" [I].
- `+0x20` is 1 in every record. `+0x38` is peer 0's transition token, as our host already serves it.
- The session is the platform's: on the 360, a session key and the console's network address. **It is personal data and stays in the private captures.**

**Where the PAH got them: the client's udt 21.** Udt 21 is also replicated. It carries the client's two slice-set records (`0x80800D9F`, 0xA0 bytes each): CURRENT at +0xB8 and TARGET at +0x158. For region 56 the TARGET record went:
- **31.4 s:** the session at record +0x14 (0x84 bytes), with record +0x11 = 1;
- **33.65 s:** the public instance's host id at record +0x98;
- **33.9 s:** record +0x11 = 2.

Record +0x11 is the pending region's state the 2 November note found the client writing (`sstm+0x141`): 2 is "public AH instance ready". The PAH copied the reported session and host id into its region record, unchanged.

The PAH region record's `+0x4C..+0xD0` (`advertised_session`, 0x84 bytes) and `+0xD0` (`advertised_activity_host`) are byte for byte the slice-set record's +0x14..+0x98 and +0x98. So plan step 5 is an echo, not something the host builds. `d1-activity-host` does it since this note (`track_public_session`).

### Restoration's matchmaking (services messages 0x2a/0x2b)

Message 42 (0x2a, `d1-bap/src/matchmaking.rs`) carries protobuf bodies. The client's exchange in Restoration:

| Time | Request | Answer |
|---|---|---|
| 20.6 s, 25.8 s | **search** (field 2 = 1): field 5 = `0x8001_0038`, region 56; field 7 = the criteria | one candidate: a session id, another player's name, a score (0.998), 400 |
| 30.9 s | activity-host assignment 0x10 with **host 0**: a new host | the client starts its public instance (host 2) |
| 31.4 s | **create/advertise** (field 2 = 2): field 8 = the new session (the one it then reports in udt 21 and the PAH advertises) and the player's own name | a registration id (field 5) |
| 34.7 s, 50.3 s | updates with that registration id and the slot states | the same id |
| 52.3 s | **search**: field 5 = `0x8001_0030`, region 48 | the other player's session |
| 56.9 s | assignment 0x10 with **that player's host id** | the client joins that instance (host 3), a citizen join |

So for region 56 the client created its own public session even though the search found one. For region 48 it joined the found one.

This is plan steps 3-5 in the public-sessions note, as the live services ran them:
- the search answer (a found session, or none);
- a new host on request;
- the registration answer;
- a join by host id;
- the PAH advertising what the client reports.

The player names in these messages are personal data and stay in the private captures.

## Every capture's host sessions

`d1-srcp.py sessions CAPTURE...` lists one row per activity host, in the order the client joined them. Each row gives:
- the join request's type byte: "starts" (3) or "joins" (2);
- the activity the host's join incident (2426) names, through the public manifest;
- the slice sets the client reported to that host (udt 21), in order.

`d1-srcp.py regions` gives the regions the mission's PAH marked public.

**How a mission runs on the live servers:**
- **The PAH stays connected all mission.** The first host is the PAH: it names the mission and receives every slice set the player passes through.
- **Public regions get extra hosts.** As the player nears a public region, the client starts that region's public instance, 20 to 90 s into the mission, or joins another player's ("joins"). These hosts run beside the PAH and receive the slice sets near their region.
- **Only the PAH serves region records.** It advertises each public instance in them. Follow-on activities (A Guardian Rises' cine_reveal and the Tower, A Key Awaits, A Hero's Return, Hangar Stakes) get a PAH of their own.

| Mission | Hosts in order: starts/joins, slice sets the client reported | Public regions (PAH `+0x04 = 1`) |
|---|---|---|
| A Guardian Rises | starts 113, 112, 24, 96, 104, 105; starts 25, 26; joins 24 (Tower) | none |
| Restoration | starts 56, 104; starts 56, 104; joins 56 | 48, 56 |
| The Dark Within | starts 56, 48, 64; starts 56, 48; starts 56, 48, 64; joins 64 | 0, 48, 56, 88 |
| The Warmind | starts 56, 48, 88, 72; joins 56, 48; starts 56, 48, 88; starts 48, 88, 72 | 48, 56, 88 |
| The Speaker | starts 1 | none |
| The Last Array | starts 48, 64, 72; joins 48, 64; starts 48; starts 64, 72 | 0, 48, 56 |
| The Dark Beyond | starts 9, 8, 48, 24, 25; joins 8; starts 8, 48, 24 | 16, 48 |
| The World's Grave | starts 8, 104, 40, 88; starts 8, 104 | 8, 16 |
| The Sword of Crota | starts 8, 16, 96; starts 8, 16; starts 8, 16, 96 | 8, 16, 48 |
| Shrine of Oryx | starts 16, 96, 40, 104, 72; starts 16, 96 | 16 |
| Chamber of Night | starts 16, 48, 24, 25, 88; starts 16, 48; starts 16, 48, 24 | 8, 16, 48 |
| A Stranger's Call | starts 161, 24, 160, 64, 65; joins 24, 160, 64 | 80, 120, 160 |
| The Awoken | starts 17, 1 | none |
| Ishtar Collective | starts 24, 160, 80, 224, 168; joins 24, 160, 80; starts 160, 80, 224; starts 80, 224, 168 | 80, 160, 224 |
| Eye of a Gate Lord | starts 24, 72, 32, 224, 168, 104; starts 24, 72, 32; starts 72, 32, 224; starts 32, 224, 168; starts 1 (A Key Awaits); starts - (Vestian Outpost) | 32, 72, 160, 224 |
| The Archive | starts 24, 160, 64, 120, 208; starts 24, 160, 64; starts 64, 120 | 120, 160 |
| Exclusion Zone | starts 145, 144, 8, 24, 208; starts 144, 8, 24; joins 24, 208 | 8, 48, 208 |
| The Garden's Spire | starts 144, 8, 160, 112, 72, 224; starts 144, 8; starts 144, 8, 160; joins 160; starts 160, 112, 72 | 8, 48, 112, 144 |
| The Buried City | starts 144, 48, 136; starts 144, 48; starts 144, 48; joins 48 | 48, 144, 192 |
| A Rising Tide | starts 144, 8, 16, 192, 96, 176; starts 144, 8; starts 144, 8, 16; starts 16, 192, 96 | 8, 48, 144, 192 |
| The Black Garden | starts 144, 8, 24, 208, 32, 200, 104, 105; starts 144, 8; starts 144, 8, 24; starts 24, 208, 32; starts 17 (A Hero's Return) | 8, 48, 144, 208 |
| Scourge of Winter | starts 24, 72, 184, 48, 88; starts 24, 72, 184 | 72, 160 |
| Fist of Crota | starts 16, 152, 120 | none |
| Siege of the Warmind | starts 48, 88, 144, 136; starts 48, 88; joins 48; starts 48, 88, 144 | 48, 56, 88 |
| The Wakening | starts 8, 48, 24, 88, 80; joins 8, 48; starts 8, 48, 24 | 8, 16, 48 |
| Scourge of Winter | starts 24, 72, 184, 48, 88; starts 24, 72, 184 | 72, 160 |
| A Kell Rising | starts 8, 48, 184 | none |
| Patrol Venus | starts 24, 72; starts 24, 72 | 32, 72, 160 |
| The Silent Fang | starts 96, 24, 112; starts 96, 24 | 96 |
| The Ruling House | starts 120, 152, 160, 32; starts 120; starts 152, 160, 32 | 88, 160 |
| Wolves' Gambit | starts 80, 224, 16, 128; joins 80, 224 | 80 |
| Queen's Ransom | starts 224, 32, 96, 192; starts 224, 32; starts 224, 32, 96 | 32, 80, 224 |
| The Coming War | starts 25, 24, 8, 1 | none |
| Cayde's Stash | starts 16, 168, 8; starts 17 (Hangar Stakes) | none |
| The Dreadnaught | starts 129, 128, 72, 8 | none |
| Enemy of My Enemy | starts 8, 96, 152 | none |
| Lost to Light | starts 40, 88, 80, 24 | 48 |
| The Promethean Code | starts 88, 144, 136, 40, 128; starts 88, 144 | 88 |
| Last Rites | starts 32 | none |
| Regicide | starts 8, 152, 136, 128, 104, 105 | none |
| Dread Patrol | starts 8, 152, 72, 128 | none |
| Crota's End | starts 16, 32 | none |
| The Devils' Lair | starts 160, 152, 16, 168; starts 160, 152 | 96, 160 |
| The Summoning Pits | starts 16, 96, 40, 112; starts 16, 96 | 16 |
| Patrol the Dreadnaught | starts 8, 96, 88; starts 8, 96; starts 96 | 8 |
| The Nexus | starts 120, 56, 40, 216; starts 120, 56 | 120 |
| Winter's Run | starts 24, 72, 184, 48, 8, 176; starts 24, 72, 184 | 72, 160 |
| Cerberus Vae III | starts 112, 72, 80, 208, 184; starts 112, 72; starts 80, 208, 184 | 112, 208 |
| Dust Palace | starts 48, 136, 40; starts 48 | 48, 144 |
| The Will of Crota | starts 88, 72, 80; starts 88, 72; starts 72, 80 | 0, 88 |
| The Undying Mind | starts 104, 200, 88 | none |
| The Shadow Thief | starts 24, 64, 56; joins 24 | 48 |

**Thirteen captures use only one host:** The Speaker, The Awoken, Fist of Crota, A Kell Rising, The Coming War, The Dreadnaught, Enemy of My Enemy, Lost to Light, Last Rites, Regicide, Dread Patrol, Crota's End and The Undying Mind. Twelve mark no region public; Lost to Light marks region 48 public but never starts an instance for it. These, like A Guardian Rises, need only what our host already serves: one connection, private regions. They are the natural next missions to bring up. The rest need the public instance (plan step 4).

`d1-srcp.py mission CAPTURE... --activity-table activity-table.tsv` gives what a host needs for each one:
- the activity, with the client's index, from the private activity table dump;
- the world group (the first group in the PAH's first sensor frame);
- the start slice set;
- the sensor groups the PAH served.

For A Guardian Rises it gives index 164, world `0x4786C0E0` and start 113, our served values. Almost every mission uses world group `0x4786C0E0`. Crota's End (`0x24C67333`) uses its own, and The Nexus, The Undying Mind and The Shadow Thief share `0xF29221F5`. The sensor lists themselves (record kinds and value types) still come from the native client in the lab.

| Mission | Activity hash | Index | World group | Start slice set | Slice sets | PAH sensor groups (records) |
|---|---|---|---|---|---|---|
| The Speaker | `0xd8b2ad53` | 167 | `0x4786c0e0` | 1 | 1 | 2 (22) |
| The Awoken | `0x9a7acc78` | 186 | `0x4786c0e0` | 17 | 17, 1 | 2 (23) |
| Fist of Crota | `0xcf0e39ac` | 207 | `0x4786c0e0` | 16 | 16, 152, 120 | 7 (220) |
| A Kell Rising | `0xaec1f80e` | 214 | `0x4786c0e0` | 8 | 8, 48, 184 | 7 (195) |
| The Coming War | `0x8c78c021` | 235 | `0x4786c0e0` | 25 | 25, 24, 8, 1 | 6 (265) |
| The Dreadnaught | `0xbe2a2b4d` | 275 | `0x4786c0e0` | 129 | 129, 128, 72, 8 | 19 (509) |
| Enemy of My Enemy | `0xece80ead` | 254 | `0x4786c0e0` | 8 | 8, 96, 152 | 14 (342) |
| Lost to Light | `0x4f2ca5ee` | 238 | `0x4786c0e0` | 40 | 40, 88, 80, 24 | 9 (295) |
| Last Rites | `0x953d15fb` | 268 | `0x4786c0e0` | 32 | 32 | 8 (230) |
| Regicide | `0xc280b997` | 273 | `0x4786c0e0` | 8 | 8, 152, 136, 128, 104, 105 | 24 (966) |
| Dread Patrol | `0x7c3c011b` | 237 | `0x4786c0e0` | 8 | 8, 152, 72, 128 | 19 (752) |
| Crota's End | `0x6d7cc3bc` | 15 | `0x24c67333` | 16 | 16, 32 | 8 (417) |
| The Undying Mind | `0x030ef564` | 306 | `0xf29221f5` | 104 | 104, 200, 88 | 7 (477) |

The Speaker and The Awoken are the smallest. Their PAHs served the world block and one other group, about 22 records, so they are the cheapest first test of serving a mission besides A Guardian Rises.

**The Speaker is a cinematic world, served like cine_reveal.**
- **Its world.** It is `cine_speaker`: Blam Network's 2014 activity table gives scenario `0x80C3E011` and 4 bubbles. The owned 56578 record gives PRIVATE `0xA3198561`, and its name's FNV-1, `0x796B3B8C`, is the one extra group the live PAH served: a single cinematic record (`/6/0`, tag `0x8080254D`).
- **The live states.** The record went idle at the join, playing about 18 s later, and done when the mission ended (197 s), with the client in slice set 1 throughout.
- **How to serve it.** `world-table-speaker-live.txt` (activity 167, start region 1) serves it, and `d1-cine-watch.py RUN --activity 167 --slice 1 --states cine_speaker-play.txt cine_speaker-done.txt` steps it. The host needs a build with the `cine_speaker` world (`d1-activity-host`, `worlds.rs`).

**The Awoken is `act_cinematic_queen_101`.**
- **Its world.** Blam's row 206 gives scenario `0x80C02004` and 3 bubbles. The owned record gives PRIVATE `0x14CDA256` (cine_reveal's too). A Key Awaits is `act_cinematic_queen_102`, which shares cine_speaker's PRIVATE.
- **Its cinematics.** The live host served two records in group `0x4AF77757`. That is not the world name's FNV-1 (`0x9B5A0BBB`) but its second, unnamed bundle. They played one after the other, from slice set 17.
- **How to serve it.** `world-table-speaker-live.txt` also serves it (activity 186, start region 17). Step it with `d1-cine-watch.py RUN --activity 186 --slice 17 --states cine_awoken-play-0.txt cine_awoken-play-1.txt cine_awoken-done.txt`.

**In every live cinematic the play state's start is the host's frame stamp minus one:**
- The Speaker: `0xC7` at stamp 200;
- The Awoken: `0xE3` at stamp 228, and `0x423` at stamp 1060;
- flag 1 in all three.

Our host stamps every frame 0, so it cannot send "one tick before now". That is the first thing to test for cine_reveal's cinematic, which reaches its world but does not start ("Serving it", above).

## Next

- **Serve 165 as the live host did** (the two cinematic records, start region 25, the state steps), and retry it.
- **Bring up the one-host missions** (above), which need no public instance.
- **Restoration's public start, the rest of step 4:** the public instance's own world, its sensor groups (in Restoration `0xEAAF16E2`, the 16 player objects, and four others), and no region records. Its connection already gets its own host id (`D1_AH_DISTINCT_HOST_IDS=1`), and the PAH already advertises what the client reports (#270).
- **Map the incident types between 51539 and 56578,** from pairs the two clients send at the same moments (cinematic start and stop), or from the builds' incident tables.
- **Delta (`fresh = 0`) sensor updates, then field by field:** what the live host's values mean, for the records where ours differ (the squads' names, the trigger values, the objectives).
- **The Tower: too little of it here.** Only A Guardian Rises (40 s) and Eye of a Gate Lord (36 s) reach it, not enough for its vendor, civilian and Xûr records. Group 0x8CC31436, which earlier notes called the Tower plaza, also appears in The Archive and The Nexus.
- **Answer udt 25 and 26 as the live host did** ("Entity-slot handover"), and see in the lab whether it changes anything at bubble transitions.
- **Compare each mission's live host timeline with what our activity host serves,** A Guardian Rises first: its incidents, memberships and objective updates.
