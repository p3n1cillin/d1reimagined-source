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
| 5 (the simulation and sensor update) | not pinned in `d1-oracle`'s call sites yet |
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

## Next

- **Map the incident types between 51539 and 56578,** from pairs the two clients send at the same moments (cinematic start and stop), or from the builds' incident tables.
- **Pin udt 5, 23 and 24 in `d1-oracle`.** `d1-oracle activity-host decode-batch` (stdin `UDT HEX` lines, JSON out) decodes a whole mission in seconds; one start costs about 3 s.
- **Compare each mission's live host timeline with what our activity host serves,** A Guardian Rises first: its incidents, world states and objective updates.
