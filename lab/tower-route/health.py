"""The player's health from the top-centre bar: None when no bar is shown (full health), else 0..1.

Images are RGB arrays (fight.grab), the bar's fill white or, when critical, red."""
import os

import numpy as np

X0, X1 = 752, 1184          # bar extent in the 1936x1119 window capture (centred on the reticle, x 968)

# The emulator's own frame (F12, what fight.grab returns in posted mode) puts the 1920x1080 game image
# at (8, 31) in the 1936x1119 capture (a 31 px black band above it, 8 px borders), and this build
# draws the health bar bottom-left, slanted, in yellow. All BL_* geometry and the colour ranges below
# were measured on build 56578 emulator frames from one lab run (tuning, not game data): full bar
# from x 101 to 568, its 16 px band at y 883-899 at the left end and 863-878 at the right.
# They apply to those frames only, so they are used in posted mode (D1_KEYS_POST=1), whose frames
# these are; desktop grabs keep the original top-centre reader.
BL_X0, BL_X1 = 101, 569     # the bar's left end and one past its full right end
BL_Y_LEFT, BL_Y_RIGHT = 891.0, 870.5   # the bar's centre row at x 110 and at x 568
BL_HALF = 5                 # rows sampled either side of the centre line


def emulator_frames():
    """Whether frames are the emulator's own (posted mode), as opposed to desktop grabs."""
    return os.environ.get('D1_KEYS_POST') == '1'


def _fill(px):
    r, g, b = px[..., 0], px[..., 1], px[..., 2]
    # the flat yellow (243-247, 220-230, 56-134: the cap, the body, a hit flash): a tight match, so a
    # bright wall behind a translucent bar (fire, sand) is not read as fill
    yellow = (r > 225) & (g > 205) & (g < 245) & (b > 40) & (b < 150) & (r - g < 40)
    red = (r > 150) & (r - np.maximum(g, b) > 80)
    return yellow, red


def bottom_left(a):
    """The health fraction 0..1 from the bottom-left bar of the emulator's frame, or None if no bar.

    The fill is yellow (red is accepted too, as the top bar's fill is) and runs from the
    left end along the slanted bar. A bar has ends: the columns just left of it and just past the
    fill are not fill, so a uniformly yellow scene is not a bar. An empty bar (no fill, the
    ability icons under it drawn, the band dark) reads 0.0."""
    h, w = a.shape[:2]
    if h < 900 or w < BL_X1 + 12:
        return None
    xs = np.arange(BL_X0, BL_X1)
    yc = np.rint(BL_Y_LEFT + (BL_Y_RIGHT - BL_Y_LEFT) * (xs - 110) / (568 - 110)).astype(int)
    rows = yc[:, None] + np.arange(-BL_HALF, BL_HALF + 1)[None, :]
    px = a[rows, xs[:, None]]                         # (columns, rows, rgb)
    yellow, red = _fill(px)
    # A red fill is a stripe: the rows above and below it (12-14 px off the centre line) are
    # not red or yellow, so a red wall or a uniformly red screen is not a bar.
    off = np.concatenate([yc[:, None] - np.arange(12, 15)[None, :], yc[:, None] + np.arange(12, 15)[None, :]], axis=1)
    oy, orr = _fill(a[np.clip(off, 0, h - 1), xs[:, None]])
    lit = (yellow.mean(axis=1) >= 0.6) | ((red.mean(axis=1) >= 0.6) & ((oy | orr).mean(axis=1) < 0.3))
    if lit[:3].mean() < 0.6:
        return _empty(a, lit)
    # the longest run from the left end, tolerating a gap of up to 3 px (the bar's glow and text)
    n, gap = 0, 0
    for i, v in enumerate(lit):
        if v:
            n, gap = i + 1, 0
        else:
            gap += 1
            if gap > 3:
                break
    # the ends: 3-10 px left of the bar and 4-10 px past the fill must not be yellow
    def margin(x_from, x_to):
        mx = np.arange(x_from, x_to)
        my = np.rint(BL_Y_LEFT + (BL_Y_RIGHT - BL_Y_LEFT) * (mx - 110) / (568 - 110)).astype(int)
        mp = a[my[:, None] + np.arange(-BL_HALF, BL_HALF + 1)[None, :], mx[:, None]]
        return float((_fill(mp)[0].mean(axis=1) >= 0.6).mean())   # yellow only: a bright wall may be red or white
    if margin(BL_X0 - 10, BL_X0 - 3) > 0.5 or margin(BL_X0 + n + 4, BL_X0 + n + 11) > 0.5:
        return None
    return min(1.0, n / (BL_X1 - BL_X0 - 1))


