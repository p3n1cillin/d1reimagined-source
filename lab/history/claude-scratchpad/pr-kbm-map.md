### What
`D1_KBM_MAP` rebinds the game's own PC keyboard layer (#126). Each `HOST=ENGINE` pair makes the host key press that engine key instead of its own. Key names follow the engine's 104-key order, case-insensitive. For example, `R=E,E=E,1=R` puts reload on R, keeps interact on E, and moves the weapon switch to 1.

Why it's needed: the engine's key bindings, read from its KBM builder (`0x926910`), are a developer layout. Each key's frame count (`0xAD300`) and held ms (`0xAD3A0`) go into one of player 0's 21 controls (`0x273ED40 + 0x2A0 + 0x14·i`):

| Engine key | Control |
|---|---|
| End, mouse 2 | 0 (L2, aim) |
| Ins | 1 (R2, fire) |
| T, G | 2, 3 (d-pad up, down: wave and sit emotes) |
| X, C | 4, 5 (d-pad left, right: C plays the dance emote) |
| PgDn, PgUp | 6, 7 (PgDn opens the character menu; PgUp closed it) |
| B, mouse 1 | 9 (R3) |
| Space | 10 (cross, jump) |
| Q | 11 (circle, crouch) |
| E | 12 (square, reload / interact) |
| R | 13 (triangle, weapon) |
| Z, F, V, U, J, K, N, M | 8, 14–20 (no visible effect from a tap) |

WASD write the move axes, and the arrows the look axes.

The patch is regenerated from fork commit `08607c5`. The fork README documents the variable and the table.

### Tested
emu79 was a Moon Private Match on the new emulator build. It ran with `D1_KBM=1`, `D1_KBM_MAP=R=E,E=E,1=R`, and the new `shadps4-input-kbm.ini`, reloaded in the match with F8. Only the game's key layer saw the keys.
- The log shows `host R presses engine E`, `host E presses engine E` and `host 1 presses engine R`.
- W walked the Guardian (6.9 m in 2 s), and Space jumped (1.54 m rise; the old pad config had Space on the touchpad).
- After one shot (24 | 184), **R reloaded**: 25 | 183.
- T waved, C danced and PgDn opened the character menu. Taps of Z, F, V, U, J, K, N and M did nothing visible.

The new pad config was not exercised with a physical controller in this run.

`d1-drive-private-match.ps1` gains `D1_PAD_UP` (default W), so a keyboard test can move the pad's stick-up off W and still drive the menus.

AI-assisted: yes.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
