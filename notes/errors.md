# Destiny error database

Scope item 39 of [`preservation-scope.md`](preservation-scope.md). One row per error the
preservation stack has hit, with what triggers it and how it was resolved. Build = where it was
observed (01.24 unless noted). Origin: CLIENT, BACKEND (a stand-in reply), NETWORK, ACTIVITY
(activity host / world state), SESSION, MATCHMAKING. "Failure (a, b)" = the client's internal
failure step/code where it was traced. Add rows as new errors appear; keep the source note link.

| Error | Build | Shown when | Cause (OBSERVED unless marked) | Fix / status | Origin | Source |
| --- | --- | --- | --- | --- | --- | --- |
| **buffalo** | 01.00/01.24 | startup | PSN check unreachable: RPCN's UDP helper bound to `[::1]` only, IPv4 probe unanswered | RPCN on IPv4 (`rpcn_local_account.py`) - fixed | NETWORK | `ps3-route.md`, `test-log.md` |
| **marionberry** | 01.00/01.24 | startup / after Verifying Content | Demonware STUN (`dev-stun.demonware.net:3074`) unanswered; also seen with a disc-only manifest (160 packages, no BAP) | `stun_stub.py` answers IP/NAT discovery - fixed for 01.00; 01.24 STUN reply format still unknown | NETWORK | `ps3-route.md`, audit checkpoint |
| **termite** | 01.24 | content manifest step | title configuration without the content-manifest URL (SignOn binary config) | SignOn field 13/14 config with URL - fixed | BACKEND | `ps3-route.md`, `final-build.md` |
| **nightingale** | 01.00 | manifest fetch | manifest request answered 503 | serve the manifest - fixed | BACKEND | `ps3-route.md` (test 019) |
| **cattle** | 01.00 (lobby) | Demonware lobby auth | `client_auth` frame left unanswered (~10 s) | `lobby_probe.py` answers - fixed (01.24 has no lobby step) | BACKEND | `ps3-route.md` |
| **centipede** | 01.00/01.24 | after BAP sign-in | BAP replies out of order: an unanswered request (acct id trans) made later replies unmatched; failure (0x18, 0xa0) | answer requests oldest-first (`acct_id_trans_reply`) - fixed | BACKEND | `bap_server.py` comment, `ps3-route.md` |
| **chicken** | 01.00 | investment sign-in | sign-in (service 503) failure (0x18, 0xa1) from `0x32d070` | typed sign-in response - fixed | BACKEND | `ps3-route.md` |
| **pigeon** | 01.24 | profile fetch | qz object type hashes differ per build | build-specific type hashes - fixed (2026-10-01) | BACKEND | `checkpoint-2026-10-01-profile.md` |
| **weasel** | 01.24 | after profile setup / user message | user-msg reply (0x21) missing its required int field 4 (decoder `0x47bf9c` / `0xbfbe0c`) | reply `20 00` - fixed (PROFILE4) | BACKEND | `checkpoint-2026-10-01-profile.md` |
| **currant** | 01.00 | activity join | `ahp req` (activity host assignment) answered with zeros; failure (0x18, 0x21) from `0x333050` | `--ahp-reply` with a host record - fixed | MATCHMAKING | `test-log.md` 078-080 |
| **newt** | 01.24 | after the prologue | `mm req` (fireteam matchmaking) answered empty for ~90 s | `--mm-rsp 4=` (match submessage) - fixed | MATCHMAKING | `checkpoint-2026-10-01-profile.md` |
| **urchin** | 01.24 | activity launch | "You do not have access to this activity": activity access rules require universe flags 3390 and 309 | `--universe-flag-values 3390=1,309=1` - fixed | BACKEND | `checkpoint-2026-10-01-profile.md` |
| **lime** | 01.00/01.24 | loading activity resources | activity checksum mismatch (state @0x5c vs scenario; 01.24 highway `0x8fe07299`), or wrong map/world | build-specific checksum and world - fixed | ACTIVITY | `checkpoint-2026-10-01-profile.md` |
| **lobster** | 01.00/01.24 | in world | leave condition: engine state 12/13 because the requested slice set's scenario record is absent (status 5; reason 0x10c in 01.24) - e.g. a bubble whose state was -1, or a zone change to an unregistered set | register the bubbles (`--ah-bubble-states`) - fixed; reappears if a script targets an unregistered set | ACTIVITY | `checkpoint-2026-09-25-orbit.md`, `checkpoint-2026-10-01-profile.md` |
| **baboon** | 01.00/01.24 | in world | "Removed from the game world": no host message within the timeout (host status 36, reason 0x67) | heartbeats (`--ah-heartbeat`) - fixed; a scheduler bug once delayed them (TEST361) | ACTIVITY / SESSION | `checkpoint-2026-09-25-orbit.md` |
| **pumpkin** | 01.24 | entering The Breach (JOIN81/82/88) | "Your connection to the Destiny servers has been lost". JOIN88: cleanup stage (bootflow 0x19) posts **reason 0x4d** (`0x355d54`, called from `0x356b9c`), about 3 minutes after the client requests slice set 24. Final-build leave-condition 32 goes through `0x35f50c -> 0x35dd14`; table `0x12f9d9c` maps session statuses **9, 37 and 38** to this reason. The 01.00 failure name suggests an initialization timeout but does not identify the final-build waiting state. Incident echo and trigger disarm did not resolve it. | Open: sample the actual session status and its producer during handover. Do not infer a unique timeout or missing incident acknowledgement from reason 0x4d alone. | ACTIVITY | `checkpoint-2026-10-02-mission.md` |
| **bass** | 01.00 | in world | host type 43 boot list contains the client's machine id (moves it to state 4) | do not list the client in type 43 - fixed | ACTIVITY | `checkpoint-2026-09-25-orbit.md` |

