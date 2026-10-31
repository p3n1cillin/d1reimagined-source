# Destiny 1 native mouse patch

This build contains an experimental, default-off native relative-mouse path for **CUSA00219 patch
01.29**. It is deliberately title- and byte-pinned; another serial, app version, or instruction
span is refused before guest execution.

## Data path

SDL3 supplies relative motion on Windows, Linux/Wayland, and Linux/X11. The SDL thread adds each
high-resolution x/y pair to one cache-line-aligned, lock-free 64-bit mailbox. D1's local-player
sample at guest `0x91AB3C` drains the mailbox after controller shaping and adds the motion to the
engine-owned look-rate fields at `+0x2CC/+0x2D4`. The game's normal converter at `0x9269D0` and its
normal aim/camera consumers remain in charge. There is no 33 ms timer and no virtual-stick
quantization.

At 240 Hz the guest drains at most once per 4.17 ms input sample. The mailbox exchange itself is a
single lock-free operation; actual end-to-end latency is bounded primarily by the next SDL event,
the next D1 input sample, and presentation.

## Controls

For a direct game launch, including preservation launchers that use `-g` and never display
shadPS4's launcher screen:

```text
shadps4 --d1-native-mouse --d1-native-mouse-sensitivity 1.0 -p PATCH -g EBOOT
```

Add `--d1-native-mouse-invert-x` / `--d1-native-mouse-invert-y` if desired. The default polarity was calibrated on the first live flight (2026-08-29): raw SDL x is applied NEGATED to yaw and raw SDL y is applied as-is to pitch; the invert switches flip from that calibrated default. These switches are process-local. Explicit CLI
sensitivity/invert options take precedence over values loaded from a profile. The title, version,
and byte pins still apply.

In Input settings:

- Enable `Destiny 1 Native Mouse (01.29)` for the game's profile.
- Relaunch the game after enabling it or changing sensitivity/invert settings; the exact-build
  detour and its settings snapshot are installed before guest threads start.
- Adjust `D1 Native Mouse Sensitivity`; `1.0` is the baseline and D1's own context/ADS look scaling
  remains downstream.
- `D1 Native Mouse Invert X` / `D1 Native Mouse Invert Y` change only native mouse yaw / pitch.
- Bind `hotkey_toggle_d1_native_mouse` if a runtime capture/release toggle is desired.

The frontend requests SDL relative mode only after the exact-build detour commits. Pausing, losing
window focus, or releasing capture clears pending motion. Activating this path disables the older
mouse-to-joystick polling mode so the same relative deltas cannot be consumed twice. Mouse buttons
and wheel events continue through shadPS4's ordinary keyboard/mouse binding path.

## Validation status

The mailbox and detour pin have offline controls and the Linux build is compile-tested. Windows,
Wayland, and X11 share the same SDL event code, but each backend still needs a live smoke run.
Before treating this as competitive-ready, validate hip-fire, ADS, scoped weapons, recoil, aim
assist, menus/overlays, respawn, focus loss, pause/resume, and 30/60/120/240 Hz frame pacing. Also
confirm with a gameplay trace that authoritative weapon direction and the first-person camera stay
identical; this patch intentionally does not claim that live gate in advance.
