# PS4 56578: character creation from our servers (new1-new3, 7 November 2026)

For the first time on PS4, the client created a Guardian against our servers and went on into the
new player's start: the prologue, then A Guardian Rises at level 1.

## How it was served

`tools/ps4_setup/d1-full-game.ps1` with:

- `server:D1_CHARACTER_SOID=0`: an account with no characters, so the client offers creation;
- `server:D1_STARTING_KIT=content`: the 56578 content's first default-gear list
  (`characters/starting-kit.tsv`);
- the level-1 fixture set from `tools/ps4_setup/d1-level1-fixtures.py --no-ability-rows` (private
  output: profile, inventory, roster cards and the rifle grant) as `D1_PROFILE_FILE`,
  `D1_INVENTORY`, `D1_ROSTER_DIR`, `D1_LOOT_GRANT` and `D1_LOOT_GRANT_ROSTER`.

The bot drove it by pad:
1. "Choose your class": the cursor on Titan, then Cross.
2. "Customize your appearance": Finish, then Cross.

## What happened

| Run | Served after Finish | Result |
| --- | --- | --- |
| new1 | the stock profile and level-40 cards | 501 `CREATE CHARACTER race=2 gender=0 class=0`, SOID minted, the starting kit as 13 store items. Then orbit as a 40/385 Guardian ("Crucible unlocked"). |
| new2 | the level-1 set | **The prologue played (the Mars landing), then A Guardian Rises' opening.** The Guardian spawned at 18:04:30, then the Wall, the Breach, the light-up and the Khvostov (phase 3 at 18:07:17). |
| new3 | the level-1 set, with the created look (below) | As new2, phase 3 at 18:40:50. The character screen: level 1, Light 5, the Khvostov, Ghost and armour, no abilities. |

The client picks the default race and gender at random. new2's request was an Awoken female
(`race=1 gender=1`), new3's an Awoken male.

## The created look

The 501 request (`d1_bap::character_lifecycle::CreateCharacterRequest`) carries:
- the identity `0x808017FC` {race, gender, class} at `+0x00`;
- the customization `0x8080181A` at `+0x04`.

The peer view `0x808017FD` and the profile's character record `0x80801714` hold the same two blocks
at `+0x08` and `+0x0C` (`ps4/fixtures/56578/rtti/fields.tsv`).

Until now the served cards and profile kept their fixtures' look, and that look is empty: the
level-1 card's customization is all zeros, apart from the helmet field.

Now `player_store::apply_created_look` handles the 501. It decodes each family-0 peer card and the
profile with the client's codec, writes race, gender, class and customization f0-f9, re-encodes,
and pushes the profile. f10 stays as it was: in the profile it is the last-orbited destination. The
wear-helmet field f11 (`+0x2C`) stays too. In new3 the look was in the card and the profile 120 ms
after the 501; the card went out at the next roster fetch.

The unit tests:
- `look_image` equals the client encoder's C object for every codec vector;
- `write_look` keeps the key, the destination and the helmet field.

## The class's starting gear (new4, new5)

new4 created a Hunter. The client drew a Hunter body, because the card's class now follows the
request, but the character screen showed the level-1 set's Titan gear. The store serves
`D1_INVENTORY`'s items; `D1_STARTING_KIT` was never applied to them. That set is a later save's
gear anyway: Striker, the Oryx Mask, the Competitive Shell.

The content's first default-gear list (`characters/starting-kit.tsv`, which the client's default-gear
loader `0x576150` preloads) is D1's starter gear. The archived manifest names it:

| Class | Helmet, gauntlets, chest, legs | Class item |
| --- | --- | --- |
| Titan | Lightmail | Mark of the Risen |
| Hunter | Prototype 0.9 | Makeshift Scarf |
| Warlock | Born Spark | Potential Bond |

All classes get the Generalist Shell Ghost.

So on a 501, with `D1_STARTING_KIT` set, `player_store::class_kit` and `apply_kit_to_profile` put
the class's armour and Ghost, and its starting subclass, into the slots the served set already
equips. The subclasses by content index are Striker 710, Gunslinger 709 and Voidwalker 713. Each
slot keeps its instance, and those instances' item elements get the new content index (`+0x24`).
Weapons stay out: a new Guardian has none until the Khvostov. So do the ship, vehicle, emblem,
emote and horn the list also names; their policy is unverified.

new5, a Human female Hunter: all seven swapped 120 ms after the 501. Then the prologue and A
Guardian Rises to the rifle. The character screen shows Gunslinger, the Prototype 0.9 armour, the
Makeshift Scarf, the Generalist Shell and the Khvostov, on a female Hunter's silhouette.

new6, a Human female Warlock: Born Spark, the Potential Bond, the Generalist Shell and Voidwalker,
then A Guardian Rises to the rifle. On "Choose your class", the class preview drew part of a Warlock
(a dark robe and hand), the first sign of the creation preview drawing anything.

## Open

- **The look can't be seen yet.** The creation screen's Guardian preview is empty, and the character
  screen draws the Guardian as a black silhouette. That's F47, the black and see-through models, so
  that the chosen face reaches the screen is unconfirmed.
- ~~Class.~~ Done the same evening, below.
- **The stock fixtures' look** is all zeros. A captured character's customization would make the
  served Guardian look like one; whether the zero look causes F57's headless Titan is to test.
- **Retail's first activity.** The client chose the prologue and A Guardian Rises by itself from the
  level-1 profile's completion record. Whether retail's new profile differs (the new-Guardian
  record, `tools/ps4_setup/d1-new-guardian-profiles.py`) is still open.
