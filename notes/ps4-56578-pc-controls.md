# PC controls on 56578 (keyboard and mouse)

The PS4 build still carries the engine's PC input layer. The emulator fork switches it on and feeds
it through code detours (`tools/ps4_setup/shadps4-0.19.0-d1-mouse.patch`); nothing on disk changes.
Tracked in issue #113.

## The emulator settings

| Setting | What it does |
|---|---|
| `D1_MOUSE_CURSOR=1` | Menu cursor follows the PC mouse (cursor update `0x985D00`). |
| `D1_MOUSE_LOOK=1` | Camera turns with the mouse in gameplay (look site `0x91C7DC`, player 0's record `+0x2CC`/`+0x2D4`). |
| `D1_MOUSE_BUTTONS=1` | Mouse buttons merged into player 0's controls (`0x9284EF`). Menus: left = cross, right = circle. Gameplay: left = fire, right = aim, middle = R3, Mouse4 = square, Mouse5 = circle. |
| `D1_KBM=1` | The engine's own keyboard layer: 104 key records written at its builder entry (`0x926965`) into the table after the mouse state (`0x1AD4860`), and the layer enabled (`0x1AD40D3`). |
| `D1_KBM_MAP=HOST=ENGINE,...` | Rebinds the keyboard layer: each host key presses that engine key instead of its own. |

With `D1_KBM` on, use `tools/ps4_setup/shadps4-input-kbm.ini` as the pad config. It keeps the
controller and drops the letter keys, so no key acts twice.

## The engine's key table

Read from the KBM builder (`0x926910`) with `python scripts/d1_eboot_ps4.py kbm-table`. Each key's
frame count (`0xAD300`) and held time (`0xAD3A0`) go into one of player 0's 21 controls. Those sit
at `0x273ED40 + 0x2A0 + 0x14 * i`, with `{u16 held_ms, u8 frames, ..., float value at +8}`.

| Engine key | Control | Seen in a match |
|---|---|---|
| End (and mouse 2) | 0 | L2, aim |
| Ins | 1 | R2, fire |
| T | 2 | d-pad up: wave emote |
| G | 3 | d-pad down: sit emote |
| X | 4 | (d-pad left, by elimination) |
| C | 5 | d-pad right: dance emote |
| PgDn | 6 | Options: the character menu |
| PgUp | 7 | Touchpad: the Ghost menu |
| Z | 8 | nothing seen, tapped or held |
| B (and mouse 1) | 9 | R3 |
| Space | 10 | cross: jump |
| Q | 11 | circle: crouch |
| E | 12 | square: reload / interact |
| R | 13 | triangle: weapon switch |
| F, V, U, J, K, N, M | 14–20 | nothing seen, tapped or held |

WASD write the move axes and the arrows the look axes. So the layer is a developer layout, and
reload is E. Sprint (L3), melee (R1), grenade (L1) and super (L1 + R1) have not been found among
these controls: W with Z or Shift ran no faster than W alone (emu80, 31 October). They may come
from controls 8 and 14–20 only in combination, or from inputs outside this table.

## Runs

- **emu78:** W walked the Guardian 10.2 m in 2 s with W unmapped in the pad config, the same as the stick. R switched weapons, T waved and G sat.
- **emu79:** with `D1_KBM_MAP=R=E,E=E,1=R`, R reloaded (24 → 25) and Space jumped 1.54 m. C danced, and PgDn opened the character menu.
- **emu80:** PgUp opened the Ghost menu. Z, F, V, U, J, K, N and M, held for 600 ms, did nothing visible.
