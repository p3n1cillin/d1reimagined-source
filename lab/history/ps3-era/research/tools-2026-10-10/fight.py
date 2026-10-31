"""Guarded aimed fire for a JOINnnn run: turn toward the nearest moving non-player entity
(entity pointers from 0x138e24 hook lines, position at entity +0x40) and fire R1 (key E).
Read-only memory; ordinary pad input only. Re-acquires the player after a death/respawn.
usage: fight.py RUN SECONDS"""
import json, math, re, struct, sys, time
from contextlib import ExitStack
from pathlib import Path
sys.path.insert(0, r"F:\d1\d1xenia\scripts")
from d1_guardian_route import HOOK, HookLease, SharedTail, RouteStopped, GuardedPad, GuardedPlayer, run_state, utc_now
from d1_walker import Grid, Walker, facing_from_pointers
from rpcs3_owned_memory import OwnedMemory, emulator_identity
GuardedPad.VK = {**GuardedPad.VK, "E": 0x45}  # R1 (fire) in this layout

ENT = re.compile(rb"pc=0x138e24\s+lr=0x([0-9a-f]+)\b.*?\br4=0x([0-9a-f]+)\b", re.I)
run, budget = sys.argv[1], float(sys.argv[2])
rt = Path(r"F:\d1\<pc>-import\d1xenia\local")
manifest = rt / f"SIGNON_LOCAL/logs/FINAL124_{run}-processes.json"
identity = emulator_identity(json.loads(manifest.read_text(encoding="utf-8-sig")))
emulator = Path(identity["executable"]).parent
output = rt / f"RESEARCH/{run}-fight"; output.mkdir(exist_ok=True)
deadline = time.monotonic() + budget
seen = {}       # entity -> last hook time
track = {}      # entity -> (time, position) history
engaged = {}    # entity -> first/last engagement time
lives, shots = 0, 0
former_players = set()
origin = {}  # entity -> first position seen


def position(memory, entity):
    try:
        p = struct.unpack(">fff", memory.read(entity + 0x40, 12))
    except OSError:
        return None
    return p if all(math.isfinite(v) and abs(v) < 1e5 for v in p) else None


with (output / "audit.jsonl").open("a", buffering=1) as stream:
    def audit(**v): stream.write(json.dumps({"utc": utc_now(), **v}) + "\n")
    with OwnedMemory(manifest) as memory, ExitStack() as cleanup:
        tail = SharedTail(emulator / "log/RPCS3.log", fresh=True)
        cleanup.enter_context(HookLease(emulator / "d1/d1ppu.txt", 1000000))
        while time.monotonic() < deadline:
            pad = None

            class G:
                audit = staticmethod(audit)
                deadline = globals()["deadline"]
                entity = last = None

                def check(self):
                    if time.monotonic() >= self.deadline: raise RouteStopped("budget ended")
                    now = time.monotonic()
                    for line in tail.lines():
                        h = HOOK.search(line)
                        if h:
                            e = int(h[1], 16)
                            if self.entity is not None and e != self.entity: raise RouteStopped("player changed")
                            self.entity, self.last = e, now
                            continue
                        m = ENT.search(line)
                        if m: seen[int(m[2], 16)] = now
                    if self.entity is None or now - self.last > 5: raise RouteStopped("no fresh player hook")
                    return run_state(memory.read)

            guard = G()
            try:
                for _ in range(100):
                    time.sleep(0.2)
                    try: guard.check(); break
                    except RouteStopped as e:
                        if "fresh" not in str(e) and "changed" not in str(e): raise
                        guard.entity = None
                player = GuardedPlayer(memory, guard.entity, guard)
                facing = facing_from_pointers(memory)
                if facing is None: raise RouteStopped("verified facing unavailable")
                pad = GuardedPad(identity, memory, guard)
                walker = Walker(memory, player, pad, (facing, 1), Grid(output / "grid.json"), lambda m: audit(ev="walker", message=m))
                lives += 1; former_players.add(guard.entity)
                print(f"life {lives} player {guard.entity:#x} at {[round(v, 1) for v in player.position()]}", flush=True)
                while True:
                    guard.check()
                    me = player.position()
                    now = time.monotonic()
                    best = None; stats = [0, 0, None]
                    for e, t in list(seen.items()):
                        if e in former_players or now - t > 60: continue
                        p = position(memory, e)
                        if p is None: continue
                        hist = track.setdefault(e, [])
                        hist.append((now, p)); del hist[:-15]
                        moved = math.dist(p, origin.setdefault(e, p))
                        d = math.dist(p, me)
                        stats[0] += 1; stats[1] += moved > 0.15; stats[2] = d if stats[2] is None else min(stats[2], d)
                        if moved > 2.0 and 1.0 < d < 30 and abs(p[2] - me[2]) < 8:
                            if best is None or d < best[0]: best = (d, e, p)
                    if now - globals().get('_last_status', 0) > 10:
                        globals()['_last_status'] = now
                        print(f'status: seen {len(seen)} live {stats[0]} moving {stats[1]} nearest {stats[2] and round(stats[2], 1)} me {[round(v, 1) for v in me]}', flush=True)
                    if best is None:
                        time.sleep(0.4); continue
                    d, e, p = best
                    walker.face(math.atan2(p[1] - me[1], p[0] - me[0]))
                    pad.hold(["E"], 700); shots += 1
                    first = engaged.setdefault(e, [now, now, 0]); first[1] = now; first[2] += 1
                    audit(ev="shot", target=f"{e:#x}", distance=round(d, 2), target_xyz=p, me=me)
                    if shots % 5 == 1:
                        print(f"shot {shots} at {e:#x} d={d:.1f} {[round(v, 1) for v in p]}", flush=True)
            except RouteStopped as exc:
                print("stopped:", exc, flush=True)
                if "budget" in str(exc): break
                time.sleep(3)
            finally:
                if pad: pad.release()
now = time.monotonic()
summary = {f"{e:#x}": {"shots": v[2], "last_hook_age_s": round(now - seen.get(e, 0), 1)} for e, v in engaged.items()}
print(json.dumps({"lives": lives, "shots": shots, "engaged": summary}), flush=True)
