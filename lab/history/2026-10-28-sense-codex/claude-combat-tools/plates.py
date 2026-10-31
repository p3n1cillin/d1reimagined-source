"""Find enemy nameplate health bars (thin red horizontal bars) in a window capture."""
import numpy as np


def plates(img):
    """[(cx, cy, width)] of red bars 30-260 px wide and 3-12 px tall, outside the fixed HUD."""
    H, W, _ = img.shape
    r, g, b = img[:, :, 0], img[:, :, 1], img[:, :, 2]
    red = (r > 150) & (g < 90) & (b < 90) & (r > 2.2 * g)
    red[:, :] &= True
    red[150:205, W // 2 - 420:W // 2 + 420] = False   # the player's own health bar
    yy, xx = np.ogrid[:340, :340]
    red[:340, :340] &= (yy - 189) ** 2 + (xx - 185) ** 2 > 115 ** 2   # radar disc only
    red[H - 330:, :480] = False                       # weapon and ammo HUD
    red[H - 260:, W - 480:] = False                   # overlay corner
    out = []
    rows = np.nonzero(red.sum(axis=1) >= 8)[0]
    if len(rows) == 0:
        return out
    # group consecutive rows into bands
    bands, start = [], rows[0]
    for a, b2 in zip(rows, rows[1:]):
        if b2 != a + 1:
            bands.append((start, a)); start = b2
    bands.append((start, rows[-1]))
    for y0, y1 in bands:
        if not 3 <= y1 - y0 + 1 <= 16:
            continue
        cols = np.nonzero(red[y0:y1 + 1].any(axis=0))[0]
        # split columns into runs
        runs, s = [], cols[0]
        for a, b2 in zip(cols, cols[1:]):
            if b2 > a + 3:
                runs.append((s, a)); s = b2
        runs.append((s, cols[-1]))
        for x0, x1 in runs:
            if not 8 <= x1 - x0 + 1 <= 300 or y0 < 30:
                continue
            # A nameplate bar is a solid rectangle of one red (190, 54, 42); damage arrows are triangles.
            box = img[y0:y1 + 1, x0:x1 + 1]
            solid = ((abs(box[:, :, 0] - 190) < 30) & (abs(box[:, :, 1] - 54) < 25) & (abs(box[:, :, 2] - 42) < 25)).mean()
            if solid < 0.75:
                continue
            # A nameplate has the enemy's name in white just above its bar.
            above = img[y0 - 30:y0 - 3, x0:min(x0 + 120, W)]
            if (above.min(axis=2) > 200).sum() < 25:
                continue
            out.append(((x0 + x1) // 2, (y0 + y1) // 2, x1 - x0 + 1))
    return out
