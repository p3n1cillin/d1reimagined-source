"""Read 01.24 performance dispatch and scene state from an exactly owned client.

No guest writes, debugger pauses or game input. Samples are non-atomic; rechecks
expose changes during a read, but cannot establish an actor's lifetime or visible
rendering. Output can contain private game/runtime data; keep it outside Git.
"""
import argparse
import datetime
import json
import math
import struct
import time
from pathlib import Path

from rpcs3_owned_memory import OwnedMemory

RESOURCE_REGISTRY = 0x1896D50
MANAGER_POINTER = 0x1ACD25C
PERFORMANCE_CLASS = 0x80AA9A07
PERFORMANCE_TYPE = 0x80802B63
START_ROUTINE = 0x42C060
REVIVAL = {0x80CB8D65: "revival", 0x80CB8D5B: "revival_first_person"}
MAX_RECORDS = 32
MAX_SCENES = 1024
MAX_BINDINGS = 64


def u32(read, address):
    return struct.unpack(">I", read(address, 4))[0]


class Resources:
    """The native 0x42478c lookup, including signed/dynamic handles."""
    def __init__(self, read):
        self.read = read
        self.registry = u32(read, RESOURCE_REGISTRY)
        self.table = u32(read, self.registry)
        self.observed = {}

    def resolve(self, handle):
        if handle == 0xFFFFFFFF:
            raise ValueError("null resource handle")
        signed = handle - (1 << 32) if handle & 0x80000000 else handle
        shifted = (signed >> 13) & 0xFFFFFFFF
        index = shifted & ((shifted >> 18) | 0x3FF)
        entry = self.table + index * 64
        metadata = self.read(entry, 0x38)
        base = struct.unpack_from(">I", metadata, 0x10)[0]
        stride, mask = struct.unpack_from(">II", metadata, 0x30)
        if not 0 < stride <= 0x100000:
            raise ValueError("invalid resource stride")
        record = (base + (handle & 0x1FFF) * stride) & 0xFFFFFFFF
        header = self.read(record, 12)
        self.observed.setdefault((entry, record), (metadata, header))
        return (record - (struct.unpack_from(">I", header, 8)[0] & mask)) & 0xFFFFFFFF

    def unchanged(self):
        return (u32(self.read, RESOURCE_REGISTRY) == self.registry
                and u32(self.read, self.registry) == self.table
                and all(self.read(entry, 0x38) == metadata
                        and self.read(record, 12) == header
                        for (entry, record), (metadata, header) in self.observed.items()))


def performance(read, resources, raw):
    fields = struct.unpack(">8I", raw)
    row = {"asset": fields[2], "asset_offset": fields[3],
           "name": REVIVAL.get(fields[2]), "record": raw.hex()}
    if fields[4] != PERFORMANCE_CLASS or fields[7] != PERFORMANCE_TYPE:
        row["error"] = "unsupported performance class/type"
        return row
    definition = resources.resolve(fields[4])
    instance = (resources.resolve(fields[5]) + fields[6]) & 0xFFFFFFFF
    meta = (definition + u32(read, definition + 0x10)) & 0xFFFFFFFF
    slot = u32(read, meta + 0x38)
    dispatch_instance = (instance + u32(read, meta + 0x3C)) & 0xFFFFFFFF
    descriptor = (u32(read, (u32(read, dispatch_instance) + slot - 1) & 0xFFFFFFFF)
                  if slot & 1 else slot)
    target = u32(read, descriptor)
    row.update(definition=definition, instance=instance, meta=meta,
               descriptor=descriptor, target=target)
    if target != START_ROUTINE:
        row["error"] = "unsupported dispatch routine"
        return row
    header = read(instance, 0xD0)
    asset, authored_offset, instance_type = struct.unpack_from(">III", header)
    if asset != fields[2] or instance_type != 0x808028A1:
        row["error"] = "instance/authored definition identity mismatch"
        return row
    # 0x42c060 resolves the instance header's authored base. The manager stores
    # a derived-class offset; use the actual header rather than guessing a delta.
    authored = (resources.resolve(asset) + authored_offset) & 0xFFFFFFFF
    authored_bytes = read(authored, 0xC0)
    if (struct.unpack_from(">I", authored_bytes)[0] != asset
            or struct.unpack_from(">I", authored_bytes, 8)[0] != PERFORMANCE_TYPE):
        row["error"] = "authored performance identity mismatch"
        return row
    row["authored_definition"] = authored
    row["authored_bytes"] = authored_bytes.hex()
    # 0x42b51c reads role/type pairs at definition+0xc0+relative. These
    # describe player replacements, not every embedded cinematic actor.
    binding_count, binding_relative = struct.unpack_from(">II", authored_bytes, 0xAC)
    if binding_count > MAX_BINDINGS:
        raise ValueError("performance binding count exceeds bound")
    binding_address = (authored + 0xC0 + binding_relative) & 0xFFFFFFFF
    binding_bytes = read(binding_address, binding_count * 2)
    row["bindings"] = [{"role": role, "type": kind}
                       for role, kind in struct.iter_unpack(">bB", binding_bytes)]
    row["binding_selection"] = {"mode": struct.unpack_from(">b", authored_bytes, 0x37)[0],
                                "filter": struct.unpack_from(">b", authored_bytes, 0x39)[0],
                                "sort": struct.unpack_from(">b", authored_bytes, 0x3A)[0]}
    cast_relative = struct.unpack_from(">I", header, 0xC0)[0]
    row["cast_source"] = ((instance + 0xC0 + cast_relative) & 0xFFFFFFFF
                          if cast_relative else u32(read, MANAGER_POINTER))
    count, relative = struct.unpack_from(">II", header, 0x34)
    if count > MAX_SCENES:
        raise ValueError("performance scene count exceeds bound")
    phase = struct.unpack_from(">b", header, 0x21)[0]
    current = struct.unpack_from(">i", header, 0x28)[0]
    row.update(state_bytes=header.hex(), phase_raw=phase,
               scene_index=current, scene_count=count, scenes=[])
    # 0x42a60c uses 72-byte slots, addressing fields from instance+relative.
    base = (instance + relative) & 0xFFFFFFFF
    for index in range(count):
        address = base + index * 72
        scene = {"index": index, "address": address, "bytes": read(address, 0x90).hex()}
        scene_handle, scene_offset = struct.unpack(">II", read(address + 0x48, 8))
        scene.update(handle=scene_handle, offset=scene_offset)
        if scene_handle != 0xFFFFFFFF:
            scene_definition = (resources.resolve(scene_handle) + scene_offset) & 0xFFFFFFFF
            scene.update(definition=scene_definition,
                         definition_bytes=read(scene_definition, 0xC0).hex())
        row["scenes"].append(scene)
    row["instance_unchanged"] = (read(instance, 16) == header[:16]
                                 and read(authored, 0xC0) == authored_bytes
                                 and read(binding_address, binding_count * 2) == binding_bytes)
    return row


