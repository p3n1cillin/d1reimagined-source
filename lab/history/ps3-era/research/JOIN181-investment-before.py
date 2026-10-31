"""Encoder for Destiny (BLES01857 01.00) investment objects, driven by the engine's own
runtime type definitions.

The schema is exported from a running local test instance with
`scripts/rpcs3_tiger_types.py schema OUT.json HANDLE...` (offsets, kinds and bit widths only;
it is kept outside Git). Encoding mirrors the client's deserializer (0x25d2dc -> 0xd0ecd8):

  * fields in order, MSB-first bit stream;
  * an optional field is preceded by one presence bit;
  * kind 1 is a nested struct; with array mode 1 it is a fixed-capacity array whose
    slots are the nested type's fields (e.g. ten u64 fields = u64[10]). The count is the
    value at struct offset `width` in the enclosing struct, and only the first `count`
    fields are serialized (0xd0f058). Such a field's value is a list of slot values;
  * primitives (readers at 0x175d5c8) store `value + min` in `width` bits: kind 2 bool
    (1 bit), 3/7 u8, 4/8 u16, 5/9 u32, 6/10 u64, 11 real32 (32 bits), 0x23 machine id (64 raw
    bits: 0xd1d184 reads type 0x80801932 or 0x808004ac, both 8 bytes MSB-first).
  * kind 0x17 is a nullable handle: one presence bit, then u32 when present;
    None/-1/0xffffffff encode null, which decodes to -1;
  * kind 0x22 is a nullable typed value: the same handle prefix, then the selected
    schema's fields. None encodes null; present values use {"type": HANDLE,
    "values": {OFFSET: VALUE}}. Unknown handles are rejected. These readers are
    0xe45198/0xe45604 in 01.24 and 0xd1ccb8/0xd1d124 in 01.00.

`values` maps a struct offset to a value (or, for nested fields, a dict or list of dicts for
that struct) at each level; everything not given is encoded as absent (optional) or zero.

Object and family wire formats (qz update, 0x51c7ec / 0x41b9ec): an object is a 16-byte
header - u32 type hash (the type definition's +4), u64 object soid, u32 payload length -
followed by the payload; a family is u32 type, u64 root soid, u32 version, u8 flags,
u32 object count, objects. 0x41b75c accepts a family only for an existing subscription
slot: a new subscription needs flags bit 0 (full snapshot); later deltas (bit 0 clear)
must carry version = previous + 1, otherwise sign-in reason 6 is set.
"""

import json
import struct
from pathlib import Path

from local_paths import SIGNON  # noqa: E402

DEFAULT_SCHEMA = SIGNON / "investment_schema.json"
MACHINE_ID_KIND = 0x23
NULLABLE_HANDLE_KIND = 0x17
TYPED_VALUE_KIND = 0x22
NULL_HANDLE = 0xFFFFFFFF


class BitWriter:
    def __init__(self):
        self.bits = []

    def put(self, value, width):
        self.bits += [(value >> (width - 1 - i)) & 1 for i in range(width)]

    def data(self):
        bits = self.bits + [0] * (-len(self.bits) % 8)
        return bytes(int("".join(map(str, bits[i:i + 8])), 2) for i in range(0, len(bits), 8))


class BitReader:
    def __init__(self, data):
        self.data = data
        self.position = 0

    def get(self, width):
        if self.position + width > len(self.data) * 8:
            raise ValueError("truncated investment value")
        value = 0
        for _ in range(width):
            value = (value << 1) | ((self.data[self.position >> 3] >> (7 - self.position % 8)) & 1)
            self.position += 1
        return value


def _slots(schema, handle):
    return [f["off"] for f in schema[f"{handle:#x}"]["fields"]]


def _read_handle(reader):
    # 01.24 0xe45198 (01.00 0xd1ccb8): absent is the native -1 handle.
    value = reader.get(32) if reader.get(1) else NULL_HANDLE
    return -1 if value == NULL_HANDLE else value


def _write_handle(writer, value):
    value = -1 if value is None else int(value)
    if value in (-1, NULL_HANDLE):
        writer.put(0, 1)
    elif 0 <= value < NULL_HANDLE:
        writer.put(1, 1)
        writer.put(value, 32)
    else:
        raise ValueError("nullable handle must be null or an unsigned 32-bit handle")


