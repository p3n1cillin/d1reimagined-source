"""Find the HUD vendor marker: a mid-blue diamond (~60-70 px) with a white icon at its centre.
Returns (x, y) in image pixels or None. Ignores the radar (top-left) and a panel docked bottom-right."""
import numpy as np
from PIL import Image


def _box(ii, r, x0, y0):
    """Sum of a mask over squares of side 2r+1 centred on every pixel, from an integral image."""
    h, w = ii.shape[0] - 1, ii.shape[1] - 1
    ys = np.clip(np.arange(h) - r, 0, h); ye = np.clip(np.arange(h) + r + 1, 0, h)
    xs = np.clip(np.arange(w) - r, 0, w); xe = np.clip(np.arange(w) + r + 1, 0, w)
    return ii[ye][:, xe] - ii[ys][:, xe] - ii[ye][:, xs] + ii[ys][:, xs]


def find(path, scale=4, require_text=True):
    """require_text=False also accepts a marker without its name (shown only when near or in focus);
    use it to steer once the strict test has found the vendor, not to find one."""
    a = np.asarray(Image.open(path).convert('RGB')).astype(int)[::scale, ::scale]
    r, g, b = a[..., 0], a[..., 1], a[..., 2]
    # the marker's flat UI blue, about (84, 117, 170): measured from a frame of Patches 99-40 (29 Oct)
    # Flare and fog tint it (to about (101, 132, 179) in sense63), so allow a band; the name text rejects scenery.
    blue = (r > 65) & (r < 120) & (g > r + 18) & (g < r + 45) & (b > g + 35) & (b > 150) & (b < 205)
    white = (r > 215) & (g > 215) & (b > 215)
    h, w = blue.shape
    for m in (blue, white):
        m[: 330 // scale, : 330 // scale] = False
        m[-300 // scale:, -300 // scale:] = False
    ib = np.pad(blue.cumsum(0).cumsum(1), ((1, 0), (1, 0))); iw = np.pad(white.cumsum(0).cumsum(1), ((1, 0), (1, 0)))
    ring = _box(ib, 30 // scale, 0, 0)            # blue around the centre (the diamond body)
    core = _box(iw, 8 // scale, 0, 0)             # white at the centre (the icon)
    area = (2 * (30 // scale) + 1) ** 2
    score = np.where((ring > 0.25 * area) & (ring < 0.75 * area) & (core >= 2), ring + 4 * core, 0)
    # An on-screen vendor marker carries its name: white text in a band just right of the diamond. Lights
    # and the green waypoint on a blue wall have none, so take the best candidate that has text.
    full = np.asarray(Image.open(path).convert('L')).astype(int)
    for idx in np.argsort(score, axis=None)[::-1][:40]:
        if score.flat[idx] == 0:
            break
        y, x = np.unravel_index(idx, score.shape); X, Y = int(x * scale), int(y * scale)
        if Y > 720:                                 # markers float above a head, never in the bottom third
            continue
        band = full[max(0, Y - 28):Y + 28, X + 50:X + 320]
        if not require_text or (band.size and (band > 215).sum() > 120 and (band > 215).mean() < 0.35):
            return X, Y
    return None


def find_waypoint(path, scale=2, require_text=True):
    """The green objective waypoint: a flat green border, about (83, 169, 88) (sense63), around a white
    diamond, with its distance ("23 m") to the right when on screen. Off-screen it is an edge arrow
    without text; require_text=False accepts that too (for turning toward it)."""
    a = np.asarray(Image.open(path).convert('RGB')).astype(int)[::scale, ::scale]
    r, g, b = a[..., 0], a[..., 1], a[..., 2]
    green = (r > 30) & (r < 115) & (g > 78) & (g < 200) & (g > r + 35) & (g > b + 28) & (b < 120)   # on screen ~(83,169,88); edge ~(44,95,49)
    white = (r > 215) & (g > 215) & (b > 215)
    for m in (green, white):
        m[: 330 // scale, : 330 // scale] = False
        m[-300 // scale:, -300 // scale:] = False
    ig = np.pad(green.cumsum(0).cumsum(1), ((1, 0), (1, 0))); iw = np.pad(white.cumsum(0).cumsum(1), ((1, 0), (1, 0)))
    ring = _box(ig, 22 // scale, 0, 0)
    core = _box(iw, 6 // scale, 0, 0)
    score = np.where((ring >= 12) & (core >= 3), ring + 2 * core, 0)
    full = np.asarray(Image.open(path).convert('L')).astype(int)
    for idx in np.argsort(score, axis=None)[::-1][:40]:
        if score.flat[idx] == 0:
            break
        y, x = np.unravel_index(idx, score.shape); X, Y = int(x * scale), int(y * scale)
        band = full[max(0, Y - 16):Y + 16, X + 30:X + 140]
        if not require_text or (band.size and 25 < (band > 215).sum() and (band > 215).mean() < 0.3):
            return X, Y
    return None


if __name__ == '__main__':
    import sys
    fn = find_waypoint if sys.argv[1] == '--waypoint' else find
    for p in sys.argv[1:]:
        if not p.startswith('--'):
            print(p.rsplit('/', 1)[-1], fn(p))
