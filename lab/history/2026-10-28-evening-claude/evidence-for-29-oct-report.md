# Evidence after the 28 Oct 19:00 cutoff (rolls into the 29 Oct daily report)

Actual event dates are 28 Oct 2026 (UTC = UK time, GMT). Nothing here has been packaged or sent.

## Milestone: A Guardian Rises outro cinematic plays on 56578

- sense44 (host from public main 981e0f6, private schedule with cohort 5–10 as an experiment):
  - 20:11:14Z: hangar cohort clear → region 105 teleport.
  - 20:13:04/08Z: manual register/play of 0x992bb471/6/29.
  - 20:13:09Z: incident 2681 (start). 20:14:33Z: incident 891 (complete, 84 s, no skip).
- sense45 (same host, schedule with automatic Phase B):
  - 21:14:22Z: bay. 21:16:20Z: cohort clear (2 min of combat).
  - 21:17:00/04Z: automatic register/play. 21:17:04Z: 2681. 21:18:28Z: 891.
- **Clip candidate:** `cfw-staging/sense44/capture/outro-cinematic.mp4`. 150 s, 1920x1080, about 29.6 fps (avg_frame_rate 266760/9011), video only (NO audio; the audio tap wasn't enabled). Not yet decode-verified end to end. The useful range is about 0:00–1:30 from the recording start (20:13:15Z).
- **Screenshot candidates:** `outro-1.png` (Guardian close-up, letterboxed) and `outro-4.png` (jumpship lifting in the hangar), in this folder. Window grabs at 1936x1119 with a black border. Crop before use; no HUD or overlays.
- `cfw-staging/sense45/capture/after-outro-phases.mp4`: black screen during the phase test. Not public material.

## Maturity

- Outro cinematic after the hangar clear: **Partially Working**. It plays and completes. The clear cohort is an experiment (squads 11/12 unreachable). No mission summary or orbit after it yet (**Known Broken**: black screen).
- Lifetime phases 4/5/7/8/9/10 after the outro: tested, **no effect**.
- Bot aim fix (eye height): the bot killed the remaining hangar squads in 2 minutes. Lab tooling only, not game functionality.

## Public PRs

- #69 notes (merged c9ab34a6f7).
- #70 replay fan nudge.

## Later the same night (28/29 Oct, after 21:30 UTC)

- **sense47/48/49:** the whole route runs unattended from the Divide to the outro (Phase B is now scheduled). The PR #70 replay nudge merged.
- **Completion.**
  - PR #72 (merged): the responder pushes a profile on demand.
  - The completed profile (field [6] activity 164 completed, plus progressions 117/118/124/249/250 at zero) is accepted, and the client selects chain activity 165 (per Codex).
  - Without the progressions the client crashed (sense47).
- **ACTIVITY REWARDS, 29 Oct 00:32Z (sense49).**
  - After the outro, profile push, and lifetime phase 7 (PR #73 `@phase`, plus a one-row workaround for the padding bug fixed in PR #74), the client shows the ACTIVITY REWARDS screen.
  - The rows still show a loading spinner and CONTINUE doesn't respond: **Partially Working**.
  - **Clip candidates:** `cfw-staging/sense49/capture/postgame.mp4` (300 s from about 00:31:30Z; the screen appears at about 00:32:50Z, i.e. about 1:20 in) and `continue.mp4` (150 s). Video only, no audio, not decode-verified.
  - **Screenshot:** `rewards-screen.png` in this folder. A window grab; crop the border.
- The `currant` boot error appears intermittently during boot (sense44, sense49); the launcher dismisses it and the run recovers.
