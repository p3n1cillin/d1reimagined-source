# 29 October 2026: A Guardian Rises reaches its ending and mission summary

Candidate public update: The opening mission now plays through to its ending. After the hangar fight, the closing cinematic plays in full, and the game moves on to its end-of-mission screens: Activity Rewards and the Mission Summary, filled in for our Guardian. The last step, leaving the screens and travelling on, is still being worked on.

Private production input. This report authorises no publication, final-video edit or release.

## Reporting window and provenance

- Europe/London reporting day 2026-10-29. Engineering coverage runs from 2026-10-28T19:00Z (after the previous report) through the capture time stated in metadata.
- Events keep their real UTC times. The previous daily report (28 Oct) is referenced, not resent.
- Build: PS4 56578 under the project's emulator fork, served by the project's own activity host and backend from the public source's main branch (commit in metadata). The mission steps come from a private schedule; no game content is in the package.

## What changed

1. **The ending plays.** After the hangar is cleared, the server moves the player to the outro area and starts the ending cinematic, which plays its full 84 s and reports completion (first seen 28 Oct 20:13Z; fully scheduled from 21:17Z on every run since). Previously the mission stopped at "Clear Out the Fallen".
2. **The mission completes.** The server sends the character's completed-activity record. The client accepts it and selects the next activity in the story chain. Previously, after the cinematic, the screen stayed black.
3. **End-of-mission screens.** The server switches the activity into its post-game state, and the client shows ACTIVITY REWARDS (29 Oct 00:32Z). From 09:42Z the server also fills the scoreboard, so the screens show our Guardian's row: kills, precision kills and deaths, plus the MISSION SUMMARY with the mission name, location and time.
4. **Combat.** The lab's test pilot now clears the hangar fight, including Rahn, the Devil Captain, in about 2 minutes, after an aiming fix. Lab tooling only, not game functionality.

## Tests

- Runs sense44 to sense51, each from boot.
  - Expected: hangar clear → outro → completion → post-game screens.
  - Observed: the ending sequence on every completed run from sense45 on. Rewards rows filled in sense50 and sense51. **PASS** for the outro and the screens' first state.
- **CONTINUE** on the post-game screen does not respond yet, and a loading indicator stays on screen. **FAIL** (known; the next task).
- The scoreboard values (50 kills, 15 precision kills, 2 deaths) are synthetic stand-ins, not the run's real statistics. The time shown is the real session time.

## Maturity

- The hangar encounter: **Partially Working**. All eight Fallen groups now clear (run sense53). Two of them only leave their hidden spawn after the server assigns them to the encounter's first task; that assignment is a diagnostic inference, not yet the original mission's recovered script.
- The ending cinematic after the hangar: **Fully Working (tested scope)**. It plays to completion with no skip input.
- Mission completion and post-game screens: **Partially Working**. They display. The values are synthetic, rewards are not granted, and CONTINUE is blocked.
- Travel after the mission (orbit or the Tower): **Planned**. The world data for the Tower has been identified; not yet tested.

## Evidence index

Clips 1–3 come from one continuous game-window capture of run sense53, and clip 4 from run sense54, an identical unattended run (29 Oct 2026). Capture started about 12:02:00Z on arrival in Dock 13. Native 1920×1080 at about 30 fps, with game audio from the emulator's audio output (one port, no microphone). Re-encoded H.264/AAC cuts with no edits inside a clip.

| Clip | Source window (UTC) | Shows | Notes |
|---|---|---|---|
| clip01 (80 s) | 12:02:00–12:03:20 | Arrival in Dock 13 and the walk through its corridors | The lab test pilot is playing |
| clip02 (85 s) | 12:17:45–12:19:10 | The final minute of the hangar fight: Rahn, the Devil Captain, under fire and defeated. All eight Fallen groups clear at 12:19:05Z | Combat is the test pilot |
| clip03 (95 s) | 12:19:45–12:21:20 | The full outro cinematic (start 12:19:49.7Z, completion 12:21:14.2Z), ending on the hooded figure | No skip input |
| clip04 (75 s) | 13:12:48–13:14:03, run sense54 | The post-game screens after the server's post-game step (13:12:49.6Z): ACTIVITY REWARDS with our row and the **jumpship reward** shown in the REWARDS column, then MISSION SUMMARY and ADVANCEMENT | The reward is a display entry; the item grant is a separate, not-yet-tested step. Loading indicator remains; CONTINUE not yet functional |

Screenshots (frames from the same capture):
1. Rahn under fire.
2. The Guardian and Ghost in the outro.
3. The jumpship and the Guardian in the hangar.
4. ACTIVITY REWARDS with the jumpship reward (sense54).
5. MISSION SUMMARY ("A Guardian Rises, Old Russia, Earth", time 31:38).

The same-day morning run (sense51) produced equivalent post-game footage and is superseded by this capture. Earlier footage (28 Oct 20:13Z, 29 Oct 00:32Z) was video only and is not included.

Proved visually: the hangar encounter clears, the cinematic plays to its end, and the post-game screens appear with server-provided values. Inferred, not shown: that the statistics match a real play session (they are synthetic) and the backend completion state (verified in engineering logs).

## Remaining issues and next steps

- CONTINUE and the loading indicator: find what the post-game screen still waits for. Acceptance: CONTINUE leaves the screens.
- The reward slots and XP: the reward grant (the jumpship) and experience. Acceptance: the rewards column shows a real reward that also lands in the inventory.
- Travel to the next activity or the Tower. Acceptance: the Guardian arrives in the Tower without a crash.
- The release of the two hidden hangar groups: replace the diagnostic assignment with the original mission's.

## Privacy, audio and rights

- Game window capture only. The capture includes the in-game HUD, and the border must be cropped. No overlays, personal identifiers or desktop.
- Game audio, where present, comes from the emulator's own audio output (no microphone). The game's music may be present and is flagged for review.
- The character name shown is the project's test character.

## Handoff state

This single consolidated 29 Oct report: 4 clips (all fully decoded at packaging) and 5 screenshots. The previous day's report is referenced, not resent. Later results today roll into the next edition.
