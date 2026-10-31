# D1 01.29: mouse-driven menu cursor (implementation handoff)

Status: static disassembly only, 2026-09-16. Nothing here has been run or traced live.
Target: CUSA00219 eboot 01.29. All addresses are eboot virtual addresses, as used by
`d1_native_mouse.cpp` (`g_eboot_address + vaddr`). Reference build: the Xbox One
`tiger_release_final.exe` (build 56578) and its adapter in
`d1-render-core-integration` (`m0/native_input_*.h`, `reports/native-input/README.md`).

## Goal

Move D1's own menu/Director cursor with relative mouse motion, the way the Xbox build does
natively. Clicks stay on shadPS4's existing button bindings (left button -> cross).

## What the PS4 eboot contains

The engine's PC keyboard/mouse input layer survived on PS4, with the same layout as on Xbox.
Only the platform producer that fills it is missing: nothing writes live mouse/key state, and
nothing sets the mouse-enable flag.

| Item | PS4 01.29 | Xbox 56578 |
|---|---|---|
| Mouse state, 24 bytes: int32 dx/dy/wheel at +0/+4/+8, 4 button frame counts at +12, 4 uint16 held-ms at +16 | `0x1abc3e0` | `0x1849650` |
| Key table: 4-byte records (held ms, frames at +2, flags at +3), starting at mouse+0x18 | `0x1abc3f8` | `0x1849668` |
| Mouse getter: returns `&state` or null | `0xad670` | (`0x228350` family) |
| Key frame-count query `f(key)` (0x69..0x6c are combined modifiers) | `0xad300` | `0x228070` |
| Input-layer flags base | `0x1abbc50` | |
| Per-player input record array base (stride 0x300; player 0 = base+0x2a0 = `0x2725C50`) | `0x27259b0` | `0x2869E00` |
| KBM builder (writes record `+0x2a0 = 1`, mouse counts -> `+0x2c8/+0x2d0`, key axes) | inside `0x924c70`, at `0x925d60..` | `0x933FC0` |
| Menu cursor update (consumes mouse deltas) | `0x984030` (single caller `0x983d40`) | `0x9AC020` |

`0xad670` getter logic:
```
if (byte[0x1abbc53] == 0) return NULL;            // mouse enabled
if (byte[0x1abbc51] == 0) return &0x1abc3e0;      // live state
return &0x1abca40;                                // zeroed state (suppressed)
```
No writer of `0x1abbc53` was found (rip-relative scan of .text). `0xad190` (reached through
`0xad120`, called at builder entry `0x924cc0`) zeroes the mouse and key blocks.

### Menu cursor function `0x984030` (rdi/r15 = UI root)

```
0x984052 cmp qword [r15+0x398],0 ; je 0x9841c3     // no cursor -> skip
0x984060 mov ebx,[r15+0x210]                       // 41 8b 9f 10 02 00 00  (7-byte span)
0x984067 sub ebx,[r15+0x2c8]                       // elapsed ms
0x98406e call 0xad670                              // mouse getter
0x984073 test rax,rax ; je 0x9840c6
         if (dx||dy) cursor.xy (float, [r15+0x350]/[+0x354]) += (float)dx,(float)dy
0x9840c6 mov edi,0x4f ; call 0xad300 ...           // key/stick-driven cursor movement (0x4f/0x50/0x4d/0x4e)
0x9841a4 vmovaps [r15+0x350], xmm0                 // writes position after key movement
0x98457f vmovaps xmm0,[r15+0x350]                  // later reads (hit testing/clamps; not characterized)
```
Mouse counts go straight into cursor units, with no scaling (the Xbox code is identical).

## Implementation

**Owner decision (2026-09-16): implement both.** The mouse must drive the cursor in every menu
(A), and the engine's native keyboard/mouse layer must be enabled globally (B). A is the
first step: it's self-contained and gets the cursor working. B is the target end state. Its risks
below are things to handle and verify, not reasons to skip it. When B is live, A's detour is
redundant for the cursor (the native mouse block at `0x98406e` does it) and can be retired or
kept behind a switch.

