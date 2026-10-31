"""Private 30-second pad experiment after the naturally crossed doorway."""
import json, math, struct, sys, time
from contextlib import ExitStack
from pathlib import Path
sys.path.insert(0, r"F:\d1\d1xenia\scripts")
from d1_guardian_route import HOOK, HookLease, SharedTail, RouteStopped, GuardedPad, GuardedPlayer, run_state, utc_now, BREACH_ROUTE, breach_continuation_points
from d1_walker import Grid, Walker, facing_from_pointers
from rpcs3_owned_memory import OwnedMemory, emulator_identity
rt = Path(r"F:\d1\<pc>-import\d1xenia\local")
manifest = rt / "SIGNON_LOCAL/logs/FINAL124_JOIN180-processes.json"
identity = emulator_identity(json.loads(manifest.read_text(encoding="utf-8-sig")))
emulator = Path(identity["executable"]).parent
output = rt / "RESEARCH/JOIN180-natural-player-continuation-2"
output.mkdir(exist_ok=False)
result = {"started_utc": utc_now(), "scope": "Ordinary pad input on the fresh naturally crossed actor; 30-second bounded corridor; no guest writes or manual host teleport."}
pad = None
with (output / "audit.jsonl").open("w", buffering=1) as stream:
    def audit(**values): stream.write(json.dumps({"utc": utc_now(), **values}) + "\n")
    try:
        with OwnedMemory(manifest) as memory, ExitStack() as cleanup:
            cleanup.callback(lambda: pad.release() if pad else None)
            tail = SharedTail(emulator / "log/RPCS3.log", fresh=True)
            cleanup.enter_context(HookLease(emulator / "d1/d1ppu.txt", 5000))
            class CorridorGuard:
                def __init__(self):
                    self.audit = audit
                    self.host = SharedTail(rt / "SIGNON_LOCAL/logs/FINAL124_JOIN180-ahl.log", fresh=True)
                    self.deadline = time.monotonic() + 30
                    self.seen = self.entity = None
                def check(self):
                    if time.monotonic() >= self.deadline: raise RouteStopped("30-second corridor budget ended")
                    if any(b"host join request" in line for line in self.host.lines()): raise RouteStopped("activity rejoin")
                    hits = [hit for line in tail.lines() if (hit := HOOK.search(line))]
                    if hits:
                        entity = int(hits[-1][1], 16)
                        if self.entity is not None and entity != self.entity: raise RouteStopped("player changed")
                        self.entity, self.seen = entity, time.monotonic()
                    if self.entity is None or time.monotonic() - self.seen > 3: raise RouteStopped("no fresh player hook")
                    state = run_state(memory.read)
                    if not (state["stable"] and state["current"] == 36 and state["engine"] == 3 and
                            state["native_slice_current"] == (24, 0x68935472) and
                            state["native_slice_target"] == (112, 0x811c9dc5)):
                        raise RouteStopped("native phase or slice changed: " + str(state))
                    xyz = struct.unpack(">fff", memory.read(self.entity + 0x40, 12))
                    if not (all(math.isfinite(v) for v in xyz) and 350 <= xyz[0] <= 400 and
                            -810 <= xyz[1] <= -790 and 10 <= xyz[2] <= 20): raise RouteStopped("player outside bounded corridor")
            guard = CorridorGuard()
            observation_deadline = time.monotonic() + 5
            while True:
                try:
                    guard.check()
                    break
                except RouteStopped as exc:
                    if str(exc) != "no fresh player hook" or time.monotonic() >= observation_deadline:
                        raise
                    time.sleep(0.1)
            player = GuardedPlayer(memory, guard.entity, guard)
            start = player.position()
            if math.dist(start, (354.614, -796.247, 14.095)) > 2: raise RouteStopped("fresh actor away from inspected start")
            facing = facing_from_pointers(memory)
            if facing is None: raise RouteStopped("verified facing unavailable")
            pad = GuardedPad(identity, memory, guard)
            result.update(entity=hex(guard.entity), start_xyz=start)
            walker = Walker(memory, player, pad, (facing, 1), Grid(output / "grid.json"), lambda msg: audit(ev="walker", message=msg))
            for index, point in breach_continuation_points((353.751, -796.292, 14.008)):
                guard.check()
                audit(ev="waypoint", route=str(BREACH_ROUTE), route_index=index, xyz=point)
                if not walker.go(point[:2], radius=1.5, budget=max(0, guard.deadline-time.monotonic())):
                    raise RouteStopped("recorded corridor waypoint not reached")
            result["recorded_corridor_reached"] = True
            result["final_xyz"] = player.position()
    except (RouteStopped, OSError, ValueError) as exc:
        result["stopped"] = str(exc)
    finally:
        result["finished_utc"] = utc_now()
        (output / "result.json").write_text(json.dumps(result, indent=2) + "\n")
        print(json.dumps(result), flush=True)
