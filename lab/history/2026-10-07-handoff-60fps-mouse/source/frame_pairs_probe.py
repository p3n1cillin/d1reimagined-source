# SPDX-License-Identifier: GPL-2.0-or-later
# Runs on the seat, in native/, while the client runs: captures N consecutive presented frames
# through the renderer's frame dump (out/frame.ppm.request -> out/frame.ppm) and reports, for
# each neighbouring pair, the time between the captures, whether the two frames are identical, and
# how much really changed (deltas above the dither) in each of 6x4 tiles. With the camera still, a
# region that changes on every pair moves at the frame rate; one that holds across a pair did not
# advance. The capture rhythm is the dump mechanism's own (two fast, one slow): compare content,
# not gaps. Record: docs/operations/native-client-20261004/CADENCE-20261006.md.
# Usage: python3 tools/frame_pairs_probe.py [frames]
import os, sys, time

count = int(sys.argv[1]) if len(sys.argv) > 1 else 60
path, request = "out/frame.ppm", "out/frame.ppm.request"
cols, rows_n = 6, 4

def capture(deadline=1.5):
    try:
        os.remove(path)
    except FileNotFoundError:
        pass
    open(request, "w").close()
    t0 = time.perf_counter()
    while time.perf_counter() - t0 < deadline:
        if os.path.exists(path):
            with open(path, "rb") as f:
                data = f.read()
            parts = data[:40].split()
            if len(parts) >= 4 and parts[0] == b"P6":
                header = sum(len(p) for p in parts[:4]) + 4
                need = header + int(parts[1]) * int(parts[2]) * 3
                if len(data) >= need:
                    return time.perf_counter(), data[:need], (int(parts[1]), int(parts[2]), header)
        time.sleep(0.0005)
    return None, None, None

frames, times, dims = [], [], None
for i in range(count):
    t, data, d = capture()
    if data is None:
        print("capture timed out at frame", i)
        break
    frames.append(data); times.append(t); dims = d
print("captured", len(frames), "frames", dims[:2] if dims else "")
if len(frames) < 2:
    sys.exit(1)
w, h, header = dims
step = 331  # sample every 331st byte: ~17k samples a frame, ~700 a tile
samples = list(range(header, len(frames[0]), step))
tile_of = []
for k in samples:
    p = (k - header) // 3
    x, y = p % w, p // w
    tile_of.append((y * rows_n // h) * cols + (x * cols // w))
per_tile_total = [0] * (cols * rows_n)
for t in tile_of:
    per_tile_total[t] += 1

def symbol(frac):
    return "." if frac < 0.002 else ("o" if frac < 0.02 else "O")

# A sampled byte counts as changed only when it moved by more than the dither and noise the
# post-processing adds every frame (small deltas everywhere); animation moves pixels a lot.
threshold = 20

identical = 0
changes_by_tile = [0] * (cols * rows_n)
print("gap ms | same | tiles (6 across x 4 down, '.' <0.5% changed, 'o' <5%, 'O' more) | whole")
for i in range(1, len(frames)):
    a, b = frames[i - 1], frames[i]
    gap_ms = (times[i] - times[i - 1]) * 1000
    same = a == b
    identical += same
    changed = [0] * (cols * rows_n)
    for k, t in zip(samples, tile_of):
        if abs(a[k] - b[k]) > threshold:
            changed[t] += 1
    fracs = [c / max(1, n) for c, n in zip(changed, per_tile_total)]
    for t, f in enumerate(fracs):
        changes_by_tile[t] += f >= 0.002
    grid = " ".join("".join(symbol(fracs[r * cols + c]) for c in range(cols)) for r in range(rows_n))
    whole = sum(changed) / len(samples)
    print(f"{gap_ms:6.1f} | {'YES' if same else 'no '} | {grid} | {whole:.3f}")
pairs = len(frames) - 1
print("pairs:", pairs, " identical whole frames:", identical)
print("pairs with change, by tile (6 across x 4 down); a tile near pairs/2 changes every other frame:")
for r in range(rows_n):
    print("   " + " ".join(f"{changes_by_tile[r * cols + c]:3d}" for c in range(cols)))