### A: cursor detour (all menus)
Add a permanent detour, using the same `InstallPermanentDetour` machinery as `d1_native_mouse.cpp`, at
**`0x984060`**, pinned span `41 8B 9F 10 02 00 00` (`mov ebx,[r15+0x210]`). That point
runs only while a cursor root exists. In the callback:

1. Drain a menu-cursor mailbox of raw relative counts.
2. Apply sensitivity, carry the fractional remainder, and add to floats at `r15+0x350` (x) and `r15+0x354` (y).
   Check the sign empirically: Xbox adds raw dx/dy (screen-down = +y).
3. Leave `0x1abbc53` alone, so the builder, camera and "input active" consumers are unaffected.

The engine's later code (from `0x9841ad`) clamps the cursor and does hit testing.

### B: enable the native layer globally (required)
Write dx/dy (and button frames/held-ms) into `0x1abc3e0`, set `0x1abbc53 = 1`, and keep
`0x1abbc51 = 0`. This is the PS4 equivalent of the Xbox adapter feeding its mouse/key tables. The
engine's own cursor, builder and camera consumers then run as they do on Xbox. These getter callers
change behavior and each needs live verification:
- `0x925d68`: the builder's KBM path. When player 0's pad reads idle, it selects KBM (`rec+0x2a0 = 1`) and
  then skips pad-sourced actions in the look sample (`0x9198ba`).
- `0x8e7520`, `0x8e7588`, `0x8e792a`: gameplay/camera consumers that branch on non-null mouse.
- `0x8dcdd6`: an "any input active" check.
The writes also race with `0xad190` zeroing and would need to land after `0x924cc0` on the same thread.

## Interaction with the existing native mouse (`D1_NATIVE_MOUSE`)

- The look hook (`0x91AB3C`) and the cursor hook would both want the same deltas. Feed two
  mailboxes from the raw-input hook, or route by whichever consumer is active.
  Unverified: whether the look sample keeps running while the Director is open. If it does, both
  would move. Check live and gate if needed (for example, drop look motion on frames where the
  cursor detour ran within the last N ms).
- Keep SDL relative capture in menus, because D1 draws its own cursor. Absolute OS-pointer mapping is
  not needed.

## Related finding (not part of the cursor work)

The current look injection runs with record `+0x2a0 == 0`, so after the converter (`0x9269D0`, the same
function as Xbox `0x9336B0`) the pad-only block `0x91ab77..0x91b7a7` still applies aim-assist slowdown
(`0x91b2d7`), a rate clamp (`0x91b76b`) and target steering (`0x91b6e3`) to mouse look. With B live, the builder sets `+0x2a0 = 1` in keyboard/mouse mode, and that skips this block
natively. Watch for one thing: with the flag set, the look sample also skips the pad read
(`0x9198ba`), so shadPS4's key-to-pad bindings must keep working while the builder chooses
keyboard/mouse mode. It chooses that mode when player 0's pad reads idle. If pad-mapped keys stop
responding, either feed the key table at `0x1abc3f8` (as the Xbox adapter does) or handle the switch.

## Live checks before calling it done

- The cursor moves in the Director, character menu, inventory and settings; clicks select the hovered item.
- Cursor clamps at the screen edges at 16:9 and ultrawide output.
- The look hook doesn't also turn the camera behind in-world menus.
- Losing focus or pausing clears pending cursor motion (mirror `g_motion.Clear()`).


## Implementation receipt (Codex, 2026-09-17)

Implemented global native publication by default under `--d1-native-mouse`. Three pinned detours
install transactionally: `0x924cc5` publishes mouse + 104 key records; `0x984060` observes active
menus (and implements the optional cursor fallback); `0x91ab3c` suppresses player look while a menu
cursor was seen in the last 100 ms. No mouse-to-stick injection runs in full-native mode.
`D1_NATIVE_CURSOR_ONLY=1` selects the interim cursor path with legacy gameplay look instead.

