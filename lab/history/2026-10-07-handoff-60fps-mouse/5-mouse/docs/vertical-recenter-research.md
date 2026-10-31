# Vertical recenter while moving ("back to center" pitch drift)

Research note, 2026-10-03. Static only: nothing was built, run or attached. Build: CUSA00219 01.29
eboot (vaddrs below are eboot-relative), fork build `build-nvtx-perf` (0.18.1 line), whose binary
contains the current uncommitted hybrid-look code (its log strings "hybrid look reached" and "late
target-assist bypass" are present in the exe).

Tags: `[C]` code or disassembly read, `[R]` doc, `[I]` inferred, `[U]` unknown.

Symptom: with keyboard and mouse, while the player moves, pitch is pulled back toward the horizon.
The in-game Auto Look Centering option is reported OFF. Once, the view was dragged down instead.

## 1. Where pitch is pulled toward level

All of it sits in the camera-angle applier `0x91EEF0`. It is called once per frame per local player
from `0x743780` (look path `0x7432B0`, fed the look output of `0x919860`), again from `0x91B82E`
(camera-impulse call inside `0x919860`, only when a kick or shake is active), and from `0x91DDCF` and
`0x91E05D` (the other look path, `0x91C360`) `[C]`.

Per-player look state: `P = *(u64*)0x2725798 + player*0x1A0`. Yaw is `P+0x38`, pitch `P+0x3C`
(radians, 0 = level, hard clamp ±1.4922565). `[C]`

There are three mechanisms. Two scale with movement speed.

### 1a. Auto look centering (setting-gated), `0x91FE7B..0x91FFD0`

```
0x91FE6B  xmm1 = [rbp-0x280]            ; tuning target pitch (block +0x18), see 1b
0x91FE73  if xmm1 != 0 -> centre on it
0x91FE7B  cmp byte [r14+r13+0xE8], 0     ; P+0xE8 = "centering active"
0x91FE84  je 0x91FFDC                    ; skip both branches
...
0x91FF88  rate = |pitch| * (2/pi) * T[+0x1D4]          ; 2/pi at 0x165D14C
          step = rate * min(|unit velocity|, f(3.0)) * dt ; 3.0 at 0x165D150 via 0x236720, 0xBCC7D0
0x91FFD0  pitch moves toward 0 by step, no overshoot
```

`T` is the input-tuning block returned by `0x525F60` (content tag `[0x2697DB8]`, or the static
default `0x2697B90`). `[C]`

`P+0xE8` is written in `0x7432B0`, after the look call and after the `0x91EEF0` call at `0x743780`,
so it acts on the next frame `[C]`:

```
0x743785  settings byte [rsp+0x2B2] == 0          -> clear
          or 0xBCF180(unit) != 0 ([rsp+0x24])      -> clear
0x74379B  byte P+0x160 != 0                        -> clear
0x7437B6  |record+0x2C0| <= 0.5  (0x163B7A0)       -> clear   ; forward/back move axis
0x7437CC  record+0x2D0 != 0                        -> clear   ; mouse pitch slot (KBM)
0x7437DC  record+0x2D4 != 0                        -> clear   ; stick or key pitch axis
          else  P+0xEC = min(P+0xEC + 1, T[+0x1D8]);  P+0xE8 = (P+0xEC == T[+0x1D8])
0x743818  clear: P+0xEC = 0, P+0xE8 = 0
```

`[rsp+0x2B2]` is byte `+0x9A` of the per-user controller-settings copy (`0x926880` copies 0xA8
bytes from `0x27259B0 + user*0xA8` to `rsp+0x218`; 0x2B2 - 0x218 = 0x9A) `[C]`. The KBM builder
`0x924C70` refreshes that table every pass: `0x1F70` returns 1 unconditionally, so it always calls
`0x924960(0..3)`. That function fills `+0x98..+0x9C` from the profile options object
`([0x26A3790] + 8)`, via `0x947A40`/`0x57CC80` `[C]`:

| table byte | options byte | use (`[C]` reads, names `[I]`) |
|---|---|---|
| `+0x98` u16 | `+0x12` | stick layout (builder switch 0..3) |
| `+0x9A` | `+0x20` | auto look centering: read only by the gate above |
| `+0x9B` | `+0x1E` | invert look: negates the pitch axes |
| `+0x9C` | `+0x1F` | negates the yaw axes |
| `+0x00/+0x04` | `+0x13` index | look rates from `T[+0x140/+0x144 + idx*8]` |

Init (`0x924240`) zeroes `+0x9A`, so the default is centering off until the profile options are
copied in `[C]`.

**Condition, as the code reads:** auto look centering on, moving forward or back with
|move| > 0.5, no vertical look input (`+0x2D0` and `+0x2D4` both exactly 0) for `T[+0x1D8]`
consecutive frames, not in the excluded unit state. Pitch then decays toward 0 at a speed
proportional to movement speed and to the pitch offset. Horizontal look does not reset the counter.

### 1b. Content pitch target (setting-independent), same block

If the unit's active look-tuning block (resolved by `0x4CACD0`, pointer `r15` / `lVar20`) has a
nonzero float at `+0x18`, pitch is pulled toward that value at
`|pitch - target| * (2/pi) * 0.08 * speed * dt`. The constant 0.08 is at `0x165D154`. This branch
ignores `P+0xE8` and the user setting `[C]`. Which units or states give `+0x18 != 0` is `[U]`; it
could be on-foot sprint, a vehicle, or nothing.

### 1c. Soft pitch-limit spring (setting-independent, not speed-scaled), `0x92011C..0x9202DC`

When the pitch input of this call (`[rbp-0x23C]`) is exactly 0, pitch outside
`[min + m_lo, max - m_hi]` is eased back with `edge + (p - edge) * exp(-rate*dt)` (`0x78B50`). The
margins are tuning `+0xC4/+0xC8`, falling back to `T[+0x134/+0x138]` when negative. The rate is
tuning `+0xCC`, falling back to `T[+0x13C]`. A rate of 0 or less disables it `[C]`.

With a stick held down, pitch input is nonzero every frame, so a pad user only meets this when
releasing the stick near the floor or the sky. With a mouse, pitch input is zero on every frame
between mouse samples, so looking at the floor triggers a steady upward pull `[I]`.

## 2. Is it gated by the KBM flag?

No. Nothing in 1a, 1b or 1c reads `record+0x2A0` or `0x1ABBC53` `[C]`. The flag only skips the
pad-only block `0x91AB77..0x91B7A7` (acceleration, assist slowdown, rate clamp, steering) `[C]`.
The engine does anticipate the mouse in one place: the gate's "no vertical look" test includes
`+0x2D0`, the slot where the native KBM builder writes mouse pitch
(`0x2725F20 = dy * -pi/1000 / (pi/3)`) `[C]`. Retail Xbox/PC behaviour was not checked `[U]`.

## 3. The fork's native-mouse feed

`src/core/d1_native_mouse.cpp` and `src/input/d1_native_look.h`, the latter untracked in git `[C]`.

- Full-native mode (default): `GateLook` (`d1_native_mouse.cpp:242-273`) runs at `0x91AB3C` for
  player 0. When the mouse moved, `LookOverride::Begin` (`d1_native_look.h:21-44`) writes
  KBM-style per-sample angles into `+0x2C8..+0x2DC` = {dx*-0.0015, 0, dy*0.003, 0, dy*0.003, 0}
  and sets `+0x2A0 = 1`. The converter `0x9269D0` therefore treats the input as a direct angle
  delta. It is not a synthetic stick: no acceleration, assist or decay applies, and the pad block
  is skipped. `RestoreLook` at `0x91B7A7` (`:284-286`) puts every axis back except `+0x2D4`, which
  stays 0 until the next Begin (`d1_native_look.h:45-56`) `[C]`.
- When the mouse is idle, the record is untouched, so the controller or native KBM record drives
  look `[C]`.
- Fallback mode `D1_NATIVE_CURSOR_ONLY=1` (`:260-272`) is a synthetic right stick: it adds
  `counts*gain/dt` to `+0x2CC/+0x2D4` in pad mode, so stick semantics apply `[C]`.
- No fork code decays or springs the y axis. Counts are consumed once per look pass
  (`g_fallback_look.Consume`) and cleared on menu, focus loss and capture loss `[C]`. Every
  centering and pull-back is game code.
- The fork writes `0x1ABBC53 = 1` while captured (`:212`), so the game's own KBM builder runs and
  selects KBM mode when the pad is idle. In gameplay the keyboard bindings to the virtual pad are
  suppressed (`input_handler.cpp:932-942`), so WASD movement reaches the game as native keys `[C]`.

### Fork interaction with 1a (matters only when the setting is on)

After `Restore`, `+0x2D0` holds the record's original value. In KBM mode that is the native mouse
pitch, which is nonzero when dy != 0, so the counter resets as designed. In pad mode it is 0. The
deferred `+0x2D4` is 0 in both modes. In pad mode, then, mouse look never resets the centering
counter, and centering would fight the mouse continuously `[C]`, `[I]`.

## 4. Why it can still run with the option OFF (ranked)

Statically, 1a cannot fire on the `0x7432B0` path while table `+0x9A` is 0: the flag is cleared
every frame. A pull with the option off is therefore one of these:

1. **1c soft-limit spring** `[I]`. It fits "looking at the floor and it keeps getting pulled". It
   is not gated by the option, and a mouse leaves pitch input at 0 on most frames. It does not need
   movement, so check it standing still.
2. **1b content target** `[I]`. It is speed-scaled ("while moving") and ignores the option. The
   target is not necessarily 0, and a negative target would also explain the one-off drag down.
3. **Table byte not actually 0** `[U]`. The menu may toggle a pending copy and not
   `[0x26A3790]+0x28`. The polarity may be inverted. Or `0x57CC80` may return 0, so
   `0x924960` stops refreshing and the table keeps an older value. The server stores the 701
   account-profile slice byte for byte (`server/crates/d1-account/src/objects/account.rs`
   `merge_account_profile`) `[C]`, but whether that slice is the options object above is `[U]`.
4. **Latched `P+0xE8` on the `0x91C360` path** `[C]`/`[I]`. That path never writes `P+0xE8`.
   `0x91D1F0` takes it when the session player record is absent or the `vfunc6` toggle is set
   (selection in `0x91D1F0`; the `0x91C360` call is at `0x91D5CA`, the `0x7432B0` call at
   `0x91D6FE`), so a flag latched earlier would persist.

### Secondary: the one-off drag down

- 1b with a negative target `[I]`.
- Recoil recovery in `0x919860`. `P+0xF0/+0xF4` (`param_6[0x34/0x35]`) is decayed back into the
  look delta. The "player already compensated" test at `0x91AA17..` compares against
  `out[0xB] = record+0x2D4` (stick or key axis), never the mouse. Mouse compensation is therefore
  not credited, and the recovery can pull below the aim point after firing `[I]`.
- No nonzero-dy leak was found in the fork feed `[C]`.

## 5. Smallest fix

**Step 0, live read (no code), to tell 1a, 1b and 1c apart.** Addresses are eboot base + vaddr:

| what | address | expect |
|---|---|---|
| auto-centering setting, user 0 | `0x27259B0 + 0x9A` (u8) | 0 with the option OFF |
| options source byte | `[0x26A3790] + 0x28` (u8) | same as above |
| pitch | `*(u64*)0x2725798 + 0x3C` (f32) | |
| centering active / counter | `P+0xE8` (u8) / `P+0xEC` (i32) | 1 means 1a is live |
| KBM record | `0x2725C50`: `+0x2A0` mode, `+0x2C0` move, `+0x2D0`, `+0x2D4` | |
| tunables | `T+0x1D4`, `T+0x1D8`, `T+0x134/0x138/0x13C` | |

Tests:

- **A:** stand still, look about 80° down, leave the mouse idle. Pitch rises to a fixed band edge
  → 1c.
- **B:** look about 30° down, hold W for 3 s, mouse idle. Pitch decays to 0 and `P+0xE8 == 1`
  → 1a. Pitch decays and `P+0xE8 == 0` → 1b.

**Fix for 1a + 1b: one new pinned detour in the existing batch**
(`d1_native_mouse.cpp`, next to `AssistSite`, appended to `requests` at `:410`):

```
site  0x91FE67
span  C5 F8 57 C0 C5 FA 10 8D 80 FD FF FF   ; vxorps xmm0,xmm0,xmm0 ; vmovss xmm1,[rbp-0x280]
```

Callback: if capture is active, not in cursor-only mode, and `regs->r13 == 0` (player 0), write
`0.0f` to `[rbp-0x280]` and `0` to byte `[r14 + r13 + 0xE8]`. The displaced load then sees target
0, and `0x91FE84 je 0x91FFDC` skips both branches. `[rbp-0x280]` is next written before it is read
(`0x92017A`), so zeroing it is safe. No jump in the function targets `0x91FE67..0x91FE84`. Register
use checked in the disassembly: `r14` = player array base, `r13` = player*0x1A0 `[C]`. Pad play is
unaffected while capture is off.

**Fix for 1c, only if test A shows it:** add a second detour at `0x9202CF`, span
`C5 FA 10 9D A0 FD FF FF` (`vmovss xmm3,[rbp-0x260]`). The callback writes `0.0f` to
`[rbp-0x260]` for player 0 under capture. `0x78B50` returns pitch unchanged when rate <= 0, and the
hard clamp to the current limits `P+0x164/P+0x168` at `0x92014C..0x920150` still runs first `[C]`.

**Rejected:**

- Setting `+0x2A0`: it does not touch these paths.
- Feeding pitch through the KBM delta: already done.
- Forcing table `+0x9A = 0` in `InjectAtSample`: one line, but it only covers 1a on the
  `0x7432B0` path.
- Changing `T[+0x1D4]` or `T[+0x13C]`: these are global and would also change controller play.

**Verify after the fix:** log once from the callback when `[rbp-0x280] != 0` or `P+0xE8` was 1, to
identify which branch fired. Repeat tests A and B: pitch `P+0x3C` should hold within 0.001 rad over
3 s of W with the mouse idle. With capture toggled off, a physical pad should still auto-centre if
the option is on.