def _empty(a, lit):
    """0.0 for a bar with no fill at all: the ability icons below it are drawn (blue squares) and the
    band is dark. None otherwise (no HUD, or a fading bar)."""
    icons = a[895:985, 95:260]
    blue = (icons[..., 2] > 130) & (icons[..., 0] < 110) & (icons[..., 2] - icons[..., 0] > 50)
    if blue.sum() < 3000 or lit.any():
        return None
    xs = np.arange(BL_X0, BL_X1)
    yc = np.rint(BL_Y_LEFT + (BL_Y_RIGHT - BL_Y_LEFT) * (xs - 110) / (568 - 110)).astype(int)
    band = a[yc[:, None] + np.arange(-BL_HALF, BL_HALF + 1)[None, :], xs[:, None]]
    return 0.0 if band.mean() < 90 else None


def health(img):
    a = img.astype(int)
    if emulator_frames():
        bl = bottom_left(a)
        if bl is not None:
            return bl
    mid = a[176:188, X0:X1].mean(axis=(0, 2))
    out = (a[164:170, X0:X1].mean(axis=(0, 2)) + a[194:200, X0:X1].mean(axis=(0, 2))) / 2
    band = a[177:187, X0:X1]
    # The fill is white, and red when health is critical: a grey-only test read a red bar as no bar
    # (full health), so the Guardian fought on at 0.78 -> None until it died (sense108's corridor).
    grey = band.std(axis=2).mean(axis=0) < 30
    red = (band[:, :, 0] - band[:, :, 1:].max(axis=2)).mean(axis=0) > 50
    lit = (mid - out > 10) & (grey | red) & (band.mean(axis=2).std(axis=0) < 30)
    # The bar's left end must be lit. 48 px was asked for at first, but at 0.10 the bar is 43 px:
    # sense108 read None (no bar: full health) there while dying.
    if lit[:20].mean() < 0.8:
        return sliver(a)
    # The fill's own brightness, from every lit column: the first 20 alone were skewed by a light
    # behind the bar's left end (sense123: 198 there, 157 along the rest), which cut a bar about
    # three-quarters full to 0.30.
    ref = np.median(mid[lit])
    lit &= mid > ref - 30
    # the white fill runs from the left end: health is the longest prefix that is at least 75% lit
    frac = np.cumsum(lit) / np.arange(1, len(lit) + 1)
    ok = np.nonzero((frac >= 0.75) & lit)[0]
    return float(ok.max() + 1) / (X1 - X0)


def drawn(a):
    """Whether the health bar is on screen at all (its outline, row 174): it hides at full health."""
    line = a[173:176, X0:X1].mean(axis=(0, 2))
    around = (a[166:171, X0:X1].mean(axis=(0, 2)) + a[178:183, X0:X1].mean(axis=(0, 2))) / 2
    return ((line - around) > 8).mean() >= 0.8


def sliver(a):
    """A red fill too short for the 20 px test, or none, inside a drawn bar: else None (no bar).

    sense116's hangar deaths read None (full health) with the bar showing 4-6 px of red, and once
    48 px of pale red over an orange wall. The bar's outline, a row at y 174 about 30 levels
    brighter than the rows around it across the whole bar, says a bar is drawn; the fill is the
    red run from its left end (red over green and blue by 60 or more; the empty track is 10-35).
    """
    if not drawn(a):
        return None
    band = a[177:187, X0:X1]
    red = (band[:, :, 0] - band[:, :, 1:].max(axis=2)).mean(axis=0) > 60
    run = int(np.argmin(red)) if not red.all() else len(red)
    # A drawn bar with no fill at all is at about zero (sense116's junction death): the bar only
    # shows below full health, so at worst this misreads as a back-off, never as a full bar.
    return run / (X1 - X0) if run >= 2 else 0.0


def critical(img):
    """Whether the screen shows the game's critical-health effect: the frame's edges turn red.

    Measured on sense108's recordings (2 Nov): the share of edge pixels (outside the middle 60%)
    whose red exceeds both green and blue by 100 was 0.119-0.522 on the critical frames and at most
    0.003 on ordinary ones, the corridor's red lamps and a close rust wall included (that wall gave
    0.226 at a margin of 60). A far stronger sign than the bar, which is small and hides at full.
    """
    a = img.astype(int)
    # Only with the bar on screen: at critical health it always shows (a red sliver), and red light
    # alone is not enough. The tripmine corridor's red lasers read as critical with the bar hidden,
    # and every walk there stopped "hurt" at full health (sense124).
    # Emulator frames: the bar is always drawn, so it says nothing; critical health needs a bar
    # below full (a red-lit screen at full health, sense124's lasers, is not critical).
    if emulator_frames():
        bl = bottom_left(a)
        if not drawn(a) and (bl is None or bl >= 0.99):
            return False
    elif not drawn(a):
        return False
    h, w = a.shape[:2]
    edge = np.ones((h, w), bool)
    edge[h // 5:4 * h // 5, w // 5:4 * w // 5] = False
    e = a[edge]
    return float(((e[:, 0] - e[:, 1:].max(axis=1)) > 100).mean()) > 0.05