def snapshot(read, revival_only=False, include_actors=False):
    manager = u32(read, MANAGER_POINTER)
    # The verified guest image exists before the performance manager is created.
    # Report that startup state without reading through a null manager.
    if not manager:
        return {"atomic": False, "initialized": False, "manager": manager,
                "count": None, "performances": [],
                "unchanged_during_read": u32(read, MANAGER_POINTER) == manager}
    count = u32(read, manager + 0x1350)
    if count > MAX_RECORDS:
        raise ValueError("performance count exceeds bound")
    records = read(manager + 0x1354, count * 32)
    resources = Resources(read)
    result = {"atomic": False, "initialized": True, "manager": manager,
              "count": count, "performances": []}
    for index in range(count):
        raw = records[index * 32:(index + 1) * 32]
        if revival_only and struct.unpack_from(">I", raw, 8)[0] not in REVIVAL:
            continue
        try:
            row = performance(read, resources, raw)
        except (OSError, ValueError, struct.error) as error:
            row = {"record": raw.hex(), "error": str(error)}
        result["performances"].append(row)
    result["machine_cast_bytes"] = read(manager + 0x17D4, 104).hex()
    if include_actors:
        # 0x42e678 indexes the six 0x338-byte actor-cache slots at manager+0;
        # their presence does not prove a physical or visible Ghost.
        result["actor_slots"] = [read(manager + index * 0x338, 0x338).hex() for index in range(6)]
    result["unchanged_during_read"] = (
        u32(read, MANAGER_POINTER) == manager
        and u32(read, manager + 0x1350) == count
        and read(manager + 0x1354, count * 32) == records
        and resources.unchanged()
        and all(row.get("instance_unchanged", False)
                for row in result["performances"] if not row.get("error")))
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--seconds", type=float, default=180)
    parser.add_argument("--interval", type=float, default=0.5)
    parser.add_argument("--revival-only", action="store_true")
    parser.add_argument("--include-actors", action="store_true")
    args = parser.parse_args()
    if (not math.isfinite(args.seconds) or not 0 < args.seconds <= 3600
            or not math.isfinite(args.interval) or not 0.1 <= args.interval <= 10):
        parser.error("seconds must be in (0,3600] and interval in [0.1,10]")
    with OwnedMemory(args.manifest) as memory, args.output.open("x", encoding="utf-8") as output:
        deadline = time.monotonic() + args.seconds
        previous = None
        while time.monotonic() < deadline:
            state = snapshot(memory.read, args.revival_only, args.include_actors)
            state["utc"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
            output.write(json.dumps(state) + "\n")
            output.flush()
            summary = [{k: row.get(k) for k in ("asset", "name", "target", "phase_raw",
                       "scene_index", "scene_count", "error")} for row in state["performances"]]
            status = state["initialized"], summary
            if status != previous:
                print(json.dumps({"utc": state["utc"], "initialized": state["initialized"],
                                  "performances": summary}), flush=True)
                previous = status
            time.sleep(args.interval)


if __name__ == "__main__":
    main()
