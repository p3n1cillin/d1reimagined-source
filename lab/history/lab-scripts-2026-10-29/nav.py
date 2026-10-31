"""Graph navigation over positions the Guardian has actually walked (56578, native position).

    nav.py TARGET [--walk|--sprint] [--max-s 180]
TARGET is a node name (gate, garage, orange, corridor, fightroom, nux, bay, ship) or "x,y".
Finds the node nearest the current position, plans the shortest path (Dijkstra) and walks it with
run.py's continuous steering. Each edge was walked successfully at least once; walk mode by
default, because sprint taps have set off the arc pose. Exit 3 on a unit change (death).
"""
import argparse, heapq, importlib.util, math, os, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
SENSE = os.path.join(HERE, 'pr57')

# name: (x, y). Edges below are undirected and were walked in sense40-sense42. The NUX doorway is
# crossed heading WSW (nux -> gap); (102.4, -539.3) is the room's corner and traps the walker.
NODES = {
    # Divide approach to the gate
    'd_entry': (349.0, -648.0), 'd_mid': (243.4, -621.4), 'drop2': (198.7, -575.6), 'pre_gate': (176.9, -585.9),
    'gate_out': (157.8, -602.6), 'gate': (146.2, -608.2),
    # garage interior
    'garage': (142.8, -606.1), 'garage_in': (131.4, -600.7), 'platform': (124.2, -597.9), 'plat_n': (123.1, -591.3),
    'north_door': (113.8, -579.6), 'bright': (121.4, -575.6), 'orange': (124.0, -574.0), 'or_n': (126.0, -571.5),
    # Dock 13
    'corridor': (132.2, -567.4), 'corr_e': (135.1, -564.0), 'corr_n': (134.9, -554.3), 'netroom': (133.6, -549.5),
    'net_w': (121.9, -549.2), 'fightroom': (117.0, -544.5), 'fr_nw': (115.6, -542.1), 'fr_n': (114.5, -540.5),
    'pass1': (111.0, -536.4), 'pass2': (109.0, -534.4), 'nux': (105.7, -536.0), 'gap': (100.6, -537.1),
    'bay_e': (95.2, -538.2), 'bay': (89.2, -540.0), 'bay_s': (90.0, -553.0), 'ship': (85.0, -560.0),
}
EDGES = [
    ('d_entry', 'd_mid'), ('d_mid', 'drop2'), ('drop2', 'pre_gate'), ('pre_gate', 'gate_out'), ('gate_out', 'gate'),
    ('gate', 'garage'), ('garage', 'garage_in'), ('garage_in', 'platform'), ('platform', 'plat_n'),
    ('plat_n', 'north_door'), ('north_door', 'bright'), ('bright', 'orange'), ('orange', 'or_n'), ('bright', 'or_n'),
    ('or_n', 'corridor'), ('corridor', 'corr_e'), ('corr_e', 'corr_n'), ('corr_n', 'netroom'), ('netroom', 'net_w'),
    ('net_w', 'fightroom'), ('fightroom', 'fr_nw'), ('fr_nw', 'fr_n'), ('fr_n', 'pass1'), ('pass1', 'pass2'),
    ('pass2', 'nux'), ('nux', 'gap'), ('gap', 'bay_e'), ('bay_e', 'bay'), ('bay', 'bay_s'), ('bay_s', 'ship'),
]
# The Divide leg between d_entry and pre_gate follows divide_crossing.json, not a straight line.
ROUTE_FILES = {('d_entry', 'd_mid'): 'divide_crossing.json', ('d_mid', 'drop2'): 'divide_crossing.json',
               ('drop2', 'pre_gate'): 'divide_crossing.json'}


def graph():
    adj = {n: [] for n in NODES}
    for a, b in EDGES:
        d = math.dist(NODES[a], NODES[b])
        adj[a].append((b, d)); adj[b].append((a, d))
    return adj


def plan(start, goal):
    adj, dist, prev, q = graph(), {start: 0.0}, {}, [(0.0, start)]
    while q:
        d, n = heapq.heappop(q)
        if n == goal: break
        if d > dist.get(n, 1e18): continue
        for m, w in adj[n]:
            if d + w < dist.get(m, 1e18):
                dist[m], prev[m] = d + w, n; heapq.heappush(q, (d + w, m))
    path, n = [goal], goal
    while n != start:
        n = prev[n]; path.append(n)
    return path[::-1]


