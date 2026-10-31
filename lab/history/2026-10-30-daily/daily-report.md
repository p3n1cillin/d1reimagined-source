# 30 October 2026: staying in the Tower, and the road to vendors

Candidate public update: The Tower no longer sends you back to orbit. In today's test the Guardian stayed in Tower North for almost two hours; before, the game returned it to orbit after a few minutes. Vendors are next. We traced exactly where opening a vendor stops, and it isn't missing shop data: the game is still running the Tower as a story mission, so it never loads the Tower's own menus.

Private production input. This report authorises no publication, final-video edit or release.

## Reporting window and provenance

- Europe/London reporting day 2026-10-30. Coverage: 09:11Z to 13:30Z, runs sense63 and sense64.
- Build: PS4 56578 under the project's emulator fork, served by the project's own activity host and backend from the public source's main branch (commits below). No game content is in the package.

## What changed

1. **The Tower keeps the Guardian (run sense63).** The game's own idle timer counts button presses as well as movement, and walking alone didn't count, so the Guardian was returned to orbit. With a button press each minute, the Guardian stayed in Tower North from 10:07 to 12:03 UTC. As a control, all input was then stopped, and the game returned the Guardian to orbit exactly 15 minutes after the last press, its normal behaviour.
2. **The full run is reliable again.** The lab bot's hangar fight, which failed last night, passed. Its tools no longer leave a button held between steps; a held button had made the Guardian use its Super by accident.
3. **Vendors traced to one cause.** Pressing □ at the emblem kiosk ("Access Emblems") runs the game's vendor code most of the way. The vendor is found, and then the screen can't be built: the Tower is running under the story-mission menus, and the vendor screens live in the social-space menus. A test that told the game it was in the Tower's own activity was refused, because the character doesn't yet have permission for it. That permission is the next step.

## Tests

- sense63: unattended run from boot through the mission, the hangar fight, the ending, the end-of-mission screens and CONTINUE into the Tower (10:01Z). The Guardian stayed in the Tower for almost two hours. **PASS** for staying in the Tower.
- sense63: emblem kiosk interaction traced in the running game. The vendor screen is still **not open**.
- sense64: the Tower presented as its own activity. The game refused it (missing permission) and returned the Guardian to orbit. **Expected failure**; it identifies the next requirement.

## Maturity

- Staying in the Tower: **Working** (seen for almost two hours, with a control).
- Vendors: **In progress**. The cause is found; the fix is not built.
- Travel to the Tower: unchanged since 29 October. CONTINUE still takes the Guardian to the Tower in place of the next story mission.

## Evidence index

The clip is a continuous game-window capture of run sense63 (30 Oct 2026), shortly after arrival in the Tower. Native 1920×1080 at about 30 fps, with game audio from the emulator's audio output (one port, no microphone). It's cut from the recording with no re-encode or edits.

| Clip | Shows | Notes |
|---|---|---|
| clip01 (45 s) | The camera turns slowly around the Tower North courtyard at night; vendor markers and the waypoint are visible | Lab input: a slow camera circle, no walking |

Screenshots (frames from the same capture):
1. The Tower North courtyard at night, with a vendor marker.
2. The stairs up from the courtyard.

Proved visually: the Guardian in Tower North. Not shown visually: that it stayed for almost two hours (shown by the server log and the control), and the vendor trace (shown in the game's own code, not on screen).

## Remaining issues and next steps

- Find what gives the character permission for the Tower's own activity, and serve it, so the Tower runs with its own menus and vendors can open.
- The other Tower areas (the plaza, the hangar) and the story cinematic before the Tower.
- The end-of-mission values are still synthetic.

## Commits

Changes merged to the public source today (full SHAs):

- ebaeab778f990e893f3cbfc0e8e15ce7bc6b9486 notes: staying in the Tower, and where vendor screens stop (#109)
- ab18afbd45ddea2bb13cea06c5f431ea7f486429 tools: one path contract instead of hardcoded local paths (#102)
- f7f2166b83d2b32f1a3762b6d7136731a3bc975f 56578: derive primary item stats through owned level curves (#51)
- 20de721f385f216860ae6e0b2bb9e5bd190108e9 test(d1-bap): cover matchmaking protobuf boundaries (#103), community contribution
- 94d486cd5a5cb585276df99b6628008a3901398a ci: run Python unit tests on Windows alongside Linux (#104), community contribution
- e2abc7b32731bfddd2d339c41895017a40f76f72 test(d1-bap): add loot claim and item-grant round-trip coverage (#90), the first community contribution
- 5ebc598f5615b732e8eec5ba81017fee9bea6452 docs: contributor checkpoint for 30 October (#96)
- 9080d53903a3ed45c55a741f3db839aa78df5c2b ci: a pre-review report and lint on added lines (#106)

## Privacy, audio and rights

- Game window capture only; no overlays, personal identifiers or desktop.
- Game audio comes from the emulator's own audio output (no microphone). The game's music may be present and is flagged for review.
- The character name shown is the project's test character.

## Handoff state

1 clip (fully decoded at packaging) and 2 screenshots.
