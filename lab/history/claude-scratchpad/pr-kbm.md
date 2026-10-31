### What
The emulator patch (`tools/ps4_setup/shadps4-0.19.0-d1-mouse.patch`) gains `D1_KBM=1`, which feeds the game's own PC keyboard layer. 56578 still has the engine's keyboard-and-mouse input path. It builds its input from a 24-byte mouse block followed by 104 key records `{u16 held_ms, u8 frames, u8 flags}` at `0x1AD4860`, and reads it only while the enable byte `0x1AD40D3` is set.

A detour at the builder's entry (`0x926965`, `lea rax,[0x273ED40]`, right after `0xAD120` clears the blocks) does two things:
1. It writes the host keyboard's state into the 104 records in the engine's key order. The order comes from the Xbox 56578 table, and 01.29's queries agree: WASD = 0x20/0x2D/0x2E/0x2F, arrows 0x4D–0x50, space 0x48.
2. It sets the enable byte while the game window has focus.

The engine then applies its own bindings. Mouse counts stay zero, so look still goes through `D1_MOUSE_LOOK`.

The patch is regenerated from fork commit `978180a` (branch `d1` of the emulator fork). STATUS updates the PC controls line.

### Tested
emu78: a Private Match on the Moon, 60 Hz clock, with `D1_MOUSE_LOOK`, `D1_MOUSE_BUTTONS` and `D1_KBM` on. For the test, the pad config's stick-up was moved from W to Y, so only the game's key layer could see W.
- Holding W for 2 s walked the Guardian 10.16 m, and again 10.29 m. Holding Y (the pad stick) as the control walked 10.15 m. Positions were read from the game's own unit.
- R, which the pad config does not map, switched to the special weapon (it did not reload). Action keys reach the game, but the engine's default PC bindings still need mapping.
- The four detours installed: `0x926965` (7 bytes relocated), `0x985d00`, `0x9284ef`, `0x91c7dc`.

### Notes
Keys the pad config also maps (WASD, IJKL and so on in the default config) now act twice: once through the pad and once through the engine. Clearing the keyboard section of the pad config avoids that.

AI-assisted: yes.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