Disassembly correction: `0xad120` is an input lock/ownership function, not the reset routine.
The zeroing routine is `0xad160`. Publication follows the builder's call to `0xad120`, on its guest
thread, before its consumers run. The native getter's enable byte is set only while captured;
the suppression byte is set when capture is inactive. Focus/pause resets pending controls.

Keyboard identifiers use the Xbox 56578 table at RVA `0x11f2a50`, cross-checked against PS4's
WASD, arrow, space and modifier queries. State tracks press/release edges, saturating frame counts
and real elapsed held-ms; fractional mouse counts survive sensitivity scaling. Native keyboard/mouse
gameplay bindings are excluded from virtual pad output; physical controllers and frontend hotkeys
remain. Menu-active frames retain existing pad bindings for UI clicks/navigation. Route changes
request frontend binding refresh, releasing held virtual axes without waiting for another key event.
Legacy command-line inversion switches apply only to fallback look; global mode uses native settings.

Focused state regression executable passes. Live acceptance remains pending: menu click behavior,
all menu surfaces, gameplay actions, polarity, native sensitivity and edge clamps at both aspect
ratios must be checked. The 100 ms menu look gate is a conservative heuristic and needs live validation.


## Hybrid controller / mouse correction (2026-09-17)

User confirmed menu mouse movement, then reported inverted gameplay Y, physical-controller input
blocked, and a recoil-like correction. Removed the adapter write to suppression byte `0x1abbc51`:
`0xad490` proves that flag suppresses the physical pad too. Mouse capture now owns only `+3`.

At `0x91ab3c`, a scoped look override preserves the selected record's actions/movement and temporarily
supplies only native mouse axes (no stick look). `0x91b7a7` restores every changed axis and mode byte,
so controller action handling retains its original mode. Pitch sign is corrected here only; global
cursor Y remains screen-down-positive. Both inversion switches now apply to this gameplay-only path.
The conversion retains PS4's native constants (-0.0015 yaw, corrected +0.003 pitch before native
sensitivity/dt conversion); this is NOT a claim of exact D2 sensitivity or ADS parity.

Xbox's `native_aim_engine.h` explicitly gates a later shared target-assist blend. PS4 equivalent:
`0x91bb3e`, before the strengths at frame -0x58/-0x54 determine AL; native capture zeros those strengths
for player zero, taking the game's unassisted branch at `0x91bb68`. Original aim/target bookkeeping
remains. Ordinary weapon recoil has not been disabled; whether this resolves the reported behavior
is still a live-test question.

Focused tests exercise physical-pad mode restoration, action bytes unchanged, native-mode restoration,
mouse polarity/inversion, zero menu camera axes, and incorrect-frame refusal, in addition to prior
input-state checks. Physical controller use plus mouse look still needs live acceptance.


## Mixed-input buttons and repeated counts (2026-09-17)

User live result: gameplay Y/physical pad patch reached both hybrid-look and late-assist hooks,
but LMB stopped firing when pad input was present, RMB did nothing, MMB acted as ADS. User requests
Mouse4 reload and Mouse5 slide; unwanted panning occurs in gameplay with menus closed.

Added a pinned callback at `0x92684f` before the builder releases its input lock. It merges physical
mouse actions into player zero's final record after exclusive pad/KBM selection. Bindings are LMB
right trigger/fire, RMB left trigger/ADS, MMB right stick click, Mouse4 square/reload-interact,
Mouse5 circle/crouch-slide. Slide retains normal sprint requirements. While a cursor is active,
LMB/RMB map to cross-confirm/circle-back. All five physical buttons now have transition storage.
Native developer mouse-button slots are neutral to avoid duplicate/wrong actions; global mouse
motion and keyboard tables remain native. Old mouse-to-pad bindings are filtered in menus too.

Action-union tests cover simultaneous input, release handoff, menu routing and unchanged physical
pad movement/mode. Look now consumes accumulated counts once; a second look update cannot reuse
the last platform sample. Menus clear pending gameplay look. This is a plausible gameplay-drift
fix, not a confirmed diagnosis of the user's panning. Live acceptance of these changes is pending.
