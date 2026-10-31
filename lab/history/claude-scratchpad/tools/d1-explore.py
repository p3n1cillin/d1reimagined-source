"""Find a way to a target point by pad input, and record it as a route for d1-walk.py (56578).

    d1-explore.py --tracked tracked.json --goal 347.4,-646.3 --out graveyard.json [--max-seconds 1800]

The walker follows known routes; this finds new ones. It works on a 2-unit grid: each step it scores
16 directions by how far they turn away from the goal and how often the cell they lead into has
been visited, skips directions that were already blocked from this cell, turns the camera there and
pushes forward. A push that does not move marks that direction blocked (after one jump try).
Revisiting cells makes their neighbours cheaper than the dead end, so it backs out of pockets on its
own. On arrival the breadcrumb trail (loops cut) is written as a route. Position tracking, the pad
and the keep-alive press come from d1-walk.py. No memory writes.
"""
import argparse, importlib.util, json, math, os, time

def load_walk():
    """Load the Windows driver only for a live run, not policy tests or --help."""
    spec = importlib.util.spec_from_file_location(
        'd1walk', os.path.join(os.path.dirname(os.path.abspath(__file__)), 'd1-walk.py'))
    walk = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(walk)
    return walk

CELL = 2.0
DIRS = 16


def available_directions(cell, blocked, lethal):
    """Fatal moves are permanent exclusions, independent of transient wall evidence."""
    return [k for k in range(DIRS)
            if (cell, k) not in lethal and blocked.get((cell, k), 0) < 2]


def reset_transient_blocks(cell, blocked, lethal):
    """Retry a dead end without forgetting fatal moves or changing other cells."""
    for k in range(DIRS):
        key = (cell, k)
        if key in lethal:
            blocked[key] = 99
        else:
            blocked.pop(key, None)
    return available_directions(cell, blocked, lethal)


def cell_of(p):
    return (round(p[0] / CELL), round(p[1] / CELL))