## Non-coded failure states (no error screen)

| Symptom | Build | Cause | Status | Source |
| --- | --- | --- | --- | --- |
| Empty black world after moving from 112 toward 24 | 01.24 | game-mode activity quarantine defaulted to bubble 0, watchdog requested slice 0 | fixed: quarantine bubble -1 (JOIN27) | profile checkpoint |
| Crash (access violation at `0x2160b4`) when a cinematic starts | 01.24 | unset non-optional object reference encoded as {0, 0, 0} | fixed: null object references (JOIN32) | profile checkpoint |
| Black screen with loading glyph after the 113 -> 112 hand-off | 01.24 | teleport finish sent as a new world-state version, the engine re-read group record 1 and restarted the switch | fixed (JOIN51) | profile checkpoint |
| Black screen with loading glyph after a fast 113 -> 112 hand-off | 01.24 | the client reported 112 as current before its state-4 report; the stand-in sent the teleport finish (state 3) and dropped the request 50 ms later, so the respawn on state 3 never ran (JOIN61) | fix: keep the finished request (state 3) for `TELEPORT_SETTLE_SECONDS` (10 s) after it is first sent, then drop it (fix in place; JOIN62 used the normal-speed path, the fast path has not recurred yet) | profile checkpoint |
| Black screen after a cinematic that ended on its own | 01.24 | fade-to-black at the end of `pf_104_res_fp` not lifted | worked around: the script stops the cinematics before the teleport (JOIN53); lifting the fade itself not investigated | profile checkpoint |
| "STILL THERE? You have been returned to Orbit because you were inactive." | 01.24 | client-side inactivity timer: no pad input for several minutes in a world (JOIN63, during static analysis) | expected behaviour; traversal/automation keeps input alive | profile checkpoint |
| Title screen "PRESS X TO PLAY" never left, "no sign-in within 8 minutes" | 01.24 | slow boot: the title appeared after the join wrapper stopped pressing Cross (110 s) (JOIN64) | fixed: `Invoke-Final124Run.ps1` keeps pressing Cross past `-PressSeconds` while no SignOn request has arrived (JOIN69 hit it again) | profile checkpoint |
| Emulation frozen: `SIG: Thread terminated due to fatal error: Unimplemented FP CAL instruction` (RSX, `FPToCFG.cpp:193`) | 01.24 | RPCS3's fragment-program CFG pass throws on CAL (unused by the RSX compiler, i.e. broken ucode) - hit when the player reached a lower area south-east of the 112 spawn (396, -969, z -8) (JOIN65) | fixed: `patches/rpcs3-fp-cal-ignore.patch` (log and continue, as the main decompiler does), built 2026-10-02 with `<D1_ROOT>\main-pc-import\dev\build-d1.cmd` and deployed to `RPCS3_FINAL124_SMOKE001` (previous binary kept as `rpcs3.before-fpcal.exe`) | emulator |
| Urgent Message / Message of the Day dialogs on world entry | 01.00/01.24 | data source unknown (not the user-msg reply) | INVESTIGATING | profile checkpoint |
| Frames stop at character select; all threads stop during the 113 -> 112 load | 01.24 | intermittent emulator stall: no error, CPU busy, PPU threads stop logging (JOIN29; JOIN55 20 s after the teleport request, with 6 PPU hooks; JOIN72 early boot; JOIN77 in the Breach at ~11 GB process memory, no hooks) | open - rerun; candidates: emulator memory growth, RSX stall | profile checkpoint |
| Vulkan "Device lost" in slice 113 | 01.24 | GPU driver loss, not reproduced (JOIN33) | open | profile checkpoint |
