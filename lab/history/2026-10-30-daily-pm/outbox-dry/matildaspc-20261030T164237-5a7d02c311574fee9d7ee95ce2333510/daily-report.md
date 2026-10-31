# 30 October 2026: the Tower opens its vendors

Candidate public update: Vendors are open in the Tower. Walk up to Executor Hideo of the New Monarchy, press □, and his screen opens just as it should: the allegiance offer, then his reputation and item exchange. Getting here took two fixes. The Guardian now stays in the Tower for as long as you play, and the Tower now loads as the city's social space, with its own menus and objectives, instead of acting as a stand-in for a story mission.

Private production input. This report authorises no publication, final-video edit or release. It replaces today's earlier package (same day, more evidence), which it supersedes.

## Reporting window and provenance

- Europe/London reporting day 2026-10-30. Coverage: 09:11Z to 16:45Z, runs sense63 to sense66.
- Build: PS4 56578 under the project's emulator fork, served by the project's own activity host and backend from the public source's main branch (commits below). No game content is in the package.

## What changed

1. **The Tower keeps the Guardian (run sense63).** The game's own idle timer counts button presses as well as movement, and walking alone didn't count. With a button press each minute, the Guardian stayed in Tower North from 10:07 to 12:03 UTC. As a control, all input was then stopped, and the game returned the Guardian to orbit exactly 15 minutes after the last press, its normal behaviour.
2. **The Tower is now its own place (run sense66).** Until today, the Tower ran as a stand-in for the next story mission, so it loaded the story menus, and those have no vendor screens. Presenting it as the Tower's own activity was refused for missing permission. The game's own rule for the Tower turned out to be "these two unlock flags must be clear", and the server had been setting both since early October work on the Director. With them no longer served, the game accepts the Tower as the city's social space: its menus load, and the Tower's own objective appears ("Armsday: speak to Banshee-44").
3. **Vendors open (run sense66).** At Executor Hideo (New Monarchy) the "□ Approach" prompt appears; pressing it opens the faction's allegiance offer and then the vendor screen, with reputation, the material exchange, a rank reward, the stock-refresh timer and item details.

## Tests

- sense63: unattended run through the mission and CONTINUE into the Tower; the Guardian stayed almost two hours, with an idle control. **PASS** for staying in the Tower.
- sense64: the Tower presented as its own activity with the old flag set: refused for missing permission. **Expected failure**, which led to the fix.
- sense66: the full unattended mission with the two flags no longer served. The mission ran normally (hangar fight cleared on its own), CONTINUE took the Guardian to the Tower as its own activity, and the game stayed. The game reported the social-space menus as loaded. **PASS**.
- sense66: Executor Hideo's vendor screen opened twice from □ Approach, and closed back to the Tower each time. **PASS** for one vendor. Other vendors (Banshee-44, the Vanguard, the Cryptarch, Xûr) were not tried yet.

## Maturity

- Staying in the Tower: **Working** (seen for almost two hours, with a control).
- The Tower as the social space: **Working** (seen: its menus and objective).
- Vendors: **Working for the one vendor tried** (Executor Hideo, seen on screen). Buying, reputation and the other vendors are untested.
- CONTINUE still takes the Guardian to the Tower in place of the next story mission (the scripted first Tower visit is not served yet).

## Evidence index

Both clips are continuous game-window captures, native 1920×1080 at about 30 fps, with game audio from the emulator's audio output (one port, no microphone). Each is cut from its recording without re-encoding or edits.

| Clip | Shows | Notes |
|---|---|---|
| clip01 (45 s) | sense63: the camera turns slowly around the Tower North courtyard at night | Lab input: a slow camera circle |
| clip02 (56 s) | sense66: the "□ Approach" prompt at Executor Hideo, the allegiance offer, the New Monarchy vendor screen with an item's details, and back to the Tower | Lab input: □, then ○ to dismiss the allegiance offer, a few cursor moves, ○ to close |

Screenshots (frames from the same captures):
1. The Tower North courtyard at night, with a vendor marker (sense63).
2. The stairs up from the courtyard (sense63).
3. Executor Hideo with the "□ Approach" prompt and the Armsday objective (sense66).
4. The New Monarchy vendor screen (sense66).

Proved visually: the Guardian in the Tower, the social Tower's objective, and a vendor screen opening and closing. Not shown visually: the two-hour stay (shown by the server log and the control) and the unlock-rule analysis (the game's own code and memory).

## Remaining issues and next steps

- Try the other vendors, and buying, in the social Tower.
- Make the corrected flag set the default, and serve the scripted first Tower visit (activity 165) for CONTINUE, so the social Tower follows it as in the original game.
- The Tower's plaza and hangar areas; the end-of-mission values are still synthetic.
- Newly logged for contributors: 60 fps and PC keyboard-and-mouse controls.

## Commits

Changes merged to the public source today (full SHAs):

- 243a9c5bdb9cba4ee4a5a05f61e808c787a1de37 docs: the Tower loads as its own social space; add 60 fps and PC controls (#114)
- facbebcc2be7c9da868e337ff6c62e1679e1be0d d1-bap: cover character lifecycle wire boundaries (#111), community contribution
- cffbeecf4cf0a78a232b235542bcb6670017b00a host: D1_AH_ACTIVITY_REMAP publishes another activity index (diagnostic) (#110)
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
- The character shown is the project's test character.

## Handoff state

2 clips (fully decoded at packaging) and 4 screenshots. Supersedes <lab-pc>-20261030T131040-ae037d882a7c44f582ab76765b8801cc.
