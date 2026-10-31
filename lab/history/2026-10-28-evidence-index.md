# Evidence after the 27 Oct 19:00 cutoff (for the next eligible edition)

Times are file-modification times in UTC (= UK time this week). Paths are relative to `cfw-staging`. Private: never in a repo. These are candidates only. The daily edition's own policy decides what is packaged; nothing here has been packaged or sent.

| time (UTC) | file | what it shows |
|---|---|---|
| 21:18 | `divide23/clips/fight2a.mp4` | Fighter v2 at drop2fall; the Skiff turret kills the Guardian ("Guardian Down: Skiff, House of Devils") |
| 21:22 | `divide23/clips/sprint1.mp4` | Sprint from drop2fall through the exit and the gate to the locker room (138, -610) |
| 21:24 | `divide23/clips/super1.mp4` | Striker Super cast at the Divide exit, then the sprint through the gate |
| 21:30 | `divide23/clips/follow1.mp4` | First waypoint-follower attempt |
| 21:36 | `divide23/clips/follow2.mp4`, `follow2b.mp4.video.mp4` | Follower with the touchpad reveal (video-only file for 2b) |
| 21:42 | `divide23/clips/follow3.mp4` | Follower lines up on the waypoint inside the interior before the ledge |
| 21:46 | `divide23/clips/follow4.mp4` | Follower jumps the ledge and reaches the gate (144.7, -607.8) |
| 21:48 | `divide23/clips/find-the-ship-nav.png` | **First native "FIND THE SHIP" objective** in nav mode (objective 20, marker 52, served by our host) |
| 21:51 | `divide23/clips/find-ship1.mp4` | Following the Find the Ship waypoint toward the gate |
| 21:56 | `divide23/clips/find-ship2.mp4` | The same, with nameplate combat enabled |
| 23:18 | `weapon26-base-stats/clips/fight1.mp4` | Rifle at 25 / 184 (Codex's full weapon table); follower from the Divide entry |
| 23:24 | `weapon26-base-stats/clips/fight2.mp4` | **Hit evidence**: one ADS burst takes a level-1 Vandal from a full bar to about 10% (frames `fight2/fo-031.png`, `fo-032.png`) |
| 23:30 | `weapon26-base-stats/clips/fight3.mp4` | Follower stuck on a raised structure after a respawn |
| 23:56 | `weapon26-base-stats/clips/cycles.mp4` | 25 min of cycles on stale tracking (little progress) |
| 00:19 (28 Oct) | `weapon26-base-stats/clips/cycles-b.mp4` | 20 min of cycles: the explorer chain to drop2fall repeatedly; deaths at the gate |

Headline for the edition: Find the Ship presented natively; the rifle's magazine fixed to 25 (Codex, PR #46); first measured damage on a Vandal; the reserve (134 vs 292) and armour (Defense 0 vs the reference's 59) still open.

## Codex continuation, 28 Oct 05:27–08:00 UK

Source55/56 merged; source57 remains draft; notes58 merged. Decoder accepted814 private udt6 payloads across sense30/31/33/34/35. No verified enemy kill, objective23 completion or reserve/subclass correction. Native tables confirmed diagnosticDefense1200/Attack200, including after the final respawn. B+0C native predicate loop is a new static lead, with byte-identity controls and explicit semantic limits.

New runs: sense32 tool-induced delayed breakpoint crash; sense33 later GPU access violation after fixed probes (not a clean run); sense34 owned stop06:58:52 after Currant recovery/failed post-respawn route test; sense35 fresh readiness-gated unattended replay to Divide, then two combat controls with no accepted kill and Guardian death07:29:56. sense35 was left live for08:00 hand-back, guard active, no Codex movement/debug observer. Check final hand-back receipt for delivery.

Evidence root: 2026-10-28-sense-codex. See late-decode-summary.json, late-static-native-controls.json, sense35-handback-native-state.json and cfw-staging run/contained receipts. Source notes contain derived findings only. Media/packets/decoded game images remain private. No new NAS package or final-video edit was made during this continuation; use this as evidence for the next eligible consolidated edition with actual timestamps.
