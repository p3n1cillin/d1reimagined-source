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

## Open

- **The look can't be seen yet.** The creation screen's Guardian preview is empty, and the character
  screen draws the Guardian as a black silhouette. That's F47, the black and see-through models, so
  that the chosen face reaches the screen is unconfirmed.
- **Class.** The kit has the class's own armour, but the level-1 set's subclass item and cards are
  the Titan's. A Hunter or Warlock needs its class's subclass and card.
- **The stock fixtures' look** is all zeros. A captured character's customization would make the
  served Guardian look like one; whether the zero look causes F57's headless Titan is to test.
- **Retail's first activity.** The client chose the prologue and A Guardian Rises by itself from the
  level-1 profile's completion record. Whether retail's new profile differs (the new-Guardian
  record, `tools/ps4_setup/d1-new-guardian-profiles.py`) is still open.
