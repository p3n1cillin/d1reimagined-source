import datetime
import json
import struct
import sys
from pathlib import Path
sys.path.insert(0, r"F:\d1\d1xenia\scripts")
from rpcs3_owned_memory import OwnedMemory
root = Path(__file__).parent
runtime = root.parent
inputs = {}
rows = []
with OwnedMemory(runtime / "SIGNON_LOCAL/logs/FINAL124_JOIN218-processes.json") as memory:
    def read(address, size):
        raw = memory.read(address, size)
        inputs[(address, size)] = raw
        return raw
    def u32(address):
        return struct.unpack(">I", read(address, 4))[0]
    def valid(address, size):
        return 0 < address <= 0x100000000 - size and address % 4 == 0
    for global_address in (0x18dedc4, 0x18dee3c, 0x18ded3c, 0x18ded40):
        pointer = u32(global_address)
        row = {"global":hex(global_address), "resource":hex(pointer)}
        if not valid(pointer, 0x20):
            raise ValueError("resource pointer invalid")
        raw = read(pointer, 0x20)
        words = struct.unpack(">8I", raw)
        row["resource_words"] = [hex(v) for v in words]
        meta = words[1]
        if not valid(meta, 0x60):
            raise ValueError("metadata pointer invalid")
        raw_meta = read(meta, 0x60)
        row["metadata"] = hex(meta)
        row["metadata_words"] = [hex(v) for v in struct.unpack(">24I", raw_meta)]
        offset = struct.unpack_from(">I", raw_meta, 0x34)[0]
        field_list = meta + 0x34 + offset
        if 0 < offset <= 0x100000 and valid(field_list, 0x14):
            count, handle, size, a, b = struct.unpack(">5I", read(field_list, 0x14))
            if count <= 4096 and 0x80800000 <= handle < 0x80810000 and size <= 0x100000:
                field_bytes = read(field_list + 0x14, count * 0x28)
                row["field_header"] = [count, hex(handle), size, hex(a), hex(b)]
                row["raw_fields"] = field_bytes.hex()
        rows.append(row)
    registrations = []
    for address in range(0x189ad48, 0x189ad9c, 12):
        event_address, schema_address, callback = struct.unpack(">3I", read(address, 12))
        registrations.append({"table":hex(address), "event_global":hex(event_address),
                              "event_hash":hex(u32(event_address)), "schema_global":hex(schema_address),
                              "schema_hash":hex(u32(schema_address)), "callback_descriptor":hex(callback),
                              "callback":hex(u32(callback))})
    constants = {hex(a):hex(u32(a)) for a in (0x1a20758, 0x1a207b8, 0x1a207fc, 0x1a20710)}
    if any(memory.read(address, size) != data for (address,size),data in inputs.items()):
        raise ValueError("resource inputs changed; preserve no mixed sample")
result = {"utc":datetime.datetime.now(datetime.timezone.utc).isoformat(), "atomic":False,
          "scope":"Read-only native incident resources, corrected signed global addresses; no tag-handle dereference or incident injection.",
          "resources":rows, "registrations":registrations, "constants":constants}
destination = root / "JOIN218-incident-resources.json"
with destination.open("x",encoding="utf-8") as output:
    json.dump(result,output,indent=2)
    output.write("\n")
print(json.dumps({"resources":[{k:v for k,v in r.items() if k not in ('raw_fields','metadata_words','resource_words')} for r in rows],
                  "registrations":registrations,"constants":constants}))