def simplify(trail):
    """Cut loops (a cell visited again drops everything since its first visit), then thin to ~3 units."""
    out, index = [], {}
    for p in trail:
        c = cell_of(p)
        if c in index:
            del out[index[c] + 1:]
            index = {cell_of(q): k for k, q in enumerate(out)}
            continue
        index[c] = len(out)
        out.append(p)
    thin = [out[0]] if out else []
    for p in out[1:]:
        if math.dist(p[:2], thin[-1][:2]) >= 3.0:
            thin.append(p)
    if out and thin[-1] != out[-1]:
        thin.append(out[-1])
    return [[round(v, 2) for v in p] for p in thin]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--tracked', required=True)
    ap.add_argument('--goal', required=True)
    ap.add_argument('--out', required=True)
    ap.add_argument('--reach', type=float, default=4.0)
    ap.add_argument('--max-seconds', type=float, default=1800)
    ap.add_argument('--rate', type=float, default=0.0022, help='camera turn, rad per ms of J/L')
    ap.add_argument('--crouch', action='store_true',
                    help='explore crouched (Circle toggled at the start, no jumps: a jump stands up again), '
                         'for low passages such as the mine corridor in the Wall')
    ap.add_argument('--stop-on-death', action='store_true',
                    help='end after a death (the caller replays the known route back to the frontier)')
    ap.add_argument('--lethal', help='JSON file of [cell x, cell y, direction] moves that killed the Guardian; '
                    'read at start, extended on each death')
    a = ap.parse_args()
    walk = load_walk()
    goal = [float(v) for v in a.goal.split(',')]
    c = walk.Client()
    tracked = json.load(open(a.tracked))
    here = walk.position(c, tracked)
    print('start', here, 'goal', goal)
    visits, blocked, jumped = {}, {}, set()
    lethal = set()
    if a.lethal and os.path.exists(a.lethal):
        lethal = {((x, y), kk) for x, y, kk in json.load(open(a.lethal))}
    for key in lethal:
        blocked[key] = 99
    trail = [list(here)]
    facing = None
    t_end = time.time() + a.max_seconds
    best = math.dist(here[:2], goal[:2])
    steps = 0

    still = 0

    def push(ms):
        nonlocal here, facing, tracked, still
        p0 = here
        walk.pad(f'Hold:W:{ms}')
        here = walk.position(c, tracked, p0) or p0
        d = math.dist(p0[:2], here[:2])
        if d > 0.6:
            facing = math.atan2(here[1] - p0[1], here[0] - p0[0])
            still = 0
        else:
            still += 1
        if d > 30 or still >= 10:
            # A respawn, or tracking frozen on copies left behind by one: keep only live copies.
            # Re-scan (if it comes to that) around the last good position, never around strays.
            good = here if walk.plausible(here) and d <= 30 else p0
            tracked = walk.retrack(c, tracked, good)
            here = walk.position(c, tracked) or here
            print(f'retracked ({"jump" if d > 30 else "no movement"}): {len(tracked)} copies at {here}', flush=True)
            still = 0
        return d, p0

    if a.crouch:
        walk.pad('NumPad6')     # Circle toggles crouch
    while time.time() < t_end:
        dist = math.dist(here[:2], goal[:2])
        if dist < a.reach:
            break
        cell = cell_of(here)
        if not available_directions(cell, {}, lethal):
            print(f'no non-lethal direction from {cell}; stopping without another move', flush=True)
            break
        walk.active(force=False)
        if facing is None:
            # Unknown camera heading: a forward push gives it; if blocked, a back-step does.
            d, p0 = push(600)
            if d <= 0.6:
                walk.pad('Hold:S:600')
                q = walk.position(c, tracked, here) or here
                if math.dist(q[:2], here[:2]) > 0.6:
                    facing = math.atan2(here[1] - q[1], here[0] - q[0])
                    here = q
                else:
                    walk.pad(*(['Hold:D:600'] if a.crouch else ['NumPad2', 'Hold:D:600']))
                    here = walk.position(c, tracked, here) or here
            continue
        cell = cell_of(here)
        visits[cell] = visits.get(cell, 0) + 1
        gdir = math.atan2(goal[1] - here[1], goal[0] - here[0])
        cands = []
        for k in available_directions(cell, blocked, lethal):
            ang = k * 2 * math.pi / DIRS
            b = blocked.get((cell, k), 0)
            nxt = cell_of((here[0] + CELL * math.cos(ang), here[1] + CELL * math.sin(ang)))
            score = abs(walk.wrap(ang - gdir)) + 0.8 * visits.get(nxt, 0) + 0.6 * b
            cands.append((score, k, ang))
        if not cands:
            # Retry wall evidence only. A blind jump/back-step can itself repeat a fatal
            # direction and bypass the lethal set, so let the normal planner choose.
            if not reset_transient_blocks(cell, blocked, lethal):
                print(f'no non-lethal direction from {cell}; stopping', flush=True)
                break
            continue
        _, k, ang = min(cands)
        err = walk.wrap(ang - facing)
        if abs(err) > 0.2:
            ms = int(min(1400, abs(err) / a.rate))
            walk.pad(f'Hold:{"L" if err < 0 else "J"}:{ms}')
            facing = ang
        d, p0 = push(700)
        if d <= 0.6:
            if (cell, k) not in jumped:
                jumped.add((cell, k))
                walk.pad(*(['Hold:W:900'] if a.crouch else ['NumPad2', 'Hold:W:600']))
                q = walk.position(c, tracked, here) or here
                if math.dist(q[:2], here[:2]) > 0.6:
                    facing = math.atan2(q[1] - here[1], q[0] - here[0])
                    here = q
                    trail.append(list(here))
                    continue
            blocked[(cell, k)] = blocked.get((cell, k), 0) + 1
            continue
        if math.dist(p0[:2], here[:2]) > 30:
            # The move killed the Guardian (a fall into a kill volume): never try it again.
            blocked[(cell, k)] = 99
            lethal.add((cell, k))
            if a.lethal:
                json.dump(sorted([list(c) + [kk] for c, kk in lethal]), open(a.lethal, 'w'))
            print(f'jumped {math.dist(p0[:2], here[:2]):.0f} units (respawn?): cell {cell} direction {k} marked lethal; '
                  f'trail restarts at {here}')
            if a.stop_on_death:
                break
            trail = [list(here)]
            continue
        trail.append(list(here))
        steps += 1
        dist = math.dist(here[:2], goal[:2])
        if dist < best - 1:
            best = dist
        if steps % 5 == 0:
            print(f'at ({here[0]:.1f}, {here[1]:.1f}, {here[2]:.1f}) dist {dist:.1f} best {best:.1f} '
                  f'cells {len(visits)} blocked {len(blocked)}')
    dist = math.dist(here[:2], goal[:2])
    route = simplify(trail)
    json.dump(route, open(a.out, 'w'))
    print(('reached' if dist < a.reach else 'stopped'), here, f'dist {dist:.1f}', f'route {len(route)} points -> {a.out}')


if __name__ == '__main__':
    main()
