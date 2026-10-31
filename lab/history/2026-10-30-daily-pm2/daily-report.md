# 30 October 2026: the Tower opens its vendors

Candidate public update: Vendors are open in the Tower. Walk up to Executor Hideo of the New Monarchy or Eva Levante, the Guardian Outfitter, press □, and their screens open just as they should. Hideo's shows the allegiance offer, then his reputation and item exchange. Eva's shows her emblems, shaders and ornaments, with each item's price and requirements, and the time until her stock refreshes. Getting here took two fixes. The Guardian now stays in the Tower for as long as you play. The Tower also now loads as the city's social space, with its own menus and objectives, instead of acting as a stand-in for a story mission. Buying is next: the screens work, but the test Guardian has no Glimmer yet.

Private production input. This report authorises no publication, final-video edit or release. It replaces today's earlier packages (same day, more evidence) and supersedes the 16:42 UK package.

## Reporting window and provenance

- Europe/London reporting day 2026-10-30. Coverage: 09:11Z to 19:37Z, runs sense63 to sense66.
- Build: PS4 56578 under the project's emulator fork, served by the project's own activity host and backend from the public source's main branch (commits below). No game content is in the package.

## What changed

1. **The Tower keeps the Guardian (run sense63).** The game's own idle timer counts button presses as well as movement, and walking alone didn't count. With a button press each minute, the Guardian stayed in Tower North from 10:07 to 12:03 UTC. As a control, all input was then stopped, and the game returned the Guardian to orbit exactly 15 minutes after the last press, its normal behaviour.
2. **The Tower is now its own place (run sense66).** Until today, the Tower ran as a stand-in for the next story mission. That loaded the story menus, which have no vendor screens. When the Tower was presented as its own activity instead, the game refused it for missing permission. The game's rule for the Tower turned out to be "these two unlock flags must be clear", and the server had been setting both since early October. With them no longer served, the game accepts the Tower as the city's social space: its menus load, and the Tower's own objective appears ("Armsday: speak to Banshee-44").
3. **Vendors open (run sense66).** At Executor Hideo (New Monarchy), the "□ Approach" prompt appears. Pressing it opens the faction's allegiance offer and then the vendor screen, with reputation, the material exchange, a rank reward, the stock-refresh timer and item details.
4. **A second vendor, with her full stock (run sense66, evening).** Eva Levante's Guardian Outfitter screen opens from "□ Approach". It lists 18 emblems, 6 shaders and 4 ornaments. Each item's details show its rarity, level requirement and price (for example, a shader that requires level 20 and costs 2,250 Glimmer, and a Chroma material that costs Silver Dust). The screen also shows the stock-refresh timer, and it closes back to the Tower.

## Tests

- sense63: unattended run through the mission and CONTINUE into the Tower; the Guardian stayed almost two hours, with an idle control. **PASS** for staying in the Tower.
- sense64: the Tower presented as its own activity with the old flag set: refused for missing permission. **Expected failure**, which led to the fix.
- sense66: the full unattended mission with the two flags no longer served. The mission ran normally (the hangar fight cleared on its own), CONTINUE took the Guardian to the Tower as its own activity, and the game stayed. The game reported the social-space menus as loaded. **PASS**.
- sense66: Executor Hideo's vendor screen opened twice from □ Approach and closed back to the Tower each time. **PASS**.
- sense66: Eva Levante's Guardian Outfitter screen opened twice, showed her stock and item details, and closed back to the Tower. **PASS**.
- sense66: Buying an emblem from Eva. The game itself refused the purchase because the test Guardian has 0 Glimmer, and nothing was sent to the server. **Not yet possible**: the server doesn't give characters any currency yet.
- sense66: Patches 99-40, the Research Assistant. The screen opens, but it stays on its loading indicator. The game sent the server nothing new while it waited. **FAIL, under investigation.**
- Banshee-44 (the Armsday objective) was not reached: the route from the Tower North rooms to his stall wasn't found in the time available.

## Maturity

- Staying in the Tower: **Working** (seen for almost two hours, with a control).
- The Tower as the social space: **Working** (seen: its menus and objective).
- Vendors: **Working for two vendors** (Executor Hideo and Eva Levante, seen on screen). One more (Research Assistant) opens but doesn't finish loading. Buying is blocked until characters have currency. Reputation and the other vendors are untested.
- CONTINUE still takes the Guardian to the Tower in place of the next story mission (the scripted first Tower visit is not served yet).

## Evidence index

All clips are continuous game-window captures, native 1920×1080 at about 30 fps, with game audio from the emulator's audio output (one port, no microphone). Each is cut from its recording without re-encoding or edits.

| Clip | Shows | Notes |
|---|---|---|
| clip01 (45 s) | sense63: the camera turns slowly around the Tower North courtyard at night | Lab input: a slow camera circle |
| clip02 (56 s) | sense66: the "□ Approach" prompt at Executor Hideo, the allegiance offer, the New Monarchy vendor screen with an item's details, and back to the Tower | Lab input: □, then ○ to dismiss the allegiance offer, a few cursor moves, ○ to close |
| clip03 (41 s) | sense66: the "□ Approach" prompt at Eva Levante, the Guardian Outfitter screen with her emblems, shaders and ornaments, item details for an emblem, two shaders and a Chroma material, the stock-refresh timer, and back to the Tower | Lab input: □, cursor moves, ○ to close. The Guardian preview on the left is a silhouette with a loading indicator |

Screenshots (frames from the same captures):
1. The Tower North courtyard at night, with a vendor marker (sense63).
2. The stairs up from the courtyard (sense63).
3. Executor Hideo with the "□ Approach" prompt and the Armsday objective (sense66).
4. The New Monarchy vendor screen (sense66).
5. Eva Levante's Guardian Outfitter with an emblem's details and price (sense66).
6. The Guardian Outfitter with a Chroma material's details and the stock-refresh timer (sense66).

Proved visually: the Guardian in the Tower, the social Tower's objective, and two vendor screens opening, showing their stock and closing. Not shown visually: the two-hour stay (shown by the server log and the control), the unlock-rule analysis (the game's own code and memory), and the refused purchase (no on-screen change).

## Remaining issues and next steps

- Give characters currency, so buying can be tried; then serve the purchase request and the item it grants.
- Find what the Research Assistant's screen waits for.
- Reach Banshee-44 and the other vendors.
- Serve the scripted first Tower visit (activity 165) for CONTINUE, so the social Tower follows it as in the original game.
- The Tower's plaza and hangar areas; the end-of-mission values are still synthetic.
- Logged for contributors: 60 fps and PC keyboard-and-mouse controls.

## Commits

Changes merged to the public source today (full SHAs):

- e58c9d2404bfc5363e040b40b4e9675f3fff9473 d1-oracle: skip tests when private inputs are absent (#116), community contribution
- 167febf7ebc2a52f480e4e05ae960d1e2edba57b d1-bap: cover activity-host notification wire boundaries (#117), community contribution
- fe7a3e146b90403d11aca5a730051656aed889ed 56578: stop serving unlock flags 308 and 3391; the Tower requires both clear (#115)
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
- The character shown is the project's test character ("D1Guardian", visible in item details).

## Handoff state

3 clips (fully decoded at packaging) and 6 screenshots. Supersedes <lab-pc>-20261030T164246-78853df8364e4dadae6c6a800905aa30.
