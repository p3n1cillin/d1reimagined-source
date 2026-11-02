"""The player's health from the top-centre bar: None when no bar is shown (full health), else 0..1.

Images are RGB arrays (fight.grab), the bar's fill white or, when critical, red."""
import numpy as np

X0, X1 = 752, 1184          # bar extent in the 1936x1119 window capture (centred on the reticle, x 968)


def health(img):
    a = img.astype(int)
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
    ref = np.median(mid[:20])
    lit &= mid > ref - 30
    # the white fill runs from the left end: health is the longest prefix that is at least 75% lit
    frac = np.cumsum(lit) / np.arange(1, len(lit) + 1)
    ok = np.nonzero((frac >= 0.75) & lit)[0]
    return float(ok.max() + 1) / (X1 - X0)


def sliver(a):
    """A red fill too short for the 20 px test, or none, inside a drawn bar: else None (no bar).

    sense116's hangar deaths read None (full health) with the bar showing 4-6 px of red, and once
    48 px of pale red over an orange wall. The bar's outline, a row at y 174 about 30 levels
    brighter than the rows around it across the whole bar, says a bar is drawn; the fill is the
    red run from its left end (red over green and blue by 60 or more; the empty track is 10-35).
    """
    line = a[173:176, X0:X1].mean(axis=(0, 2))
    around = (a[166:171, X0:X1].mean(axis=(0, 2)) + a[178:183, X0:X1].mean(axis=(0, 2))) / 2
    if ((line - around) > 8).mean() < 0.8:
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
    h, w = a.shape[:2]
    edge = np.ones((h, w), bool)
    edge[h // 5:4 * h // 5, w // 5:4 * w // 5] = False
    e = a[edge]
    return float(((e[:, 0] - e[:, 1:].max(axis=1)) > 100).mean()) > 0.05