def divide_points(a, b):
    import json
    r = json.load(open(os.path.join(SENSE, 'divide_crossing.json')))
    ia = min(range(len(r)), key=lambda k: math.dist(r[k][:2], NODES[a]))
    ib = min(range(len(r)), key=lambda k: math.dist(r[k][:2], NODES[b]))
    seg = r[ia:ib + 1] if ia <= ib else r[ib:ia + 1][::-1]
    return [tuple(p[:2]) for p in seg[::2]]


# Behind the NUX room's counter, just south of the trapping corner, is a pocket that hangar combat
# can push the Guardian into (sense65: 40 min of seeks went nowhere). Walking out is blocked on every
# side; it leaves by jumping east over the counter while walking, landing at about OVER.
POCKET, OVER = (103.8, -540.8), (107.2, -543.0)


def escape_pocket(w, c):
    import threading
    stop = threading.Event()
    def jumps():
        while not stop.wait(1.2):
            subprocess.run(['powershell', '-NoProfile', '-File', r'F:\d1\tools\d1-pad.ps1', '-Keys', 'Hold:NumPad2:300'],
                           capture_output=True)
    t = threading.Thread(target=jumps, daemon=True); t.start()
    try:
        subprocess.run([sys.executable, '-u', os.path.join(HERE, 'run.py'), f'{OVER[0]},{OVER[1]}', '--max-s', '20',
                        '--tol', '1.5', '--walk'], capture_output=True, text=True, timeout=50)
    except subprocess.TimeoutExpired:
        pass
    stop.set(); t.join(5)
    subprocess.run([sys.executable, os.path.join(HERE, 'release.py')])
    here = w.position(c, None)[:2]
    print(f'pocket escape: now {here[0]:.1f},{here[1]:.1f}', flush=True)
    return here


def main():
    ap = argparse.ArgumentParser(); ap.add_argument('target'); ap.add_argument('--sprint', action='store_true')
    ap.add_argument('--max-s', type=float, default=180); a = ap.parse_args()
    s = importlib.util.spec_from_file_location('w', os.path.join(SENSE, 'd1-walk.py')); w = importlib.util.module_from_spec(s); s.loader.exec_module(w)
    c = w.Client(); c.native_unit = True
    here = w.position(c, None)[:2]
    if math.dist(here, POCKET) < 2.0:
        here = escape_pocket(w, c)
    start = min(NODES, key=lambda n: math.dist(NODES[n], here))
    if start in ('nux', 'gap') and here[1] < -538.5:   # south of the doorway the counter is in the way
        start = 'fr_n'
    goal = a.target if a.target in NODES else min(NODES, key=lambda n: math.dist(NODES[n], tuple(map(float, a.target.split(',')))))
    path = plan(start, goal)
    pts = []
    for x, y in zip(path, path[1:]):
        pts += divide_points(x, y) if (x, y) in ROUTE_FILES or (y, x) in ROUTE_FILES else [NODES[y]]
    if math.dist(here, NODES[start]) > 1.5:
        pts.insert(0, NODES[start])
    print(f'from {here[0]:.1f},{here[1]:.1f} via {" > ".join(path)}', flush=True)
    cmd = [sys.executable, '-u', os.path.join(HERE, 'run.py'), ';'.join(f'{x},{y}' for x, y in pts),
           '--max-s', str(a.max_s), '--tol', '1.5'] + ([] if a.sprint else ['--walk'])
    # The walker honours --max-s; the margin only covers its startup. Never outlive our own caller.
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=a.max_s + 30)
    except subprocess.TimeoutExpired:
        subprocess.run([sys.executable, os.path.join(HERE, 'release.py')])
        print('timeout: walker overran its budget and was stopped; keys released', flush=True)
        sys.exit(1)
    out = [l for l in r.stdout.splitlines() if not l.startswith('shadPS4')]
    print(out[-1] if out else r.stderr[-300:], flush=True)
    sys.exit(r.returncode)


if __name__ == '__main__':
    main()
