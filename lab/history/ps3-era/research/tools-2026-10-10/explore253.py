"""Guarded Breach exploration for JOIN253: ordinary pad input toward given x,y points.
Read-only memory; stops on stage change, rejoin, player change/jump, or leaving slice 24."""
import json, math, struct, sys, time
from contextlib import ExitStack
from pathlib import Path
sys.path.insert(0, r"F:\d1\d1xenia\scripts")
from d1_guardian_route import HOOK, HookLease, SharedTail, RouteStopped, GuardedPad, GuardedPlayer, run_state, utc_now
from d1_walker import Grid, Walker, facing_from_pointers
from rpcs3_owned_memory import OwnedMemory, emulator_identity
rt = Path(r"F:\d1\<pc>-import\d1xenia\local")
manifest = rt / "SIGNON_LOCAL/logs/FINAL124_JOIN253-processes.json"
identity = emulator_identity(json.loads(manifest.read_text(encoding="utf-8-sig")))
emulator = Path(identity["executable"]).parent
budget = float(sys.argv[1]); radius = float(sys.argv[2])
points = [((p.split(":")[0] if ":" in p else ""),) + tuple(map(float, p.split(":")[-1].split(","))) for p in sys.argv[3:]]
output = rt / "RESEARCH/JOIN253-explore"; output.mkdir(exist_ok=True)
pad = None
with (output / "audit.jsonl").open("a", buffering=1) as stream:
    def audit(**v): stream.write(json.dumps({"utc": utc_now(), **v}) + "\n")
    result = {"started": utc_now(), "points": points}
    try:
        with OwnedMemory(manifest) as memory, ExitStack() as cleanup:
            cleanup.callback(lambda: pad.release() if pad else None)
            tail = SharedTail(emulator / "log/RPCS3.log", fresh=True)
            cleanup.enter_context(HookLease(emulator / "d1/d1ppu.txt", 100000))
            host = SharedTail(rt / "SIGNON_LOCAL/logs/FINAL124_JOIN253-ahl.log", fresh=True)
            class G:
                audit = staticmethod(audit)
                deadline = time.monotonic() + budget
                entity = seen = None
                def check(self):
                    if time.monotonic() >= self.deadline: raise RouteStopped("budget ended")
                    if any(b"host join request" in l for l in host.lines()): raise RouteStopped("activity rejoin")
                    hits = [int(h[1], 16) for line in tail.lines() if (h := HOOK.search(line))]
                    if hits:
                        if self.entity is not None and hits[-1] != self.entity: raise RouteStopped("player changed")
                        self.entity, self.seen = hits[-1], time.monotonic()
                    if self.entity is None or time.monotonic() - self.seen > 5: raise RouteStopped("no fresh player hook")
                    s = run_state(memory.read)
                    if s["stable"] and not (s["current"] == 36 and s["native_slice_current"][0] in (112, 24)):
                        raise RouteStopped("left mission or slices 112/24: " + str(s))
                    return s
            guard = G()
            for _ in range(50):
                time.sleep(0.2)
                try: guard.check(); break
                except RouteStopped as e:
                    if "fresh" not in str(e): raise
            player = GuardedPlayer(memory, guard.entity, guard)
            result.update(entity=hex(guard.entity), start=player.position(), state=run_state(memory.read))
            facing = facing_from_pointers(memory)
            if facing is None: raise RouteStopped("verified facing unavailable")
            pad = GuardedPad(identity, memory, guard)
            walker = Walker(memory, player, pad, (facing, 1), Grid(output / "grid.json"), lambda m: audit(ev="walker", message=m))
            reached = []
            for f, *p in points:
                if f in ("face", "jump"):
                    here = player.position(); walker.face(math.atan2(p[1] - here[1], p[0] - here[0])); print("faced", p, flush=True)
                    if f == "jump":
                        import threading
                        th = threading.Thread(target=pad.hold, args=(["W"], 1500)); th.start()
                        time.sleep(0.25); pad.hold(["X"], 200); time.sleep(0.4); pad.hold(["X"], 200); th.join()
                        print("jumped ->", [round(v, 2) for v in player.position()], flush=True)
                    continue
                ok = walker.go(tuple(p), radius=radius, budget=max(0, guard.deadline - time.monotonic()))
                reached.append([p, ok, player.position()])
                print("point", p, "ok" if ok else "NOT reached", [round(v, 2) for v in player.position()], flush=True)
                if not ok: break
            result["reached"] = reached
    except (RouteStopped, OSError, ValueError) as exc:
        result["stopped"] = str(exc)
    finally:
        result["finished"] = utc_now()
        try: result["final"] = player.position.__func__(player) if False else None
        except Exception: pass
        print(json.dumps(result), flush=True)
































