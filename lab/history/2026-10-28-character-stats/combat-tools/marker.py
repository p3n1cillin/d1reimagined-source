"""Find the objective waypoint diamond (white core, black ring, grey outer diamond) in a window capture."""
import numpy as np


def _blobs(mask, max_blobs=400):
    """Connected 4-neighbour components of a boolean mask: [(ys, xs)] (small masks only)."""
    seen = np.zeros_like(mask)
    out = []
    for y0, x0 in zip(*np.nonzero(mask)):
        if seen[y0, x0]:
            continue
        stack, ys, xs = [(y0, x0)], [], []
        seen[y0, x0] = True
        while stack:
            y, x = stack.pop(); ys.append(y); xs.append(x)
            for dy, dx in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                v, u = y + dy, x + dx
                if 0 <= v < mask.shape[0] and 0 <= u < mask.shape[1] and mask[v, u] and not seen[v, u]:
                    seen[v, u] = True; stack.append((v, u))
            if len(ys) > 600:
                break
        out.append((np.array(ys), np.array(xs)))
        if len(out) >= max_blobs:
            break
    return out


def markers(img):
    """[(cx, cy, size)] of waypoint diamonds outside the fixed HUD."""
    a = img.astype(int)
    H, W, _ = a.shape
    white = a.min(axis=2) > 240
    white[:340, :340] = False            # radar
    white[H - 330:, :480] = False        # weapon HUD
    white[H - 260:, W - 480:] = False    # overlay corner
    white[140:215, :] = False            # health bar band
    out = []
    for ys, xs in _blobs(white):
        w, h = xs.max() - xs.min() + 1, ys.max() - ys.min() + 1
        if not (6 <= w <= 28 and 6 <= h <= 28 and 20 <= len(ys) <= 450):
            continue
        fill = len(ys) / (w * h)
        if abs(w - h) > 3 or not 0.38 <= fill <= 0.7:
            continue                      # a diamond core: square bounding box, half filled
        cy, cx = int(ys.mean()), int(xs.mean())
        r = max(w, h) // 2
        # the black ring just outside the white core, along the four diamond axes
        ring, grey, axes_dark = [], [], 0
        for dy, dx in ((0, 1), (0, -1), (1, 0), (-1, 0)):
            axis = []
            for k in range(r + 1, r + 5):
                y, x = cy + dy * k, cx + dx * k
                if 0 <= y < H and 0 <= x < W:
                    axis.append(a[y, x].max())
            ring += axis
            axes_dark += bool(axis) and min(axis) <= 70
            for k in range(r + 6, r + 11):
                y, x = cy + dy * k, cx + dx * k
                if 0 <= y < H and 0 <= x < W:
                    grey.append(a[y, x])
        if axes_dark < 4:
            continue
        g = np.array(grey)
        neutral = (g.max(axis=1) - g.min(axis=1) < 30) & (g.mean(axis=1) < 245)
        if neutral.mean() < 0.5:
            continue
        out.append((cx, cy, max(w, h)))
    return out
