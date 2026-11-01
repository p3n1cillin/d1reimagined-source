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
    if lit[:48].mean() < 0.8:
        return None
    ref = np.median(mid[:48])
    lit &= mid > ref - 30
    # the white fill runs from the left end: health is the longest prefix that is at least 75% lit
    frac = np.cumsum(lit) / np.arange(1, len(lit) + 1)
    ok = np.nonzero((frac >= 0.75) & lit)[0]
    return float(ok.max() + 1) / (X1 - X0)
