"""Private bounded read-only player evidence; no pad inputs or guest writes."""
import datetime
import json
import math
import struct
import sys
import time
from pathlib import Path

sys.path.insert(0, r"F:\d1\d1xenia\scripts")
from d1_guardian_route import HOOK, HookLease, SharedTail, run_state
from rpcs3_owned_memory import OwnedMemory, emulator_identity

manifest = Path(sys.argv[1])
output = Path(sys.argv[2])
seconds = float(sys.argv[3])
if not math.isfinite(seconds) or not 0 < seconds <= 240:
    raise ValueError("bounded observation requires 0 < seconds <= 240")
identity = emulator_identity(json.loads(manifest.read_text(encoding="utf-8-sig")))
emulator = Path(identity["executable"]).parent
output.mkdir(exist_ok=False)
count = fresh_count = 0
error = None
started = datetime.datetime.now(datetime.timezone.utc).isoformat()
try:
    with OwnedMemory(manifest) as memory, (output / "samples.jsonl").open("w", buffering=1) as stream:
        tail = SharedTail(emulator / "log/RPCS3.log", fresh=True)
        lease = HookLease(emulator / "d1/d1ppu.txt", math.ceil(seconds * 120) + 512)
        # Static 01.24 caller uses r5 for the chosen target and r6 for its named-set hash.
        # The bounded watch records real native calls; it does not alter return values.
        lease.installed += "352138 32 if=lr==0x352f8c r3+0x36c:48 r6:4\n"
        with lease:
            deadline = time.monotonic() + seconds
            entity = seen = None
            while time.monotonic() < deadline:
                hits = [hit for line in tail.lines() if (hit := HOOK.search(line))]
                if hits:
                    entity = int(hits[-1][1], 16)
                    seen = time.monotonic()
                age = None if seen is None else time.monotonic() - seen
                row = {"utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
                       "atomic": False, "state": run_state(memory.read),
                       "entity": None if entity is None else hex(entity), "hook_age": age}
                engine = struct.unpack(">I", memory.read(0x01ABC8AC, 4))[0]
                if engine:
                    raw = memory.read(engine + 0x36c, 48)
                    row["native_choice_36c_39b"] = raw.hex()
                    row["choice_unchanged_during_read"] = raw == memory.read(engine + 0x36c, 48)
                if entity is not None and age <= 3 and entity + 0x4c <= 0x100000000:
                    before = memory.read(entity + 0x40, 12)
                    row["xyz"] = struct.unpack(">fff", before)
                    row["position_unchanged_during_read"] = before == memory.read(entity + 0x40, 12)
                    fresh_count += 1
                stream.write(json.dumps(row) + "\n")
                count += 1
                time.sleep(0.25)
except Exception as exc:
    error = repr(exc)
finally:
    result = {"started_utc": started,
              "finished_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "identity": identity, "samples": count, "fresh_player_samples": fresh_count,
              "error": error, "scope": "Read-only, non-atomic observation; fresh hook required for every reported position."}
    (output / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result), flush=True)
if error:
    raise SystemExit(1)
