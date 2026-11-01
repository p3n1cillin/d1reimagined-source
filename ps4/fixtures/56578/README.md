# Served fixtures for PS4 build 56578

What the project's servers hand the 56578 client during a lab run: the Guardian, the mission schedule
and the post-game screens. None of it is a game file. Each file is an object the server encodes and
sends, or a list the activity host reads, built by the project's own tools (`d1-oracle`,
`d1-inventory-renumber.py`, `d1-agr-steps.py`, `d1-winguest.py sensor-clients`) from the maintainer's
owned copy. The scripts find this folder through `D1_FIXTURES` (default: here; see
`tools/ps4_setup/d1paths.py`).

| Folder | What | Used by |
|---|---|---|
| `characters/` | Profiles, inventories and roster (peer) cards: the geared Guardian (`profile-geared-385-director-c.bin`, `inventory/`, `rosters-f0-geared/`), the unarmed tutorial Guardian (`profile-tutorial-t.bin`, `inventory-tutorial/`, `rosters-f0-tutorial/`), the Breach rifle grant (`grants/`), and the cards served after it (`roster-def1200-helm-saint14/`, `roster-defense1200-attack200/`). `activity-table.tsv` maps activity indexes to hashes. | `d1-lab-served.ps1`, `d1-agr-run.ps1`, `lab/tower-route/ending-run.ps1` |
| `mission/` | The A Guardian Rises schedule (`agr-steps-divide.txt`) with every step's sensor list, the world tables (`world-table.txt`; `world-table-tower.txt` adds the Tower as 0 and 165), the Breach prop list and the post-game phase 7 list. Relative names in a list are read from this folder. | `D1_AH_MISSION_STEPS`, `D1_AH_WORLD_TABLE`, the route |
| `postgame/` | Profile pushes for the end of the mission (XP 0 and 900, the completed profile with the Arcadia) and the post-game scoreboard. | `lab/tower-route/postgame5.sh` |
| `vendor/` | The account with Glimmer 25,000 (slot 308), Silver 500 (309) and Legendary Marks 200 (310), the profile and item template the responder applies purchases to, and the sale table (`d1-oracle vendor-sales write`). | `ending-run.ps1` (vendors on by default) |

The Guardian is a Titan Striker. Every peer card carries the Striker's ability rows and perks, and
both subclass items name grid 132 with its default nodes (`tools/ps4_setup/d1-talent-grid.py 132`,
written with `d1-character-fixup.py`; issue #134). Without them there is no grenade, melee or super.

Every profile and peer card also sets the wear-helmet flag (customization block `0x8080181A`, the
2-bit field at `+0x2C`; `d1-character-fixup.py --helmet`). With it clear, the Guardian had no head in
the Tower (issue #128); run sense77 (31 October) served it set and the helmet showed.

The three post-game profiles (`postgame/profile-active164-xp0`, `-xp900`, `arcadia-profile-xp900-owned`)
carry the gear the Guardian leaves the mission with. That is the tutorial inventory's subclass,
armour, Ghost and vehicle, plus the granted Khvostov, each in its bucket's slot and in the equipped
array (`d1-character-fixup.py --item`; notes/ps4-56578-equip-2026-11-01.md). Without it the character
screen after the mission was empty. The items are the ones `characters/inventory-tutorial` and the
rifle grant serve.

The defaults of `lab/tower-route/ending-run.ps1` serve what run sense73 (31 October 2026) served,
except for the account (sense73's put Legendary Marks in Silver's slot) and the post-game gear above. Files named `*.draft.*` are lab drafts, kept as they were served.
