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

- The ending cinematic after the hangar: **Partially Working**. It plays and completes. The hangar clear uses an experimental subset of the enemy groups, because two groups spawn out of reach; under investigation.
- Mission completion and post-game screens: **Partially Working**. They display. The values are synthetic, rewards are not granted, and CONTINUE is blocked.
- Travel after the mission (orbit or the Tower): **Planned**. The world data for the Tower has been identified; not yet tested.

## Evidence index

To be completed at packaging, with clip timecodes:
1. Dock 13 walk.
2. The hangar fight.
3. The outro cinematic (full).
4. The post-game screens tour.

Screenshot candidates: the Guardian close-up in the outro, the jumpship lift-off, ACTIVITY REWARDS with the filled row, and MISSION SUMMARY.

## Remaining issues and next steps

- CONTINUE and the loading indicator: find what the post-game screen still waits for. Acceptance: CONTINUE leaves the screens.
- The reward slots and XP: the reward grant (the jumpship) and experience. Acceptance: the rewards column shows a real reward that also lands in the inventory.
- Travel to the next activity or the Tower. Acceptance: the Guardian arrives in the Tower without a crash.
- The two hidden enemy groups in the hangar.

## Privacy, audio and rights

- Game window capture only. The capture includes the in-game HUD, and the border must be cropped. No overlays, personal identifiers or desktop.
- Game audio, where present, comes from the emulator's own audio output (no microphone). The game's music may be present and is flagged for review.
- The character name shown is the project's test character.

## Handoff state

To be completed at packaging.