def _typed_handle(schema, value):
    try:
        handle = int(value, 0) if isinstance(value, str) else int(value)
    except (ValueError, TypeError) as exc:
        raise ValueError("typed value needs an integer type handle") from exc
    if not 0 <= handle < NULL_HANDLE or f"{handle:#x}" not in schema:
        raise ValueError(f"typed value has an unknown type handle {handle:#x}")
    return handle


def decode_from(reader, schema, handle, limit=None):
    values = {}
    for field in schema[f"{handle:#x}"]["fields"][:limit]:
        if field["opt"] and not reader.get(1):
            continue
        off = field["off"]
        kind = field["kind"]
        if kind == 1 and field["arr"] == 1:
            count = values.get(field["width"], 0)
            slots = decode_from(reader, schema, field["nested"], count)
            values[off] = [slots.get(o) for o in _slots(schema, field["nested"])[:count]]
        elif kind == 1:
            values[off] = decode_from(reader, schema, field["nested"])
        elif kind == 2:
            values[off] = bool(reader.get(1))
        elif kind == 11:
            values[off] = struct.unpack(">f", struct.pack(">I", reader.get(32)))[0]
        elif kind == MACHINE_ID_KIND:
            values[off] = reader.get(64)
        elif kind == NULLABLE_HANDLE_KIND:
            values[off] = _read_handle(reader)
        elif kind == TYPED_VALUE_KIND:
            # 0xe45604 reads a nullable type handle, then that type's fields.
            nested = _read_handle(reader)
            values[off] = (None if nested == -1 else
                           {"type": _typed_handle(schema, nested),
                            "values": decode_from(reader, schema, nested)})
        else:
            values[off] = reader.get(field["width"]) - field["min"]
    return values


def decode(schema, handle, data):
    reader = BitReader(data)
    return decode_from(reader, schema, handle)


def load_schema(path=DEFAULT_SCHEMA):
    return json.loads(Path(path).read_text(encoding="utf-8"))


def _primitive(w, field, value):
    kind = field["kind"]
    if kind == 2:
        w.put(1 if value else 0, 1)
        return
    if kind == MACHINE_ID_KIND:
        w.put(int(value or 0), 64)
        return
    if kind == NULLABLE_HANDLE_KIND:
        _write_handle(w, value)
        return
    width = 32 if kind == 11 else field["width"]
    if kind == 11:
        value = struct.unpack(">I", struct.pack(">f", float(value or 0)))[0]
        w.put(value, 32)
        return
    w.put((int(value or 0) + field["min"]) & ((1 << width) - 1), width)


def encode_into(w, schema, handle, values=None, limit=None):
    values = values or {}
    for field in schema[f"{handle:#x}"]["fields"][:limit]:
        off = field["off"]
        present = off in values
        if field["opt"]:
            w.put(1 if present else 0, 1)
            if not present:
                continue
        value = values.get(off)
        if field["kind"] == TYPED_VALUE_KIND:
            if value is None or value in (-1, NULL_HANDLE):
                _write_handle(w, None)
            else:
                if not isinstance(value, dict) or set(value) != {"type", "values"}:
                    raise ValueError("typed value needs a type handle and a values mapping")
                nested = _typed_handle(schema, value["type"])
                if not isinstance(value["values"], dict):
                    raise ValueError("typed value fields must be a mapping")
                _write_handle(w, nested)
                encode_into(w, schema, nested, value["values"])
        elif field["kind"] != 1:
            _primitive(w, field, value)
        elif field["arr"] == 1:
            items = list(value or [])
            if len(items) != int(values.get(field["width"], 0)):
                raise ValueError(f"array at {off:#x} needs its count at {field['width']:#x}")
            slots = _slots(schema, field["nested"])
            if len(items) > len(slots):
                raise ValueError(f"array at {off:#x} holds at most {len(slots)} items")
            encode_into(w, schema, field["nested"], dict(zip(slots, items)), len(items))
        else:
            encode_into(w, schema, field["nested"], value)


def encode(schema, handle, values=None):
    w = BitWriter()
    encode_into(w, schema, handle, values)
    return w.data()


def obj(type_hash, soid, payload):
    return struct.pack(">IQI", type_hash, soid, len(payload)) + payload


def family(family_type, root, objects, version=1, flags=0):
    out = struct.pack(">IQIBI", family_type, root, version, flags, len(objects))
    return out + b"".join(objects)


def qz_payload(families):
    return struct.pack(">I", len(families)) + b"".join(families)
