"""Bounded ordinary-pad experiment on the freshly observed post-teleport player."""
import json, math, sys, time
from contextlib import ExitStack
from pathlib import Path
sys.path.insert(0, r"F:\d1\d1xenia\scripts")
from d1_guardian_route import HOOK, HookLease, SharedTail, RouteStopped, GuardedPad, GuardedPlayer, run_state, utc_now
from d1_walker import Grid, Walker, facing_from_pointers
from rpcs3_owned_memory import OwnedMemory, emulator_identity
rt = Path(r"F:\d1\<pc>-import\d1xenia\local")
manifest = rt / "SIGNON_LOCAL/logs/FINAL124_JOIN179-processes.json"
identity = emulator_identity(json.loads(manifest.read_text(encoding="utf-8-sig")))
output = rt / "RESEARCH/JOIN179-doorway-crossing"
output.mkdir(exist_ok=False)
result = {"started_utc": utc_now(), "scope": "Ordinary pad input on fresh post-teleport actor, bounded corridor and 25 seconds; no guest writes."}
pad = None
with (output / "audit.jsonl").open("w", buffering=1) as stream:
    def audit(**values): stream.write(json.dumps({"utc": utc_now(), **values}) + "\n")
    try:
        with OwnedMemory(manifest) as memory, ExitStack() as cleanup:
            cleanup.callback(lambda: pad.release() if pad else None)
            class CorridorGuard:
                def __init__(self):
                    self.audit = audit
                    self.tail = SharedTail(Path(identity["executable"]).parent / "log/RPCS3.log", fresh=True)
                    self.host = SharedTail(rt / "SIGNON_LOCAL/logs/FINAL124_JOIN179-ahl.log", fresh=True)
                    self.deadline = time.monotonic() + 25
                    self.seen = None
                    self.entity = None
                def check(self):
                    if time.monotonic() >= self.deadline: raise RouteStopped("25-second corridor budget ended")
                    if any(b"host join request" in line for line in self.host.lines()): raise RouteStopped("activity rejoin")
                    hits = [hit for line in self.tail.lines() if (hit := HOOK.search(line))]
                    if hits:
                        entity = int(hits[-1][1], 16)
                        if self.entity is not None and entity != self.entity: raise RouteStopped("player changed")
                        self.entity, self.seen = entity, time.monotonic()
                    if self.entity is None or time.monotonic() - self.seen > 3: raise RouteStopped("no fresh player hook")
                    state = run_state(memory.read)
                    if not (state["stable"] and state["current"] == 36 and state["engine"] == 3 and
                            state["native_slice_current"] == (24, 0x68935472) and
                            state["native_slice_target"] == (112, 0x68935472)):
                        raise RouteStopped("native phase or slice changed: " + str(state))
                    import struct
                    xyz = struct.unpack(">fff", memory.read(self.entity + 0x40, 12))
                    if not (all(math.isfinite(v) for v in xyz) and 320 <= xyz[0] <= 360 and
                            -800 <= xyz[1] <= -790 and 10 <= xyz[2] <= 18): raise RouteStopped("player outside bounded corridor")
            guard = CorridorGuard()
            cleanup.enter_context(HookLease(Path(identity["executable"]).parent / "d1/d1ppu.txt", 4096))
            # A new tail must observe the actual callback before accepting the actor.
            time.sleep(0.3)
            guard.check()
            player = GuardedPlayer(memory, guard.entity, guard)
            start = player.position()
            if math.dist(start, (353.883, -796.364, 14.074)) > 2: raise RouteStopped("fresh actor away from inspected start")
            facing = facing_from_pointers(memory)
            if facing is None: raise RouteStopped("verified facing unavailable")
            pad = GuardedPad(identity, memory, guard)
            result.update(entity=hex(guard.entity), start_xyz=start)
            walker = Walker(memory, player, pad, (facing, 1), Grid(output / "grid.json"), lambda msg: audit(ev="walker", message=msg))
            result["endpoint_reached"] = walker.go((356.0, -796.29150390625), radius=0.25, budget=24)
            result["final_xyz"] = player.position()
    except (RouteStopped, OSError, ValueError) as exc:
        result["stopped"] = str(exc)
    finally:
        if pad: pad.release()
        result["finished_utc"] = utc_now()
        (output / "result.json").write_text(json.dumps(result, indent=2) + "\n")
        print(json.dumps(result), flush=True)
