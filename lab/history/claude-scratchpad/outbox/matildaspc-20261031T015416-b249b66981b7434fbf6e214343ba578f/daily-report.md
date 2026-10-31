# 31 October 2026: buying from a vendor

Candidate public update: You can now buy things in the Tower. At Eva Levante, the Guardian Outfitter, the Guardian bought two emblems with Glimmer. The game sent its purchase, the Glimmer went down by the price, the game announced the new emblem, and the emblem was marked as owned. Getting there meant finding where the game keeps Glimmer and how its price check counts it, and reading every vendor's stock and prices from the game's own data. The server now handles a purchase entirely by itself. Also new: the PC mouse moves the game's menu cursor and turns the camera in a match, and the 60 fps clock runs at the game's normal speed.

DRAFT (prepared 30 Oct, late). Private production input. This report authorises no publication, final-video edit or release.

## Reporting window and provenance

- Europe/London reporting day 2026-10-31. Evidence from late on 30 October (after the day's last production pass) and from 31 October. Runs sense67 to sense72, plus emulator test runs emu73 to emu78.
- Build: PS4 56578 under the project's emulator fork, served by the project's own activity host and backend from the public source's main branch (commits below). No game content is in the package.

## What changed

1. **Glimmer reaches the game.** The game keeps Glimmer in the account's own inventory, in one fixed slot. Its price check sums that slot for the Glimmer item. With 25,000 Glimmer served there, Eva's emblems show as affordable, with "✕ Acquire".
2. **Purchases go through.** Pressing ✕ sends the purchase to the server, naming the vendor and the item's place in her list. The server answers, takes the price off the Glimmer, and adds the emblem to the character's emblems. The game then shows its "new item" notice. The emblem's details change to "Can only hold one at a time", and its tile greys out as owned.
3. **Every vendor's stock is known.** A new tool reads what all 124 vendors sell, and what each item costs, from the game's own data: 8,773 offers. Eva's list matches the two purchases exactly (Command and Crypto Shift, 570 Glimmer each).
4. **The server buys for itself.** The server now reads each purchase, looks up the item and its price, takes the price from the right place (account or character) and delivers the item, with no manual steps.
5. **PC mouse and keyboard.** The project's emulator can now run its own code at chosen points in the game, without changing the game's files on disk. The first uses: the PC mouse moves the menu cursor (orbit, Private Match lobby), clicks menu buttons, turns the camera in a match, and fires and aims. The PS4 game still contains its PC keyboard layer; the emulator now feeds and switches it on, and the game reads the keys with its own bindings.
6. **60 fps at the right speed.** The game's 30 fps limit is lifted, and the game keeps its normal speed when a busy scene draws fewer frames. Measured from the game's own clock: 0.99 seconds of game time per real second in a Moon match. Menus and orbit run at 60 fps; busy scenes run in the mid-30s for now, limited by the emulator's drawing speed.

## Tests

- sense69: two emblem purchases at Eva. Glimmer 25,000 → 24,430 → 23,860, the acquisition notices, and "Can only hold one at a time". **PASS** (the purchase's effects were applied by hand).
- sense72: the same with the server applying the purchase by itself: Command, then Crypto Shift, with nothing applied by hand. Glimmer 25,000 → 24,430 → 23,860, about 3 seconds after ✕ each time. **PASS**.
- Vendor stock: 124 vendors, 8,773 offers read; Eva's two entries match the purchases. **PASS**.
- emu73: the PC mouse moves the menu cursor in orbit and the Private Match lobby (a 400 × 150 pixel move moved it by the same amount), and alone put it on LAUNCH. **PASS**. Game clock at 60 Hz in the Moon match: 59.2–59.7 ticks a second = 0.99 game seconds per real second. **PASS**.
- emu74: mouse look in the Moon match; 600 counts right turned the view 18°, 400 counts down tilted it 6.75°. **PASS**.
- emu75: mouse buttons. A left click on LAUNCH started the match; holding the left button fired the hand cannon (magazine 3 → 2). **PASS**.
- emu78: the game's own PC keyboard layer. W was removed from the emulator's controller mapping, so only the game itself could see it. Holding W for 2 s walked the Guardian 10.2 m (twice), and the controller stick, as a control, walked 10.2 m. **PASS**. R switched to the special weapon instead of reloading, so the game's own key bindings differ from what was assumed. **Partial**. Further keys tried: T plays the wave emote, G the sit emote; taps of Z, P, 1–4, Tab, Shift, Ctrl and Alt did nothing visible. Reading the game's own key handling then gave the full table: the keyboard layer is a developer layout, with Space for jump, Q for crouch, E for reload and interact, R for the weapon, and T and G for the d-pad. That explains the R result; putting reload on R needs only a key table in the emulator.

## Maturity

- Vendors: **Working** for Executor Hideo and Eva Levante (screens, stock, prices).
- Buying: **Working**, applied by the server itself (seen twice in sense72).
- Selling, other vendors, reputation: untested.
- PC mouse: **Working** for the menu cursor, menu clicks, camera look, firing and aiming (opt-in lab settings).
- PC keyboard: **Partial**. Movement works through the game's own key layer; the other key bindings are not yet mapped.
- 60 fps: **Working** at full game speed; frame rate limited in busy scenes.

## Evidence index

| Clip | Shows | Notes |
|---|---|---|
| clip02 (46 s) | sense72: the "□ Approach" prompt at Eva, her screen, Command at 25,000 Glimmer with ✕ Acquire, and the purchase applied by the server: 24,430 Glimmer, the "Command" notice, "Can only hold one at a time" | Lab input: □, cursor moves, ✕ |
| clip03 (28 s) | A Private Match on the Moon (The Anomaly) with the 60 Hz game clock: the camera turns left, right, up and down with the PC mouse, then the Guardian walks forward | Lab input: scripted PC mouse movement, then forward. No overlays |
| clip04 (32 s) | emu78: a Moon Private Match. The Guardian walks with the keyboard's W key through the game's own key layer, and the camera turns with the PC mouse. The special weapon is out because R switched to it | Lab input: scripted W, A, D and mouse movement. No overlays |
| clip05 (15 s) | emu78: T plays the wave emote ("D1Guardian waves" in the feed) and G the sit emote, each from the keyboard through the game's own key layer. The third-person view shows the Guardian from behind, with a head | Lab input: S back, T, A nudge, G. No overlays |
| clip01 (48 s) | sense69: Eva's Guardian Outfitter with Crypto Shift selected at 24,430 Glimmer, ✕ Acquire; the Glimmer drops to 23,860; the "Crypto Shift" notice; the details change to "Can only hold one at a time" | Lab input: ✕ to buy, small cursor moves. The Guardian preview on the left is still a silhouette |

Screenshots: 1–3 from clip01 (before, paid, owned); 4 from clip02 (the server-applied purchase with the "Command" notice); 5 from clip03 (the Moon match under mouse look); 6 from clip04 (walking by keyboard); 7 from clip05 (the wave emote, third person).

## Remaining issues and next steps

- Purchases priced in character-held currencies (marks, Strange Coins), and the other vendors.
- Selling.
- The Guardian's head: in the Moon match the emotes switch to third person, and the Guardian is drawn with a head (the served Festival of the Lost mask, with its horned crest). In the Tower the head looked missing with both the mask and a regular helmet, while the mask draws normally on the Moon, so the helmet items are probably not the cause; the Tower view had the camera almost inside the Guardian. Next: the wave emote in the Tower, for a clear third-person view.
- PC controls: the game's own key bindings (R switched weapons), aim assist and the look feel (the camera currently turns through the controller's look rate).
- 60 fps: smoother motion above the game's own clock (interpolation), and drawing speed in busy scenes.

## Commits

Public source, main branch (merge commits):

- fe7a3e146b90403d11aca5a730051656aed889ed: stop serving two unlock flags the Tower needs clear
- e58c9d2404bfc5363e040b40b4e9675f3fff9473: decoder tests skip when private inputs are absent
- 167febf7ebc2a52f480e4e05ae960d1e2edba57b: activity-host notification tests
- ad6c0b48f96b5802b180ed97db7154511f8d7bad: serve and push the account object; answer vendor buy and sell
- 9546cafe79543f11ffc082e6ae4bda4120187126: notes on currencies and the first purchase
- f7595f4e6895451fb0e054789ee8d6ac5955589e: vendor sales table, 60 Hz clock patch, emulator frame-rate setting
- c68af9e1e39f6c0f202397ba188280f6e15c8540: vendor sales record each item's inventory bucket
- 99f9f5a89507a7140fad8148cdbd5d604534c78f: the server applies purchases and pushes the results
- 52ce6b75dcd4a0358f56336f08912d52694527f3: 60 Hz at full game speed; emulator code hooks and the PC mouse cursor
- f26260e4931e0026d37ce8396d9594166898213f: PC mouse look in gameplay
- 34b7de2bc8e4843bcf11a01af58db7eab419c4cb: PC mouse buttons
- 689024a8635b7c9d0f4166a4962435d28b462f1c: the game's own PC keyboard layer

## Privacy, audio and rights

- Game window capture only; no overlays, personal identifiers or desktop.
- Game audio from the emulator's own output (no microphone); music may be present.
- The character is the project's test character ("D1Guardian", visible in item details).
