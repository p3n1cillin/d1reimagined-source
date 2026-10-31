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
    # Solid nameplate red only, so damage arrows (other reds, triangles) rarely join a bar.
    solid = red & (np.abs(img[:, :, 0].astype(int) - 190) < 30) & (np.abs(img[:, :, 1].astype(int) - 54) < 25) & (np.abs(img[:, :, 2].astype(int) - 42) < 25)
    rows = np.nonzero(solid.sum(axis=1) >= 6)[0]
    if len(rows) == 0:
        return out
    # Per row, find horizontal runs; then stack runs that overlap in x on consecutive rows.
    boxes = []                                    # [x0, x1, y0, y1]
    for y in rows:
        cols = np.nonzero(solid[y])[0]
        runs, s0 = [], cols[0]
        for a, b2 in zip(cols, cols[1:]):
            if b2 > a + 3:
                runs.append((s0, a)); s0 = b2
        runs.append((s0, cols[-1]))
        for x0, x1 in runs:
            if x1 - x0 + 1 < 6:
                continue
            for bx in boxes:
                if bx[3] == y - 1 and x0 <= bx[1] and x1 >= bx[0] and abs(x0 - bx[0]) <= 30:   # near plates are drawn slanted
                    bx[0] = min(bx[0], x0); bx[1] = max(bx[1], x1); bx[3] = y; break
            else:
                boxes.append([x0, x1, y, y])
    for x0, x1, y0, y1 in boxes:
        if not 3 <= y1 - y0 + 1 <= 24 or not 6 <= x1 - x0 + 1 <= 300 or y0 < 30 or y0 > H - 150:
            continue
        box = solid[y0:y1 + 1, x0:x1 + 1]
        if box.mean() < (0.75 if y1 - y0 < 14 else 0.45):
            continue
        # A nameplate has the enemy's name in white just above its bar.
        above = img[max(0, y0 - 30):y0 - 3, x0:min(x0 + 120, W)]
        if (above.min(axis=2) > 200).sum() < 25:
            continue
        out.append(((x0 + x1) // 2, (y0 + y1) // 2, x1 - x0 + 1, y1 - y0 + 1))
    return out
