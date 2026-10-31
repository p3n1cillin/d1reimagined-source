"""Loopback Destiny BAP (Bungie session server) stand-in: answers the session hello.

After content registration the client connects to the address from the SignOn reply
(127.1.1.127:12850) and sends `shello req`. Framing, from BLES01857 01.00:

  outer   u8 1 (version), u8 kind, u32be length of the rest            (0x7c413c)
          kind 2 = plaintext; kind 1 = encrypted with the shello session key (0x7c3fbc)
  header  kind 1 (client->server): u16be type, u32be id                (size 6, 0x81684)
          kind 2 (server->client): u16be type, u32be id, u16be status  (size 8, 0x82e30)
          The message codec's method 1 picks the header kind (0x41501c table).
  body    message payload.

Message names come from 0x7c6ef4: 0x19 "shello req", 0x1a "shello rsp", 0x12/0x13 client
config request/response, and so on. shello req (descriptor 0x11f3e00) is protobuf
{1 bytes SignOn token, 2 int32 mode, 3 optional uint64, 4 optional bool} (0x7c648c).

shello rsp (codec 0x41676c) is u32be N (N <= 0x5c) followed by N bytes, sealed as in
0x7c7794 and opened by 0x7c7660:
  u32be N | IV[16] | AES-128-CBC(key, IV, plaintext + PKCS#7) | HMAC-SHA256(mac_key, u32|IV|ct)
The AES key is SignOn session key 2 and the MAC key session key 3 (0x7c75f8). The
plaintext is nonce[12] || session_key[16] (0xb0cd30); the reply status must be 200.

Kind 1 frames (after shello) are AES-128-GCM with that session key, no AAD, 16-byte tag
first: tag[16] || ciphertext(inner header || payload) (0xb0ce4c / 0xb0cf58, 0x7c743c).
Each direction has its own 12-byte nonce: the client sends with the reply nonce's last byte
XOR 1 and receives with the nonce unchanged (0xb0cd30 with direction 0); after each message
the nonce is incremented as a little-endian counter from byte 0.

Encrypted requests seen so far: 0x79 "qz_reg req" (empty payload), then 0x12e "rrc req"
(40 bytes of address data: LAN IPv4, port 3074 little-endian, masks, the STUN-reported
address and a u64). Their replies have header kind 2 and decoders that ignore the payload.
Then 0x0a "c->ws req": a web-service call (codec 0xb4e208/0xb4e444) encoded with the
engine's bit-packed type serializer: 16-bit service id (503 = 0x01f7 in the first call),
the service's request structure, then a flag bit for an optional blob. The reply 0x0b
uses the same layout with the service's response type (descriptor +0x10). Without the
type definitions, --ws-zero-bytes answers with the service id followed by zero bytes,
which deserializes as an all-zero response structure and no blob.
"""

import argparse
from mission_script import ReactionQueue, ReactionRule, TeleportReaction, reaction_scope
from incident_codec import INCIDENT_HANDLE, decode_incident, validate_schema as validate_incident_schema
import datetime
import hashlib
import hmac
import ipaddress
import itertools
import json
import math
import re
import secrets
import socket
import struct
import sys
import threading
import time
from pathlib import Path

from cryptography.exceptions import InvalidTag
from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes
from cryptography.hazmat.primitives.ciphers.aead import AESGCM

sys.path.insert(0, str(Path(__file__).resolve().parent))
import investment
from player_db import PlayerDB  # noqa: E402
from local_paths import SIGNON  # noqa: E402
import signon_proto  # noqa: E402

# Root object types per family (object pools at mgr+0x2a3c0 in 0x41a0d0's manager):
# family -> (type hash, type handle). The self (4) root object is what the sign-in step
# looks up (0x293108); select_character (3) type 0 is its roster header.
ROOT_OBJECTS = {4: (0x88190FE1, 0x80801803, lambda soid: {0: {0: soid}}),
                3: (0xA2AC2C74, 0x80801951, lambda soid: {0: soid})}
SELECT_CHARACTER_OBJECT = (0xF65ABD52, 0x80801954)
SELF_CHARACTER_OBJECT = (0x242B2F61, 0x80801743)
# Self-family kind 3 holds individual item instances, separately from the
# character's inventory summary and equipped IDs. Confirmed in 01.24's
# 0x482aa0 -> 0x47e3cc lookup and live family-4 pool descriptors.
SELF_ITEM_TYPE = 0x80801732
SELF_ITEM_HASHES = {168: 0x0EB6AC38}
# Peer family (0): up to 20 peers, kind 0 = account summary keyed by the account soid (+8 = its
# character soid), kind 1 = character summary (id, class, appearance like the self character).
# The player manager's character cache (0x2998e0) builds a player's appearance and blueprint
# (player record +0x8b0 / +0x1428, needed to spawn) from these two objects.
PEER_ACCOUNT_OBJECT = (0x1D7BB8F6, 0x80801921)
PEER_CHARACTER_OBJECT = (0xA0A1BC80, 0x808017FD)
# The object type hash in each qz object header is a layout fingerprint of the type: the title
# updates that grow a type change its hash, and the client rejects an object with an unknown hash
# (01.24 FINAL124_START2/NOPUSH: the 01.00 hashes for the grown root and character types end the
# session with "pigeon"). Keyed by (handle, the schema's size of that type) -> hash. 01.24 values
# from the EBOOT's type descriptors ({..., 0xf4, hash, 0, wrapper handle, size}; the method returns
# the unchanged 0xA2AC2C74 and 0x1D7BB8F6 for the two types whose size did not change).
OBJECT_HASHES = {
    (0x80801803, 8184): 0x88190FE1, (0x80801803, 28752): 0xB5DF2907,   # self account root
    (0x80801951, 160): 0xA2AC2C74,                                     # account root (family 3)
    (0x80801954, 2024): 0xF65ABD52, (0x80801954, 2368): 0x7D473C17,    # select character
    (0x80801743, 21936): 0x242B2F61, (0x80801743, 28088): 0xEB4B8439,  # self character
    (0x80801921, 80): 0x1D7BB8F6,                                      # peer account
    (0x808017FD, 1968): 0xA0A1BC80, (0x808017FD, 2312): 0x30D274D2,    # peer character
}


def object_hash(schema, default, handle):
    """The qz object type hash for `handle` in the schema's build (`default`, the 01.00 value,
    without a schema or for an unknown size)."""
    size = ((schema or {}).get(f"{handle:#x}") or {}).get("size")
    return OBJECT_HASHES.get((handle, size), default)


def default_ws_response(schema, wire, values=None):
    """Service reply with its response type encoded by default (result error_none, other
    fields absent or zero) and no blob; None if the service is not in the schema."""
    entry = (schema or {}).get("services", {}).get(str(wire))
    if not entry:
        return None
    w = investment.BitWriter()
    w.put(wire, 16)
    investment.encode_into(w, schema, int(entry[1], 16), values)
    w.put(0, 1)  # no blob
    return w.data()


def with_service_map(schema, source):
    """Add service IDs from a package-derived map to an EBOOT-only type export."""
    if schema is None or schema.get("services"):
        return schema
    services = (source or {}).get("services")
    if not services:
        raise ValueError("schema has no service IDs; provide a saved service schema")
    missing = [(wire, handle) for wire, pair in services.items() for handle in pair
               if handle not in schema]
    if missing:
        raise ValueError(f"service mapping references absent type {missing[0][1]} (wire {missing[0][0]})")
    return {**schema, "services": services}


def character_from_request(schema, character_id, request_payload):
    """Profile of a created character: its id plus the class and appearance structs of the
    service-501 request (request offsets 0 and 4 -> character offsets 8 and 0xc)."""
    profile = {0: character_id}
    request_handle = int(schema["services"]["501"][0], 16)
    if f"{request_handle:#x}" in schema:
        request = investment.decode(schema, request_handle, request_payload)
        profile.update({8: request[0], 12: request[4]})
    return profile


# Equipment. The guardian's look (0x6706b0, called from 0x670e40 for player record +0x8b0) is
# built from the 20 equipment slots of the character's progression struct 0x80801855 (peer
# summary +0x30, select-character summary +0x68): +0x138 = 20 x 0x8080183c {u64 item instance
# id, i16 item index, i8, i16, +0xe six dyes 0x80801704 {i8 channel, i16 dye}}. The item index
# is a position in the investment globals item list (0x28a304: root tag +0xd8 -> 8-byte
# entries {u32, u32 item definition tag}); -1 is no item. Which slot holds which bucket comes
# from the investment globals too (root tag +0xc8 -> byte table read by 0x28a1dc: +6 one
# item, +7..+0xb the five armor slots, +0xc..+0xe weapons, +0xf..+0x11 three more).
# The self character (0x80801714) keeps the owned items: +0x30 inventory {i32, 256 x
# 0x808016e0 {i16 item index, u64 instance id, i32, i32}} and +0x1838 the 20 equipped instance
# ids. Without any of this every slot is empty and no gear loads.
GEAR_SLOTS = 20
# The equipment slots follow the 14 equippable buckets of the investment globals (root +0xb0;
# `scripts/d1_pkg.py items --buckets`): 0 subclass, 1 helmet, 2 gauntlets, 3 chest, 4 legs,
# 5 class armour, 6-8 primary/special/heavy, 9 ship, 10 vehicle, 11 ghost, 12 emblem, 13 shader.
# That order matches the appearance builder's use (five armour, three weapons, three more).
# Starter kits: the "wire armor" a new guardian wakes up in ("Your Ghost built this wire
# armor from available mass"), its class subclass and the Generalist Shell ghost; item
# indexes from `d1_pkg.py items` on the BLES01857 disc (each armour piece exists twice, a
# 118-byte and a 152-byte definition; these are the 152-byte ones). No weapon: the first
# mission hands out the rifle.
STARTER_GEAR = {
    "titan": {0: 776, 1: 200, 2: 194, 3: 196, 4: 202, 5: 198, 11: 892},
    "hunter": {0: 774, 1: 12, 2: 6, 3: 8, 4: 14, 5: 10, 11: 892},
    "warlock": {0: 778, 1: 388, 2: 382, 3: 384, 4: 390, 5: 386, 11: 892},
}
# Confirmed against the 01.24 investment library references and bucket hashes.
# These indexes are build-specific; keep the existing 01.00 shorthand intact.
STARTER_GEAR_0124 = {
    "titan": {0: 710, 1: 204, 2: 198, 3: 200, 4: 206, 5: 202, 11: 862},
    "hunter": {0: 709, 1: 16, 2: 10, 3: 12, 4: 18, 5: 14, 11: 862},
    "warlock": {0: 713, 1: 392, 2: 386, 3: 388, 4: 394, 5: 390, 11: 862},
}
# Progression equipment +0xc is an arrangement index, separate from the
# investment item index at +8. In 01.24, 0x46ed4c resolves this through
# 0x2a6474 before populating the normalized character's model references.
# An absent field becomes -1 and leaves equipped armour without a world model.
# These wildcard-class defaults come from each starter item's 0x8080030a
# array in the final client asset library (root +0x144), not the item list.
STARTER_ARRANGEMENTS_0124 = {
    16: 8, 10: 5, 12: 6, 18: 9, 14: 7,             # hunter
    204: 191, 198: 188, 200: 189, 206: 192, 202: 190, # titan
    392: 374, 386: 371, 388: 372, 394: 375, 390: 373, # warlock
}
# 01.24 investment globals: equipped-slot table (root +0xdc) gives each
# slot's bucket; bucket ranges (root +0x84) give its fixed inventory start.
# Inventory +0 is a mutation serial, not the length of a packed item list.
INVENTORY_SLOTS_0124 = (40, 43, 53, 63, 73, 83, 93, 103, 113,
                        123, 143, 153, 210, 220, 230, 240, 268)
PROGRESSION_ITEMS = 0x138
PEER_PROGRESSION, SELECT_PROGRESSION = 0x30, 0x68
SELF_INVENTORY, SELF_EQUIPPED = 0x30, 0x1838
# These offsets are those of 01.00. The title updates keep the type handles but grow the types
# (01.24: slot 0x28 -> 0x30 bytes with dyes at +0x18, progression items at +0x1e8, equipped ids
# at +0x2900; notes/final-build.md), so character_layout() finds each field in the schema by the
# handle of its nested type and keeps the 01.00 value for anything the schema lacks.
LAYOUT_0100 = {"progression_items": PROGRESSION_ITEMS, "peer_progression": PEER_PROGRESSION,
               "select_progression": SELECT_PROGRESSION, "self_inventory": SELF_INVENTORY,
               "self_equipped": SELF_EQUIPPED, "slot_size": 0x28, "slot_dyes": 0xe,
               "inventory_items": 8, "inventory_entry": 0x18,
               "inventory_slots": None}


def field_offset(schema, owner, nested=None, after=None):
    """Offset of the field of type `owner` whose nested type is `nested` (or of the field that
    follows the one nesting `after`); None if the schema does not have it."""
    fields = ((schema or {}).get(owner) or {}).get("fields", [])
    for i, f in enumerate(fields):
        if nested is not None and f.get("nested") == int(nested, 16):
            return f["off"]
        if after is not None and f.get("nested") == int(after, 16) and i + 1 < len(fields):
            return fields[i + 1]["off"]
    return None


def character_layout(schema=None):
    """The character-object offsets gear_objects() writes, for the build `schema` describes."""
    def size(handle):
        return ((schema or {}).get(handle) or {}).get("size")

    # the inventory {i32 count, items} follows the appearance; its type handle differs by build
    self_fields = ((schema or {}).get("0x80801714") or {}).get("fields", [])
    self_inventory = field_offset(schema, "0x80801714", after="0x8080181a")
    inventory = next(((schema or {}).get(f"{f['nested']:#x}") for f in self_fields if f["off"] == self_inventory), None)
    inventory_fields = (inventory or {}).get("fields", [])
    found = {"progression_items": field_offset(schema, "0x80801855", "0x808045d9"),
             "peer_progression": field_offset(schema, "0x808017fd", "0x80801855"),
             "select_progression": field_offset(schema, "0x80801954", "0x80801855"),
             "self_inventory": self_inventory,
             "self_equipped": field_offset(schema, "0x80801714", "0x808045da"),
             "slot_size": size("0x8080183c"),
             "slot_dyes": field_offset(schema, "0x8080183c", "0x808046bc"),
             "inventory_items": inventory_fields[1]["off"] if len(inventory_fields) > 1 else None,
             "inventory_entry": size("0x808016e0")}
    layout = {key: LAYOUT_0100[key] if value is None else value for key, value in found.items()}
    layout["inventory_slots"] = (INVENTORY_SLOTS_0124 if
        size("0x80801714") == 0x6db8 and (inventory or {}).get("size") == 0x2808
        and layout["self_equipped"] == 0x2900 and layout["inventory_entry"] == 0x20
        else None)
    return layout


def gear_spec(value):
    """Parse --character-gear: "SLOT=ITEM[/DYECH:DYE...],..." (item indexes of the investment
    globals item list), "starter:CLASS[,SLOT=ITEM...]" (01.00),
    "starter:01.24:CLASS[,SLOT=ITEM...]" (01.24), or
    @FILE with JSON {"SLOT": {"item": N, "id": N, "dyes": [[ch, dye]],
    "arrangement": N}}. The optional arrangement is a build-specific model
    index, not an item index. Instance ids default to 0x100 + slot."""
    if value.startswith("starter:"):
        kit, _, rest = value[len("starter:"):].partition(",")
        kits = STARTER_GEAR
        if kit.startswith("01.24:"):
            kits, kit = STARTER_GEAR_0124, kit[len("01.24:"):]
        if kit not in kits:
            raise ValueError(f"unknown starter kit {kit!r} (one of {', '.join(kits)})")
        base = {slot: {"item": item, "id": 0x100 + slot, "dyes": []} for slot, item in kits[kit].items()}
        gear = {**base, **(gear_spec(rest) if rest else {})}
        if kits is STARTER_GEAR_0124:
            for entry in gear.values():
                arrangement = STARTER_ARRANGEMENTS_0124.get(entry["item"])
                if arrangement is not None:
                    entry.setdefault("arrangement", arrangement)
        return gear
    if value.startswith("@"):
        raw = json.loads(Path(value[1:]).read_text(encoding="utf-8"))
        entries = {int(k, 0): v if isinstance(v, dict) else {"item": v} for k, v in raw.items()}
    else:
        entries = {}
        for part in filter(None, (x.strip() for x in value.split(","))):
            slot, _, rest = part.partition("=")
            item, *dyes = rest.split("/")
            entries[int(slot, 0)] = {"item": int(item, 0),
                                     "dyes": [[int(y, 0) for y in d.split(":")] for d in dyes]}
    gear = {}
    for slot, entry in sorted(entries.items()):
        if not 0 <= slot < GEAR_SLOTS:
            raise ValueError(f"gear slot {slot} outside 0-{GEAR_SLOTS - 1}")
        dyes = [tuple(int(x) for x in d) for d in entry.get("dyes", [])]
        if len(dyes) > 6:
            raise ValueError(f"gear slot {slot}: at most 6 dyes")
        gear[slot] = {"item": int(entry["item"]), "id": int(entry.get("id", 0x100 + slot)),
                      "dyes": dyes}
        if "arrangement" in entry:
            arrangement = entry["arrangement"]
            if (type(arrangement) is not int or not -1 <= arrangement <= 0x7fff):
                raise ValueError(f"gear slot {slot}: arrangement must be an integer from -1 to 32767")
            gear[slot]["arrangement"] = arrangement
    return gear


def gear_objects(gear, layout=None):
    """Field values that equip `gear` (optional model arrangement per slot) on the three character
    objects: {"peer": ..., "select": ..., "self": ...}, for deep_merge into their values.
    `layout` is character_layout() of the target build (default 01.00)."""
    L = layout or LAYOUT_0100
    slots = {}
    for slot, g in gear.items():
        item = {0: g["id"], 8: g["item"]}
        if "arrangement" in g:
            item[0xc] = g["arrangement"]
        if g["dyes"]:
            item[L["slot_dyes"]] = {i * 4: {0: ch, 2: dye} for i, (ch, dye) in enumerate(g["dyes"])}
        slots[slot * L["slot_size"]] = item
    progression = {L["progression_items"]: slots}
    positions = L.get("inventory_slots")
    if positions and any(slot >= len(positions) for slot in gear):
        raise ValueError("gear slot has no verified inventory bucket in this build")
    inventory = {(positions[slot] if positions else i) * L["inventory_entry"]:
                 {0: g["item"], 8: g["id"], 0x10: 1}
                 for i, (slot, g) in enumerate(gear.items())}
    return {"peer": {L["peer_progression"]: progression},
            "select": {L["select_progression"]: progression},
            "self": {L["self_inventory"]: {0: len(gear), L["inventory_items"]: inventory},
                     L["self_equipped"]: {slot * 8: g["id"] for slot, g in gear.items()}}}


def deep_merge(base, extra):
    """Copy of dict `base` with `extra` merged in (nested dicts merged, other values replaced)."""
    out = dict(base)
    for key, value in (extra or {}).items():
        out[key] = (deep_merge(out[key], value) if isinstance(value, dict) and isinstance(out.get(key), dict)
                    else value)
    return out


def overlay_spec(value):
    """Parse --character-overlay JSON (or @FILE): {"peer"|"select"|"self": {offset: value}}."""
    raw = json.loads(Path(value[1:]).read_text(encoding="utf-8") if value.startswith("@") else value)
    unknown = set(raw) - {"peer", "select", "self"}
    if unknown:
        raise ValueError(f"unknown character objects {sorted(unknown)}")
    return {name: offset_keys(values) for name, values in raw.items()}


def character_extras(gear=None, overlay=None, layout=None):
    """Extra field values for the peer, select and self character objects: equipment from
    --character-gear, then a raw --character-overlay JSON {"peer"|"select"|"self": {offset: ...}}."""
    extras = {"peer": {}, "select": {}, "self": {}}
    for part in (gear_objects(gear, layout) if gear else {}, overlay or {}):
        for name in extras:
            extras[name] = deep_merge(extras[name], part.get(name))
    return extras


def character_item_objects(schema, character_id, profile):
    """Individual items for a character snapshot, using its merged inventory.

    Inventory summaries alone leave equipped instance lookups unresolved. The
    01.24 item object carries its ID at +0, owner at +8, quantity at +0x18 and
    item index at +0x24. Other instance attributes remain absent until known.
    Unknown builds retain their existing payload rather than using a guessed
    object fingerprint.
    """
    handle = SELF_ITEM_TYPE
    type_hash = SELF_ITEM_HASHES.get((schema.get(f"{handle:#x}") or {}).get("size"))
    if type_hash is None:
        return []
    layout = character_layout(schema)
    inventory = profile.get(layout["self_inventory"], {})
    entries = inventory.get(layout["inventory_items"], {})
    objects, seen = [], set()
    for offset, item in sorted(entries.items()):
        if offset < 0 or offset % layout["inventory_entry"]:
            raise ValueError(f"unaligned inventory entry offset {offset:#x}")
        instance = item.get(8, 0)
        if not instance:  # uninstanced/empty summary entries are not qz objects
            continue
        if instance in seen:
            raise ValueError(f"duplicate inventory instance {instance:#x}")
        seen.add(instance)
        values = {0: instance, 8: {0: character_id, 8: 0},
                  0x18: {0: item.get(0x10, 1)}, 0x24: item.get(0, -1)}
        objects.append(investment.obj(type_hash, instance, investment.encode(schema, handle, values)))
    return objects


def load_store(path):
    try:
        return json.loads(path.read_text(encoding="utf-8")) if path else {}
    except FileNotFoundError:
        return {}


def save_character(path, account_id, character_id, request_payload):
    """Remember one created character per local account (JSON; kept outside Git)."""
    store = load_store(path)
    store.setdefault(f"{account_id:#x}", {}).update(id=character_id, request=request_payload.hex())
    path.write_text(json.dumps(store, indent=1), encoding="utf-8")


def save_account_data(path, account_id, request_payload):
    """Remember the account's user data from its latest service 701 request (see
    account_data_offset), next to its character."""
    store = load_store(path)
    store.setdefault(f"{account_id:#x}", {})["account_data"] = request_payload.hex()
    path.write_text(json.dumps(store, indent=1), encoding="utf-8")


def account_data_offset(schema):
    """Offset of the account user data in the self account (0x808018c7): the field of the type
    service 701 uploads (01.24: 0x80801716 at +0x80, 1088 bytes, holding the profile-setup
    settings 0x808017f0 at +0x410/+0x10). The client sends 701 every ~30 s and at sign-in reads
    its settings back from this field; while it is absent every launch runs the first-run
    profile setup (Screen Bounds, Brightness, Subtitles). None when the schema has no match."""
    entry = (schema or {}).get("services", {}).get("701")
    request = schema.get(entry[0]) if entry else None
    data_type = next((f["nested"] for f in request["fields"] if f["off"] == 0 and f["kind"] == 1),
                     None) if request else None
    account = (schema or {}).get("0x808018c7", {}).get("fields", [])
    return next((f["off"] for f in account if f["kind"] == 1 and f["nested"] == data_type),
                None) if data_type is not None else None


def account_data_values(schema, request_payload):
    """Decoded account user data from a stored 701 request body (without the wire ID)."""
    reader = investment.BitReader(request_payload)
    return investment.decode_from(reader, schema, int(schema["services"]["701"][0], 16))[0]


def parse_flag_spec(spec, counts=None):
    """"2" (every flag) or "2@A0-511,B0-15" - value, then index ranges in account table A
    and B and character table C. Unqualified "2" fills each table's build-specific length."""
    counts = counts or {"A": 1024, "B": 256, "C": 1024}
    value, _, ranges = str(spec).partition("@")
    out = {"A": set(), "B": set(), "C": set()}
    if not ranges:
        out = {table: set(range(counts[table])) for table in out}
    for part in filter(None, ranges.split(",")):
        table, span = part[0].upper(), part[1:]
        if table not in counts:
            raise ValueError(f"unknown unlock flag table {table!r}")
        lo, _, hi = span.partition("-")
        first, last = int(lo), int(hi or lo)
        if not 0 <= first <= last < counts[table]:
            raise ValueError(f"unlock flag {table}{first}-{last} outside 0-{counts[table] - 1}")
        out[table].update(range(first, last + 1))
    return int(value), out


def account_flag_values(spec, counts=None):
    """Layered specs separated by ";" - later ones win, e.g. "1;2@B0" = every flag false
    (1) except B0 true (2). Returns {"A": {index: value}, "B": {...}}."""
    values = {"A": {}, "B": {}, "C": {}}
    for part in filter(None, str(spec).split(";")):
        value, chosen = parse_flag_spec(part, counts)
        for table, indexes in chosen.items():
            values[table].update(dict.fromkeys(indexes, value))
    return values


def flag_layout(schema):
    """Find the A/B/C unlock tables from the selected build's nested type layout."""
    def is_table(entry, minimum):
        fields = (entry or {}).get("fields", [])
        return (len(fields) >= minimum and entry.get("size") == len(fields)
                and all(fields[i]["off"] == i and fields[i]["kind"] == 3
                        and fields[i]["width"] == 2 and fields[i]["min"] == 1
                        for i in (0, 1, len(fields) - 1)))

    def table(owner, minimum):
        fields = schema.get(owner, {}).get("fields", [])
        matches = [(field["off"], field["nested"],
                    schema.get(f"{field['nested']:#x}", {}).get("fields", []))
                   for field in fields if field["kind"] == 1
                   and is_table(schema.get(f"{field['nested']:#x}"), minimum)]
        if len(matches) != 1:
            raise ValueError(f"expected one unlock flag table in {owner}, found {len(matches)}")
        offset, handle, entries = matches[0]
        return {"offset": offset, "handle": handle, "fields": entries, "count": len(entries)}

    account = schema.get("0x808018c7", {}).get("fields", [])
    wrappers = [field for field in account if field["kind"] == 1 and field["nested"] == 0x80801807]
    if len(wrappers) != 1:
        raise ValueError("expected one account B flag wrapper")
    return {"A": table("0x808018c7", 1024),
            "B": {**table("0x80801807", 256), "wrapper_offset": wrappers[0]["off"]},
            "C": table("0x80801714", 1024)}


def account_flags(schema, spec, layout=None, values=None):
    """Self root account flag tables, read by the unlock-expression evaluator (0xb674ec /
    0xb67564 in 01.00). Values are tri-state (2 = true)."""
    layout = layout or flag_layout(schema)
    if values is None:
        values = account_flag_values(spec, {key: item["count"] for key, item in layout.items()})
    def slots(table):
        return {f["off"]: values[table][i] for i, f in enumerate(layout[table]["fields"])
                if i in values[table]}
    return {layout["A"]["offset"]: slots("A"),
            layout["B"]["wrapper_offset"]: {layout["B"]["offset"]: slots("B")}}


def active_character_offset(schema):
    """Account's active character SOID: 01.00 +0x60; updates insert a u32 before it."""
    fields = (schema.get("0x808018c7") or {}).get("fields", [])
    if not fields:
        return 0x60  # historical/minimal schemas
    roster = next((i for i, field in enumerate(fields)
                   if field.get("nested") == 0x80804621), None)
    if roster is not None:
        tail = fields[roster + 1:]
        if tail and tail[0]["kind"] == 9:
            tail = tail[1:]
        if tail and tail[0]["kind"] == 6:
            return tail[0]["off"]
    raise ValueError("unsupported account active-character layout")


def family_blob(schema, family_type, root, character_id=None, character_profile=None,
                version=1, flags=1, active_id=None, self_flags=None, peer=False, extras=None,
                account_data=None):
    """qz payload for one family. For self (4), `active_id` fills the account's active
    character: the user manager copies it to its entry +0x20 (0x2942fc) once a self
    character object with that id exists (0x29305c), and the bootflow's character step
    (0x32f748 via 0x325350) proceeds only when it is set."""
    objects = []
    extras = extras or {}
    flags_layout = flag_layout(schema) if schema and family_type == 4 and self_flags is not None else None
    flag_values = (account_flag_values(self_flags, {key: item["count"] for key, item in flags_layout.items()})
                   if flags_layout else None)
    if schema and family_type in ROOT_OBJECTS:
        type_hash, handle, values = ROOT_OBJECTS[family_type]
        fields = values(root)
        if character_id is not None and family_type in (3, 4):
            character_list = {0: 1, 8: [character_id]}
            if family_type == 3:
                fields[8] = character_list
            else:
                fields[0][8] = character_list
                if active_id is not None:
                    fields[0][active_character_offset(schema)] = active_id
        if family_type == 4 and self_flags is not None:
            fields[0].update(account_flags(schema, self_flags, flags_layout, flag_values))
        if family_type == 4 and account_data is not None and account_data_offset(schema) is not None:
            fields[0][account_data_offset(schema)] = account_data
        objects.append(investment.obj(object_hash(schema, type_hash, handle), root, investment.encode(schema, handle, fields)))
        if family_type == 3 and character_id is not None and character_profile is not None:
            type_hash, handle = SELECT_CHARACTER_OBJECT
            objects.append(investment.obj(object_hash(schema, type_hash, handle), character_id, investment.encode(
                schema, handle, deep_merge(character_profile, extras.get("select")))))
        if family_type == 4 and character_id is not None and character_profile is not None:
            type_hash, handle = SELF_CHARACTER_OBJECT
            profile = deep_merge(character_profile, extras.get("self"))
            if self_flags is not None:
                chosen = flag_values["C"]
                profile[flags_layout["C"]["offset"]] = {
                    f["off"]: chosen[i] for i, f in enumerate(flags_layout["C"]["fields"])
                    if i in chosen}
            objects.append(investment.obj(object_hash(schema, type_hash, handle), character_id,
                                          investment.encode(schema, handle, {0: profile})))
            objects.extend(character_item_objects(schema, character_id, profile))
    if peer and schema and family_type == 0 and character_id is not None and character_profile is not None:
        type_hash, handle = PEER_ACCOUNT_OBJECT
        objects.append(investment.obj(object_hash(schema, type_hash, handle), root, investment.encode(schema, handle, {0: root, 8: character_id})))
        type_hash, handle = PEER_CHARACTER_OBJECT
        summary = deep_merge({off: character_profile[off] for off in (0, 8, 12) if off in character_profile},
                             extras.get("peer"))
        objects.append(investment.obj(object_hash(schema, type_hash, handle), character_id, investment.encode(schema, handle, summary)))
    # flags bit 0 = full snapshot: required for a new subscription (0x41b75c, slot state 2).
    # Updates to an existing subscription use a delta and the next family version.
    return investment.qz_payload([investment.family(family_type, root, objects,
                                                     version=version, flags=flags)])

CAPTURE_DIR = SIGNON / "captures" / "bap"
SHELLO_REQ, SHELLO_RSP = 0x19, 0x1A
# 01.24 opens with "c->b start req" (0x1e): plaintext, payload "DESTINY" 0x01 + 120 bytes of
# LCG output (builder 0x8c74e4). The reply dispatcher 0x8c7c94 expects "start rsp" (0x1f) for
# request kind 1, checks status 200 and decodes the payload into a discarded 0x800-byte buffer;
# the start rsp decoder (codec vtable slot 3, 0x478764) accepts exactly 0x80 bytes.
START_REQ, START_RSP = 0x1E, 0x1F
QZ_REG_REQ, QZ_REG_RSP = 0x79, 0x7A
ECHO_REQ, ECHO_RSP, RRC_REQ, RRC_RSP = 0xFA, 0xFB, 0x12E, 0x12F
WS_REQ, WS_RSP = 0x0A, 0x0B
SUB_REQ, SUB_RSP = 0x0C, 0x0D
QZ_UPDATE = 0x7B
ACCT_ID_TRANS_REQ, ACCT_ID_TRANS_RSP = 0x17, 0x18
CLIENT_CONFIG_REQ, CLIENT_CONFIG_RSP = 0x12, 0x13
# Activity host proxy assignment: u64 request id; reply = echoed u64 | IPv4 | u16 | port.
# 01.24 consumer 0x48ea44 rejects a reply whose first u64 differs from request state +8
# (comparison at 0x48ea78 through 0x1da944), before storing the address and accepting it.
AHP_REQ, AHP_RSP = 0x10, 0x11
# b->c notification push (type 9; decoder 0x416064): u8 flag, u64 id (flag 1), u32 subtype,
# u32 length, data. Subtype 1 appears to reach the ahp consumer 0x42a5a0 (16 bytes).
BC_NOT = 0x09
# Activity host proxy connection (address from the ahp reply): the client sends
# c->ah mgr req (0x06, 3103 bytes: u8 subtype + body); the reply c->ah mgr rsp (0x07) must be
# exactly 137 bytes, subtype 2 + 136 bytes (decoder 0x415be4). Contents still unknown.
AH_MGR_REQ, AH_MGR_RSP = 0x06, 0x07
# Client -> activity host message relayed by the proxy: u64 host id | u8 1 | u32 type |
# u32 length | u32 0 | body (type 3 = host join request carrying account and character ids).
AH_NOT = 0x08
# Exact routine notification observed repeatedly in 01.24. Keep its full capture
# and normal processing, but avoid two flushed stdout writes per packet by default.
ROUTINE_TYPE15_PAYLOAD = struct.pack(">BIII", 1, 15, 1, 0) + b"\x00"
# Activity host messages are typed structs in the investment bit format (0x25bd0c /
# 0x25bc88). Client -> host types come from the table at *(0x179eba4), host -> client types
# from *(0x179eba8); the schema is exported with scripts/rpcs3_tiger_types.py (outside Git).
# Join: the client sends c->ah not type 3 with a 0x80800e98 struct and keeps a copy
# (0x259c84; its first i32 is a random token). The host answers with b->c not type 4 (handler 0xd0d560 -> 0x256ad8), a
# 0x80800a58 struct: @0 the request echoed (its first i32 must match the copy), @0xf8 a
# result (0 = joined, stored as bits - 1 in 3 bits), @0xf9 char[128] activity session id,
# then i32s @0x17c..0x188, u8 @0x18c, u16s @0x18e/0x190 (kept at 0x6930/0x6934), i32 @0x194.
# These are 01.00 offsets. In 01.13/01.24 the request grows by 8 bytes, so the result and
# session id move to +0x100/+0x101; join_response() finds them in the selected schema.
# Next the client waits for its entity indices: b->c not type 0 (handler 0xd0d020), a
# 0x80800d64 struct holding a 384-word bitmap (0x808043a7) that is ORed into the client's
# index set (+0x5bf8) and marks the indices received (+0x61f8).
# The client counts as established (0x258324) once it also has host flag 0x100 (type 11,
# body ignored, handler 0x25715c) and an activity state: type 1 (handler 0xd0d250), a
# 0x80800e76 struct whose 0xa20-byte body is copied into the simulation (0x1c95f0) and must
# have bool @0 set and either bool @1 clear or i32 @0x10 > 0. Loading the activity then
# needs the map name (0x305a2c): state @0x60 (0x80800cc5) @0x994 (0x8080409f) = char[40],
# looked up as a resource key (0x1f6564, e.g. "orbit"; ":scenario_fah" etc. for parts).
# The world id is the i16 at state @0x60 @0x990 (0x3254ec; checked for DLC by 0x4105c4,
# "World-transition requires DLC."); service 105 carries the world the client asked for.
# Those scene offsets are 01.00. The static 01.13/01.24 export moves world/map to
# +0x5c8/+0x5d0; activity_state() reads the selected schema.
AH_JOIN_REQUEST, AH_JOIN_RESPONSE, AH_ENTITY_INDICES = 3, 4, 0
AH_STATE, AH_ESTABLISH = 1, 11
STATE_TYPE = 0x80800E76
JOIN_REQUEST_TYPE, JOIN_RESPONSE_TYPE = 0x80800E98, 0x80800A58
ENTITY_INDICES_TYPE, ENTITY_WORDS = 0x80800D64, 384
ACTIVITY_SCHEMA = SIGNON / "activity_schema.json"


def ahp_response(request, configured):
    """Correlate a valid proxy assignment with its request, keeping the configured endpoint.

    Malformed lengths remain raw experiment payloads, as before; never manufacture a valid
    assignment by truncating a request or padding a configured reply.
    """
    if len(request) == 8 and len(configured) == 16:
        return request + configured[8:]
    return configured


def char_array(text, size=128):
    """A char[size] field (type 0x80804056 -> 0x80804008: size i8 fields, one per byte)."""
    data = text.encode("latin1")[:size - 1]
    return {0: dict(enumerate(data))}


def join_request(schema, body):
    return investment.decode(schema, JOIN_REQUEST_TYPE, body)


def entity_indices(schema, first, last):
    """Bitmap granting entity indices first..last-1 (bit order inside a word is unverified,
    so grant whole words: multiples of 32)."""
    words = [0] * ENTITY_WORDS
    for index in range(first, last):
        words[index // 32] |= 0x80000000 >> (index % 32)
    return investment.encode(schema, ENTITY_INDICES_TYPE, {0: {4 * i: w for i, w in enumerate(words)}})


def activity_state(schema, map_name=None, world=None, checksum=None, bubbles=None, zone=None,
                   bubble_states=None):
    values = {0: True, 0x10: 1}
    if zone is not None:
        # u32 @0x54 object index / i32 @0x58 name hash: the zone record the world controller
        # adopts on arrival (0x31d778 via activity object +0x50b0, slice set +0x130). The leave
        # check (0x31bfe0) then requires that object in the scenario object table; index -1
        # (hash 0x811c9dc5, FNV of "") is the game's default and skips the check.
        values[0x54], values[0x58] = zone
    if bubbles:
        # i32 @0x10 = bubble count (must equal the scenario's, def +0x30; 0x1c7608 "activity
        # client bubble count mismatch"), i8[64] @0x14 = per-bubble state index. The sim start
        # (0x1c7608) registers bubble i's zone objects in the scenario object table from it:
        # -1 leaves the base entry (index 8*i) absent, 0.. registers the bubble's base entry and
        # its sub-states (8*i + state + 1, 0x14a8e8); zone sets load only from present entries.
        states = dict(bubble_states or {})
        default = states.pop(None, -1)
        values[0x10] = bubbles
        values[0x14] = {i: states.get(i, default) for i in range(bubbles)}
    if checksum is not None:
        # u32 @0x5c: the activity checksum; 0x258550 compares it with the loaded scenario's
        # ("activity checksum mismatch" fails the world load).
        values[0x5C] = checksum
    scene = {}
    if map_name or world is not None:
        scene_fields = schema["0x80800cc5"]["fields"]
        map_offset = field_offset(schema, "0x80800cc5", "0x8080409f")
        world_fields = [f for f in scene_fields if f["kind"] == 4 and f["width"] == 16
                        and f["min"] == 0x8000]
        if map_offset is None or len(world_fields) != 2 or world_fields[1]["off"] >= map_offset:
            raise ValueError("unsupported activity scene layout")
        if map_name:
            scene[map_offset] = {0: char_array(map_name, 40)[0]}
        if world is not None:
            scene[world_fields[0]["off"]] = world
    if scene:
        values[0x60] = scene
    return investment.encode(schema, STATE_TYPE, {0: values})


WORLD_STATE_TYPE = 0x808009ED
ROSTER_TYPE, ROSTER_SLOTS = 0x80800CF7, 65
AH_SIMULATION = 5
OBJECT_ID_TYPE, SIM_PLAYER_TYPE, GAME_MODE_TYPE = 0x808004B1, 0x808022A5, 0x808008A3
AH_MACHINES, MACHINES_TYPE = 43, 0x80800F1A
# Client -> host type 21: one present bit + a delta of the client's 0x220-byte state
# 0x80800c56. On 01.24 the sender is 0x2720e4; host +0x66e0 is the sent baseline and
# +0x6900 the pending state encoded by 0x1db1cc. A successful send copies pending state
# to the baseline at 0x27241c. The earlier 01.00 addresses are 0x257d28,
# +0x6498/+0x66b8 and 0x1ca00c; they must not be used as 01.24 trace points. Its
# +0xb8 is the world controller's slice-set record 0x80800d9f: current record @0, target record
# @0xa0 (0x80800d82: i32 slice-set index, u32 hash, i32), i8 @0x140/0x141, u3 state @0x142. The
# client is the authority over its slice set and reports switches here (TEST366: target 112,
# then current 112 with state 0).
AH_CLIENT_STATE, CLIENT_STATE_TYPE = 21, 0x80800C56

# How long a finished teleport request (state 3) stays in the world state before it is dropped
# (the client respawns the player while handling state 3; JOIN61 dropped it 50 ms after sending)
TELEPORT_SETTLE_SECONDS = 10

# client -> host type 17 {u32 counter, u32 status}: 0x7fffffff while loading, 0x8000000e once the
# client is in the world (JOIN53: 14 s after the world state, again after each slice-set switch)
AH_CLIENT_PROGRESS, CLIENT_IN_WORLD = 17, 0x8000000E
# Activity-host message 18 is "incident" (name table at 0x1898168 in 01.24; payload 0x8080097d:
# incident type list, 49-byte body). Clients send them (JOIN81: "player entered trigger"), and hosts
# can send them too (handler 0xe2e9ec).
AH_INCIDENT = 18


def client_in_world(body):
    return len(body) >= 8 and struct.unpack_from(">I", body, 4)[0] == CLIENT_IN_WORLD


def client_slice_record(schema, body):
    """The slice-set part of a type-21 client state delta: {"current"/"target": (index, hash),
    "state": n} with the keys the delta carries, or None (no slice-set part, or no schema)."""
    if not schema or f"{CLIENT_STATE_TYPE:#x}" not in schema or not body:
        return None
    reader = investment.BitReader(body)
    if not reader.get(1):
        return None
    zone = investment.decode_from(reader, schema, CLIENT_STATE_TYPE).get(0xB8)
    if not zone:
        return None
    record = {name: (zone[off].get(0), zone[off].get(4, 0))
              for name, off in (("current", 0), ("target", 0xA0)) if off in zone}
    if 0x142 in zone:
        record["state"] = zone[0x142]
    if 0x140 in zone:
        # i8 @0x140 = engine +0x1d0: the world controller copies a world-state group record only
        # when that record's per-peer byte (@0x38 + local slot) equals it.
        record["peer_byte"] = zone[0x140]
    return record or None


def client_slice_state(schema, body):
    """One-line summary of client_slice_record, or None."""
    record = client_slice_record(schema, body)
    if not record:
        return None
    parts = [f"{name} {record[name][0]}:{record[name][1]:#010x}" for name in ("current", "target")
             if name in record]
    if "state" in record:
        parts.append(f"state {record['state']}")
    if "peer_byte" in record:
        parts.append(f"peer byte {record['peer_byte']}")
    return ", ".join(parts)


def requested_client_zone(record):
    """Choose an actual slice request; a cleared zero record is not zone zero.

    The client sends target {index=0, hash=0} when clearing a transition.
    Empty names use FNV 0x811c9dc5, including a valid zone-zero request.
    Treating the cleared record as Skywatch makes the stand-in initiate an
    unrelated load while the client is completing its previous transition.
    """
    for name in ("target", "current"):
        index, name_hash = record.get(name, (None, None))
        if index is not None and index >= 0 and name_hash:
            return index
    return None


def machine_list(schema, machine_ids):
    """Host type 43 (0xd0e214): {5-bit count, up to 20 8-byte machine ids} - a **boot list**.
    Each listed id sets the flag of the matching entry of the host's machine list (host
    +0x6204 count, 10-byte records at +0x6208 = id + flag); the per-host update (0x30956c ->
    0x25afcc) then moves that machine to state 4 and the client is removed from the game
    world (error bass, TEST176)."""
    return investment.encode(schema, MACHINES_TYPE, {0: {0: len(machine_ids), 4: list(machine_ids)}})


def bubble_reset_spec(text):
    """Inactive roster-slot generations to test the client's bubble reset path."""
    if not isinstance(text, str):
        raise ValueError("bubble resets expect a SLOT[:GEN],... string")
    resets = []
    for part in text.split(","):
        fields = part.split(":")
        if len(fields) not in (1, 2) or not fields[0]:
            raise ValueError("bubble resets expect SLOT[:GEN],...")
        slot = int(fields[0], 0)
        generation = int(fields[1], 0) if len(fields) == 2 else 1
        if not 0 <= slot < ROSTER_SLOTS or not 1 <= generation <= 0xFFFF:
            raise ValueError("bubble reset slots must be 0..64 and generations 1..65535")
        if any(existing == slot for existing, _ in resets):
            raise ValueError(f"duplicate bubble reset slot {slot}")
        resets.append((slot, generation))
    return tuple(resets)


def update_bubble_resets(current, changes, client_slot):
    """Merge one validated control batch without mutating the shared startup options."""
    merged = dict(current)
    merged.update(changes)
    resets = tuple(merged.items())
    simulation_roster(None, client_slot, 0, resets)  # validate the entire update atomically
    return resets


def simulation_roster(peer_id, slot, generation, bubble_resets=()):
    """Keep the client slot active while changing only explicit inactive slots."""
    if not 0 <= slot < ROSTER_SLOTS or not 0 <= generation <= 0xFFFF:
        raise ValueError("client roster slot or generation is out of range")
    slots = {0xC * slot: {0: peer_id, 8: generation, 0xA: True}}
    for reset_slot, reset_generation in bubble_resets:
        if not 0 <= reset_slot < ROSTER_SLOTS or not 1 <= reset_generation <= 0xFFFF:
            raise ValueError("bubble reset slot or generation is out of range")
        if 0xC * reset_slot in slots:
            raise ValueError(f"bubble reset slot {reset_slot} collides with the client or another reset")
        slots[0xC * reset_slot] = {8: reset_generation, 0xA: False}
    return slots


def sim_packet(schema, peer_id, slot=0, generation=1, epoch=0, tick=0, objects=(),
               bubble_resets=()):
    """Host type 5 (0x25b920 -> session receive -> event 0x13 -> 0x1c85e0): u8 epoch, roster
    present bit, 65 changed-slot bits, roster delta (present bit + type 0x80800cf7: 65 slots of
    {opt 8-byte peer id, opt u16, bool active}; changed slots add/remove peers via 0x1c8280),
    u32 host tick, then the replicated object stream (0x1ce4a4; only read when the session's
    simulation is active and the epoch matches): a start bit (0 = no objects), then per object
    a 1 bit, its id (type 0x808004b1), the u32 bit length of the rest (used to skip unknown
    objects), state A (reset bit + present bit + the record's +0xc type) and, when the record has
    a state B type (+8), a 0 bit for it; a 0 bit ends the list. `objects` holds (id values,
    state A type or None for a default baseline, state values, has state B). Replicated state is
    applied only once every object of the active bubbles has had a baseline (`0x1ce1b8`: record
    +0x15), so all of them must be listed. `bubble_resets` explicitly changes inactive roster
    slots without assigning a peer. In 01.24, 0x1d90e0 arms both reset flags on a changed
    generation; 0x1d8b6c clears the waiting flag after that bubble's baselines are ready.
    Repeated packets preserve the generation, so a heartbeat does not reset it again.
    This opt-in experiment does not establish retail authority ownership semantics."""
    roster = simulation_roster(peer_id, slot, generation, bubble_resets)
    w = investment.BitWriter()
    w.put(epoch, 8)
    w.put(1, 1)
    for i in range(ROSTER_SLOTS):
        w.put(1 if 0xC * i in roster else 0, 1)
    w.put(1, 1)
    investment.encode_into(w, schema, ROSTER_TYPE, {4: roster})
    w.put(tick, 32)
    w.put(1 if objects else 0, 1)
    for object_id, state_type, state, has_b in objects:
        w.put(1, 1)
        investment.encode_into(w, schema, OBJECT_ID_TYPE, object_id)
        body = investment.BitWriter()
        body.put(1, 1)
        body.put(0 if state_type is None else 1, 1)
        if state_type is not None:
            investment.encode_into(body, schema, state_type, null_object_refs(schema, state_type, state))
        if has_b:
            body.put(0, 1)
        w.put(len(body.bits), 32)
        w.bits += body.bits
    if objects:
        w.put(0, 1)
    return w.data()


NULL_OBJECT_REF = {0: 0x811C9DC5, 4: -1, 6: -1}


def null_object_refs(schema, handle, values):
    """`values` with every unset, non-optional object reference (type 0x808004b1) set to the
    null reference {empty-name group, kind -1, index -1}, recursively. Unset fields are otherwise
    encoded as zeros, and {group 0, kind 0, index 0} is a real reference to the client: JOIN32's
    revival cinematic (state +0xc, the area the player must be inside) resolved it to nothing and
    crashed in 0x2160ac (read of 0x110). The client's own reset state holds the null reference."""
    out = dict(values or {})
    for field in schema[f"{handle:#x}"]["fields"]:
        if field["kind"] != 1 or field["arr"] == 1:
            continue
        off, nested = field["off"], field["nested"]
        if nested == OBJECT_ID_TYPE:
            if off not in out and not field["opt"]:
                out[off] = dict(NULL_OBJECT_REF)
        elif off in out or not field["opt"]:
            out[off] = null_object_refs(schema, nested, out.get(off))
    return out


def sim_player(schema, group, index, machine_id, spawn=None):
    """Replicated state of simulation player INDEX (kind 0xc entities of the world's entity group
    GROUP; record +0xc type 0x808022a5 = the entity's 0x50-byte block at +0x144): the bools at +8
    (loading task 9, 0x3286a8) and +9 (spawn readiness 0x35793c) set, owner machine id at
    +0x18 in 01.00 (0x80800646, entity +0x15c), which 0x4ed7b4 -> 0x14a270 matches against the
    local player (pla +0x18) to bind it (pla +0x50). The 01.24 state puts the owner at +0x3c
    and wraps the spawn flag in a struct at +0x6c; the offsets come from the selected schema."""
    object_id = {0: group, 4: 0xC, 6: index}
    fields = schema[f"{SIM_PLAYER_TYPE:#x}"]["fields"]
    owner_offset = field_offset(schema, f"{SIM_PLAYER_TYPE:#x}", "0x80800646")
    tail = fields[-2:]
    if (owner_offset is None or len(tail) != 2 or tail[1]["kind"] != 5
            or tail[1]["width"] != 32):
        raise ValueError("unsupported simulation player layout")
    state = {0: {0: -1, 4: 0x811C9DC5, 8: True, 9: True},
             owner_offset: {0: machine_id}, tail[1]["off"]: -1}
    if spawn:
        # 01.00 bool +0x48 (entity +0x18c, reached through entity +0x1ec/+0x1f0 by 0x4ec7b4): the spawn
        # processing (0x358688) sets a request flag on another component and waits for it.
        spawn_field = tail[0]
        if spawn_field["kind"] == 2:
            state[spawn_field["off"]] = True
        elif (spawn_field["kind"] == 1 and
              schema[f"{spawn_field['nested']:#x}"]["fields"][0]["kind"] == 2):
            state[spawn_field["off"]] = {0: True}
        else:
            raise ValueError("unsupported simulation player spawn field")
    return object_id, SIM_PLAYER_TYPE, state, True


def game_mode(schema, group, index, state):
    """Replicated state of the game-mode object (kind 0x10; type 0x808008a3 at entity +0x110).
    Its first field, a 4-bit state, is what loading task 9 (`0xb26e4c`, must be non-zero) and the
    spawn check `0x35793c` (jump table 0x11ed724 indexed by the value: only 3 continues) read."""
    fields = schema[f"{GAME_MODE_TYPE:#x}"]["fields"]
    nested = field_offset(schema, f"{GAME_MODE_TYPE:#x}", "0x8080533d")
    hashes = [(i, f) for i, f in enumerate(fields) if f["kind"] == 9 and f["width"] == 32
              and nested is not None and f["off"] > nested]
    if (not hashes or hashes[0][0] == 0 or fields[hashes[0][0] - 1]["kind"] != 5):
        raise ValueError("unsupported game mode layout")
    index_field = fields[hashes[0][0] - 1]["off"]
    hash_field = hashes[0][1]["off"]
    values = {0: state, 8: 0x811C9DC5, 0xC: -1, index_field: -1,
              hash_field: 0x811C9DC5}
    # Activity quarantine (01.24: i32 bubble @0x4d8, object id @0x4dc, u32 hash @0x4e4). The
    # apply handler 0xc0f740 sets `activity_quarantine` for a bubble in 0..127 and clears it
    # otherwise; once set, the watchdog 0x39bce8 sends a player who stays outside that bubble
    # back to its base slice set. A zero default meant "bubble 0": JOIN25/26 lost the world to
    # a {0, 0} slice request as soon as the Guardian walked from 112 into bubble 3.
    for i in range(len(fields) - 2):
        bubble, area, name = fields[i:i + 3]
        if (bubble["kind"] == 5 and bubble["width"] == 32 and area["kind"] == 1
                and area["nested"] == OBJECT_ID_TYPE and name["kind"] == 9 and name["width"] == 32):
            values[bubble["off"]], values[name["off"]] = -1, 0x811C9DC5
    return {0: group, 4: 0x10, 6: index}, GAME_MODE_TYPE, values, False


def baseline_list(value):
    """Parse KEY/KIND/INDEX[/B],... (or @FILE holding that list) into (key, kind, index, has_b)."""
    if value.startswith("@"):
        value = Path(value[1:]).read_text(encoding="utf-8")
    return [tuple(int(y, 0) for y in (x.strip().split("/") + ["0"])[:4]) for x in value.split(",") if x.strip()]


def offset_keys(v):
    """Normalize JSON field offsets, retaining a validated typed-value wrapper."""
    if isinstance(v, dict):
        if set(v) == {"type", "values"}:
            handle, payload = v["type"], v["values"]
            if isinstance(handle, bool) or not isinstance(handle, (int, str)):
                raise ValueError("typed value needs an integer type handle")
            handle = int(handle, 0) if isinstance(handle, str) else handle
            if not 0 <= handle < investment.NULL_HANDLE:
                raise ValueError("typed value needs a non-null 32-bit type handle")
            if not isinstance(payload, dict):
                raise ValueError("typed value fields must be a mapping")
            return {"type": handle, "values": offset_keys(payload)}
        return {int(k, 0): offset_keys(x) for k, x in v.items()}
    if isinstance(v, list):
        return [offset_keys(x) for x in v]
    return v


SQUAD_STATE_TYPE = 0x8080071E
SQUAD_EXISTING_MEMBERS_OFFSET = {0x60: 0x5C, 0x7C: 0x74}


def mission_offset_keys(values, schema, state_type):
    """Resolve the squad existing-member policy for the selected client build."""
    if "squad_existing_members" not in values:
        return offset_keys(values)
    if state_type != SQUAD_STATE_TYPE:
        raise ValueError("squad_existing_members requires a squad state")
    entry = schema.get(f"{state_type:#x}", {})
    offset = SQUAD_EXISTING_MEMBERS_OFFSET.get(entry.get("size"))
    field = next((f for f in entry.get("fields", []) if f["off"] == offset), None)
    if offset is None or field is None or field["kind"] != 3 or field["width"] != 2:
        raise ValueError(f"unsupported squad state layout for {state_type:#x}")
    result = dict(values)
    if str(offset) in result or f"{offset:#x}" in result:
        raise ValueError("squad_existing_members conflicts with a numeric offset")
    result[str(offset)] = result.pop("squad_existing_members")
    return offset_keys(result)


def object_spec(value):
    """Parse --ah-object GROUP:KIND:INDEX:TYPE[:B]=JSON into an sim_packet object: JSON maps field
    offsets (decimal or 0x strings) to values, nested dicts for struct fields, lists for counted
    arrays, e.g. 0x992bb471:0x20:7:0x80802a1b={"0": 1}; =@FILE reads the JSON from a file."""
    head, _, body = value.partition("=")
    parts = [int(x, 0) for x in head.split(":")]
    group, kind, index, state_type = parts[:4]
    has_b = bool(parts[4]) if len(parts) > 4 else False

    if body.startswith("@"):  # JSON in a file: command lines mangle embedded quotes (Start-Process)
        body = Path(body[1:]).read_text(encoding="utf-8")
    return {0: group, 4: kind, 6: index}, state_type, offset_keys(json.loads(body or "{}")), has_b


def timed_object_spec(value):
    """Parse --ah-object-at SECONDS@GROUP:KIND:INDEX:TYPE[:B]=JSON: (seconds, object_spec)."""
    seconds, _, spec = value.partition("@")
    return float(seconds), object_spec(spec)


def reactive_object_spec(value):
    """Parse --ah-on TYPE[/REGEX]=>SECONDS@GROUP:KIND:INDEX:TYPE[:B]=JSON: (client message type,
    compiled regex over the body hex or None, delay in seconds, object_spec)."""
    trigger, sep, action = value.partition("=>")
    if not sep:
        raise ValueError("--ah-on expects TYPE[/REGEX]=>SECONDS@OBJECT")
    ah_type, _, pattern = trigger.partition("/")
    seconds, spec = timed_object_spec(action)
    return int(ah_type, 0), re.compile(pattern) if pattern else None, seconds, spec


def replace_object(objects, spec):
    """Replace (or add) the replicated object that `spec` names in `objects`; returns its
    (group, kind, index)."""
    ident = (spec[0][0], spec[0][4], spec[0][6])
    objects[:] = [o for o in objects if (o[0][0], o[0][4], o[0][6]) != ident] + [spec]
    return ident


ANY_MESSAGE = -1  # --ah-on / mission rule type matching every client message type
WORLD_HEARTBEAT = -2  # --ah-heartbeat SECONDS:world


class ObjectIdMatcher:
    """Matches a client message body (hex, as `--ah-on` regexes see it) whose bits contain the
    encoded id of one replicated object (type 0x808004b1: u32 group key, 6-bit kind, 16-bit
    index) at any bit offset. The upstream object stream (client type 6) carries the ids of the
    objects the client reports (TEST366/367: its own player 0x4786c0e0/0xc/5 at bit 11), so a
    trigger the guardian walks into should show up the same way."""

    def __init__(self, schema, group, kind, index):
        w = investment.BitWriter()
        investment.encode_into(w, schema, OBJECT_ID_TYPE, {0: group, 4: kind, 6: index})
        self.bits, self.ident = "".join(map(str, w.bits)), (group, kind, index)

    def search(self, body_hex):
        return self.bits in "".join(f"{b:08b}" for b in bytes.fromhex(body_hex))


class SliceSetMatcher:
    """Matches a client type-21 state delta (body hex) whose slice-set record reports slice set
    `index` as current - e.g. the client has switched into The Breach (24) by walking there."""

    def __init__(self, schema, index):
        self.schema, self.index = schema, index

    def search(self, body_hex):
        record = client_slice_record(self.schema, bytes.fromhex(body_hex))
        return bool(record) and record.get("current", (None,))[0] == self.index


class CinematicCompletionMatcher:
    """01.24 incident 734, with its kind-6 ID at the measured payload field.

    Native 0x580640 posts this incident when the performance ends or is stopped. Matching
    the ID at any bit would also match an unrelated field or a started incident.
    JOIN185 verifies the 53-byte body and object ID at bit 303 for both sequences.
    A matching incident permits the handover; it does not prove natural completion,
    visible actors or audio. Natural completion requires a separate scene trace.
    """
    def __init__(self, schema, ident):
        validate_incident_schema(schema, INCIDENT_HANDLE)
        if ident[4] != 6:
            raise ValueError("cinematic completion needs a kind-6 object")
        self.schema, self.ident = schema, (ident[0], ident[4], ident[6])

    def search(self, body_hex):
        try:
            if len(body_hex) > 16384:
                return False
            payload = bytes.fromhex(body_hex)
            if len(payload) > 8192:
                return False
            types, body = decode_incident(self.schema, INCIDENT_HANDLE, payload)
            if types != (734,) or len(body) != 53:
                return False
            reader = investment.BitReader(body)
            reader.position = 303
            ident = investment.decode_from(reader, self.schema, OBJECT_ID_TYPE)
            reader.get(64)  # two native trailing words
            padding = len(body) * 8 - reader.position
            return (0 <= padding < 8 and not reader.get(padding)
                    and tuple(ident.get(off) for off in (0, 4, 6)) == self.ident)
        except (KeyError, ValueError, TypeError):
            return False


def resolve_values(value, context):
    """Fill mission placeholders at the moment a state is sent: "$player" (the local player's
    machine id, the --ah-sim-player owner) and "$now" (the host simulation tick, see
    --ah-sim-clock; 0 without it), optionally with an offset ("$now+300")."""
    if isinstance(value, dict):
        return {k: resolve_values(v, context) for k, v in value.items()}
    if isinstance(value, list):
        return [resolve_values(v, context) for v in value]
    if isinstance(value, str) and value.startswith("$"):
        match = re.fullmatch(r"\$(\w+)([+-]\d+)?", value)
        if not match or match.group(1) not in context:
            raise ValueError(f"unknown mission placeholder {value}")
        return context[match.group(1)] + int(match.group(2) or 0)
    return value


def resolve_spec(spec, context):
    ident, state_type, values, has_b = spec
    return ident, state_type, resolve_values(values, context), has_b


def expand_mission(mission, schema):
    """A mission script (--ah-mission JSON) as --ah-object-at and --ah-on entries.
    {"objects": {NAME: "GROUP:KIND:INDEX:TYPE[:B]"}, "steps": [{"at": SECONDS} or
    {"on": {"type": N, "object": NAME} | {"type": 21, "slice_set": INDEX} | {"type": N, "regex": R},
    "delay": SECONDS},
    plus "set": {NAME: {offset: value}}]}. Each "set" entry becomes one timed or reactive object
    change; reactive steps fire once, on the first matching client message."""
    heads = {name: object_spec(f"{head}=") for name, head in mission.get("objects", {}).items()}
    timed, reactive = [], []
    for number, step in enumerate(mission.get("steps", [])):
        scope = reaction_scope(step)
        changes = []
        for name, values in step.get("set", {}).items():
            if name not in heads:
                raise ValueError(f"mission step {number}: unknown object {name!r}")
            ident, state_type, _, has_b = heads[name]
            changes.append((ident, state_type, mission_offset_keys(values, schema, state_type), has_b))
        if "at" in step:
            timed += [(float(step["at"]), change) for change in changes]
            continue
        if "teleport" in step:
            if changes:
                raise ValueError(f"mission step {number}: separate teleport and state changes")
            changes.append(TeleportReaction(parse_slice_set(step["teleport"])))
        on = step.get("on")
        if not on or "type" not in on:  # "type": "any" matches every client message type
            raise ValueError(f"mission step {number}: needs \"at\" or \"on\": {{\"type\": ...}}")
        if "cinematic_complete" in on:
            if (on["cinematic_complete"] is not True or on.get("type") != AH_INCIDENT
                    or "object" not in on):
                raise ValueError("cinematic completion needs type 18 and an object")
            matcher = CinematicCompletionMatcher(schema, heads[on["object"]][0])
        elif "object" in on:
            ident = heads[on["object"]][0]
            matcher = ObjectIdMatcher(schema, ident[0], ident[4], ident[6])
        elif "slice_set" in on:
            matcher = SliceSetMatcher(schema, int(on["slice_set"]))
        else:
            matcher = re.compile(on["regex"]) if on.get("regex") else None
        on_type = ANY_MESSAGE if on["type"] == "any" else int(on["type"])
        reactive += [ReactionRule(on_type, matcher, float(step.get("delay", 0)), change, scope)
                     for change in changes]
    return timed, reactive


# Where a mission script's "at" times count from: "world" = when the stand-in sends the world
# state / simulation (the original behaviour), "ready" = the client's first in-world report
# (type 17 0x8000000e), so a script's times do not depend on how long the client takes to load.
MISSION_CLOCKS = ("world", "ready")


class ControlChanges(list):
    """Object specs retain the original list API; roster controls carry separate metadata."""

    def __init__(self):
        super().__init__()
        self.bubble_resets = []


def control_steps(path, offset, objects, schema):
    """DEBUG TOOL (--ah-control): mission-script steps appended to a file, one JSON object per
    line ({"set": {NAME: {offset: value}}}, names from the mission's "objects"; or
    {"teleport": SLICE_SET} or {"bubble_reset": "SLOT[:GEN],..."}), applied live. Returns
    (ControlChanges object-spec list with bubble_resets metadata, [slice sets to teleport to],
    new offset, [errors]). Each connection reads from the file size at
    its start, so earlier lines are not replayed. Replacement: an operator console on the
    activity-instance service (notes/architecture-multiplayer.md)."""
    try:
        size = path.stat().st_size
    except OSError:
        return ControlChanges(), [], offset, []
    if size <= offset:
        return ControlChanges(), [], size, []
    with path.open("rb") as f:
        f.seek(offset)
        data = f.read(size - offset)
    end = data.rfind(b"\n") + 1  # only complete lines
    specs, teleports, errors = ControlChanges(), [], []
    for line in data[:end].decode("utf-8", "replace").splitlines():
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        try:
            step = json.loads(line)
            if "bubble_reset" in step:
                specs.bubble_resets.append(bubble_reset_spec(step["bubble_reset"]))
                continue
            if "teleport" in step:
                teleports.append(parse_slice_set(step["teleport"]))
                continue
            timed, _ = expand_mission({"objects": objects, "steps": [{"at": 0, "set": step["set"]}]}, schema)
            specs += [spec for _, spec in timed]
        except (ValueError, KeyError, TypeError) as exc:
            errors.append(f"{line.strip()[:80]}: {exc}")
    return specs, teleports, offset + end, errors


def mission_clock(mission):
    clock = mission.get("clock", "world")
    if clock not in MISSION_CLOCKS:
        raise ValueError(f"mission clock {clock!r} is not one of {MISSION_CLOCKS}")
    return clock


def parse_slice_set(value):
    """A slice set as INDEX or "INDEX:NAMEHASH" -> (index, hash or None). Named sets need their
    hash in a teleport request (24 = 0x68935472 on the highway, JOIN71); None means "use the hash
    the client reported for this index, else the empty name"."""
    index, _, name_hash = str(value).partition(":")
    return int(index, 0), (int(name_hash, 0) if name_hash else None)


def mission_teleports(mission):
    """Timed host teleports of a mission script: steps {"at": SECONDS, "teleport": SLICE_SET}
    (e.g. the intro's hand-off 113 -> 112) as [((index, hash), seconds)]."""
    return [(parse_slice_set(step["teleport"]), float(step["at"])) for step in mission.get("steps", [])
            if "teleport" in step and "at" in step]


def validate_mission_states(schema, timed, reactive):
    """Catch fields silently omitted or changed by the selected build's encoder."""
    # Small stand-ins fit narrow synthetic test schemas; the preflight checks
    # field layout and literal values, not a particular live identity or tick.
    context = {"player": 1, "now": 1}

    def check(expected, actual, path):
        if isinstance(expected, dict):
            if not isinstance(actual, dict):
                raise ValueError(f"{path}: expected a struct")
            for offset, value in expected.items():
                label = f"{offset:#x}" if isinstance(offset, int) else offset
                if offset not in actual:
                    raise ValueError(f"{path}/{label}: field is absent from the encoded state")
                check(value, actual[offset], f"{path}/{label}")
        elif isinstance(expected, list):
            if not isinstance(actual, list) or len(expected) != len(actual):
                raise ValueError(f"{path}: array length changed during encoding")
            for index, value in enumerate(expected):
                check(value, actual[index], f"{path}/{index}")
        elif isinstance(expected, float):
            if not math.isclose(expected, actual, rel_tol=1e-6, abs_tol=1e-6):
                raise ValueError(f"{path}: encoded value changed from {expected} to {actual}")
        elif expected != actual:
            raise ValueError(f"{path}: encoded value changed from {expected} to {actual}")

    for spec in [s for _, s in timed] + [entry[3] for entry in reactive]:
        if isinstance(spec, TeleportReaction):
            continue
        ident, state_type, values, _ = resolve_spec(spec, context)
        path = f"{ident[0]:#x}/{ident[4]:#x}/{ident[6]} state {state_type:#x}"
        try:
            encoded = investment.encode(schema, state_type, values)
            check(values, investment.decode(schema, state_type, encoded), path)
        except KeyError as exc:
            raise ValueError(f"{path}: schema lacks {exc}") from exc
    return len(timed) + len(reactive)


def default_baseline(key, kind, index, has_b=False):
    """A baseline without fields (reset to the type's defaults) for replicated object key/kind/index."""
    return {0: key, 4: kind, 6: index}, None, None, has_b


def world_state_delta(schema, request, zone=None, peer_byte=1, group_peer=0, target=None, teleport=None,
                      version=1):
    """Body of host type 11 (0x25715c): one present bit, then a world-state delta (type
    0x808009ed, decoded with 0x1ca478 onto the host simulation's copy at +0xbc0 and posted to
    the world controller as RPC 6). Peer slot 0 is the joining client: its identity (@0x10)
    and player data (@0x38) from the join request, member mask @0x5028 and occupied mask
    @0x502c bit 0 (0x1c9ab8 finds the local slot by the 8-byte id). With `zone`, group record 0 (@0x2e48, 0x80800c6f)
    names that zone-set index with state 1, 5-bit peer field `group_peer`, and per-peer byte
    `peer_byte`, which
    the world controller copies to slice +0x310 when it matches its own +0x1d0 (0x31d06c)
    and which gates the zone-set switch (0x31c750)."""
    # Masks (bit per peer slot): @0x5028 = session members - the manager entry's member list
    # (0x81c0c4 via 0x1c9b98) that world-controller RPC 16 turns into the machine table, @0x502c
    # = identities to search for the local slot (0x1c9ab8).
    if not 0 <= group_peer < 20:
        raise ValueError("group peer must be a world-state peer slot (0-19)")
    # u32 @0 `version`: the world-state version of this activity instance. The world controller
    # re-reads the group records (0x351df0) only when it differs from its cached copy (engine
    # +0x3b0, initially 0), so a constant 0 meant no group record ever reached the client. The
    # caller owns the counter (one per activity instance's world state), never the process.
    client_state = {0: request[0x38]}
    values = {0: version, 8: {0: {0: request[0x10], 0x28: client_state}},
              0x5028: 1, 0x502C: 1}
    if zone is not None:
        records = {0xD8 * i: {0: -1} for i in range(1, 40)}  # unused groups: index -1
        records[0] = {0: zone, 8: {1: 1, 2: group_peer}, 0x28: {0x10: {0: peer_byte}}}
        if target is not None:
            # Group record 1 = the fireteam's slice-set switch in progress (01.24 engine +0x490..):
            # the client's phase updates 0x3518f4 / 0x351794 start the switch (phase 0x08 ->
            # 0x354acc -> 0x40 -> 0x354144 posts "Change slice-set") only when this record names a
            # zone with state 1 and the local slot as its peer.
            records[0xD8] = {0: target, 8: {1: 1, 2: group_peer}, 0x28: {0x10: {0: peer_byte}}}
        values[0x2E48] = {0: records}
    if teleport is not None:
        # Group block @0x2e48 + 0x21c8 (0x80800bf9 = {u3 state, {u8, i10 slice-set index, u32
        # hash}}): the host's slice-set request for the whole group. The world state is copied to
        # the activity simulation at +0x70, so this lands at sim+0x5080, where the request machine
        # 0x3adf8c latches state 1 into a pending switch (0x35097c), waits for the engine to reach
        # the index and finishes on state 3 (0x391068 / 0x72932c). Not optional: every world state
        # without it reads as "no request" (JOIN39/40).
        state, index, name_hash = teleport
        values.setdefault(0x2E48, {})[0x21C8] = {0: state, 4: {0: 0, 4: index, 8: name_hash}}
    w = investment.BitWriter()
    w.put(1, 1)
    investment.encode_into(w, schema, WORLD_STATE_TYPE, values)
    return w.data()


def join_response(schema, request_body, session_id="d1local-activity", result=0, extra=None):
    request = join_request(schema, request_body)
    fields = schema[f"{JOIN_RESPONSE_TYPE:#x}"]["fields"]
    results = [f["off"] for f in fields if f["kind"] == 3 and f["width"] == 3 and f["min"] == 1]
    if (len(results) != 1 or not any(f["off"] == results[0] + 1
                                    and f["nested"] == 0x80804056 for f in fields)):
        raise ValueError("unsupported activity join response layout")
    values = {0: request, results[0]: result, results[0] + 1: char_array(session_id)}
    values.update(extra or {})
    return investment.encode(schema, JOIN_RESPONSE_TYPE, values)


def bc_notification(subtype, data, ident=0):
    return struct.pack(">BQII", 1, ident, subtype, len(data)) + data
# On 01.00, service 104 carries the universe object at +8 (type 0x80801941,
# as in the sign-in reply); an absent universe root gives pig (0xa2).
WS_REPLY_VALUES = {104: {8: {0: 0x7FFFFFFFFFFFFFFF, 8: 1}}}
# Universe +8 behaves like the server clock (the director compares character timestamps
# with it, and the client reads no calendar clock of its own); its unit is still unknown.
UNIVERSE_TIME_UNITS = {"one": lambda t: 1, "s": lambda t: int(t), "ms": lambda t: int(t * 1e3),
                       "us": lambda t: int(t * 1e6),
                       "filetime": lambda t: int((t + 11644473600) * 1e7)}


def universe_service(schema=None):
    """The service whose reply carries the universe object (type 0x80801941 at +8): 104 on 01.00.
    01.24's 104 returns a bool and 205 returns the universe; the client fetches it after the
    prologue and fails with pig (family record not found) when its root is absent
    (FINAL124_GRID1, 2026-10-01)."""
    if not schema:
        return 104
    for wire in ("104", "205"):
        entry = schema.get("services", {}).get(wire)
        response = schema.get(entry[1]) if entry else None
        field = next((f for f in response["fields"] if f["off"] == 8), None) if response else None
        if field is not None and field["kind"] == 1 and field["nested"] == UNIVERSE_TYPE:
            return int(wire)
    return None


def ws_reply_values(args, wire):
    """Field values for a default service reply: the universe object for the universe service;
    nothing for 01.24's 104, whose +8 is a bool rather than the 01.00 universe."""
    if wire == universe_service(args.schema):
        return universe_reply(args, args.schema)
    return None if wire == 104 else WS_REPLY_VALUES.get(wire)


def universe_reply(args, schema=None):
    """The universe object for the service universe_service() names."""
    universe = {0: UNIVERSE_SOID, 8: universe_time(args.universe_time)}
    if args.universe_flags:
        universe[0x10] = {0: len(args.universe_flags), 4: list(args.universe_flags)}
    if args.universe_values:
        universe[0x3C] = {0: len(args.universe_values),
                          4: [{0: index, 2: value} for index, value in args.universe_values]}
    return {8: universe}


def universe_time(unit, now=None):
    import time as _time
    return UNIVERSE_TIME_UNITS[unit](_time.time() if now is None else now)


def client_config_reply(value=1, version=1):
    """client config rsp (0x13): nanopb message at 0x11d0bf0 - required 1 uint64, required
    2 uint32, 3 a fixed64 callback field (omitted). Values are local placeholders."""
    return bytes([0x08]) + signon_proto.varint(value) + bytes([0x10]) + signon_proto.varint(version)


def acct_id_trans_reply(request, account_ids):
    """Reply to "acct id trans" (0x17): the request echoed - u16 count, u8 7, u8 0xff and
    `count` name entries (decoder 0x415764 requires the same header and reads the entries
    with 0x113090) - followed by one u64 account id per entry (64 bits each, MSB first)."""
    count = struct.unpack_from(">H", request)[0]
    if request[2:4] != b"\x07\xff" or len(account_ids) != count:
        raise ValueError("unexpected acct id trans request")
    return request + b"".join(struct.pack(">Q", a) for a in account_ids)
UNIVERSE_SOID = 0x7FFFFFFFFFFFFFFF
UNIVERSE_TYPE = 0x80801941
FAMILY_TYPES = {"peer": 0, "inspection": 1, "roster": 2, "select_character": 3, "self": 4,
                "universe": 5, "party_details": 6, "party_network_details": 7}  # 0xb6068c


class BitWriter:
    """MSB-first bit stream, as written by the engine's type serializer (0x25e080)."""

    def __init__(self):
        self.bits = []

    def put(self, value, width):
        self.bits += [(value >> (width - 1 - i)) & 1 for i in range(width)]

    def data(self):
        bits = self.bits + [0] * (-len(self.bits) % 8)
        return bytes(int("".join(map(str, bits[i:i + 8])), 2) for i in range(0, len(bits), 8))


def signin_response(account_id, result=0, word1=1, word2=1, optional_ids=True, opt_values=None,
                    universe_flags=(), universe_values=(), schema=None):
    """Service 503 (investment sign-in) response, type 0x80801825:
    {u4 result, i32 detail} u64 +0x08, u32 +0x10, u32 +0x14, then struct 0x80801941 of six
    optional fields (presence bit each): u64, u64, three counted lists, u32. Lists and the
    last u32 are left absent here; the trailing bit is the ws blob flag (0).

    Primitive kinds are biased on the wire (seen in decoded memory): the kind-3 result enum
    decodes as bits - 1 and the signed kind-5 detail as bits - 0x80000000. The completion
    handler 0x295794 (via 0xbb8a40) needs result 0 and takes the user's root soid from +0x08;
    it then moves the local user entry from state 2 to 3. With a schema, the
    response uses its current field widths and optional-field count."""
    if schema:
        universe = {}
        if optional_ids:
            values = opt_values or (UNIVERSE_SOID, 1)
            universe.update({0: values[0], 8: values[1]})
        if universe_flags:
            universe[0x10] = {0: len(universe_flags), 4: list(universe_flags)}
        if universe_values:
            universe[0x3C] = {0: len(universe_values),
                              4: [{0: index, 2: value} for index, value in universe_values]}
        return default_ws_response(schema, 503, {0: {0: result, 4: 0}, 8: account_id,
                                                 16: word1, 20: word2, 24: universe})
    w = BitWriter()
    w.put(0x01F7, 16)
    w.put(result + 1, 4); w.put(0x80000000, 32)
    w.put(account_id, 64); w.put(word1, 32); w.put(word2, 32)
    # The +0x18 struct is the universe object (0x41becc): its first u64 must be the
    # universe root soid the client subscribed to (0x7fffffffffffffff, rodata 0x11f4590).
    for value in (opt_values or (UNIVERSE_SOID, 1)):
        w.put(1 if optional_ids else 0, 1)
        if optional_ids:
            w.put(value, 64)
    # +0x10: optional list 0x808045f0 = u5 count + up to 20 i16 (biased by 0x8000).
    w.put(1 if universe_flags else 0, 1)
    if universe_flags:
        w.put(len(universe_flags), 5)
        for flag in universe_flags:
            w.put((flag + 0x8000) & 0xFFFF, 16)
    # +0x3c: optional list 0x808046e7 = u5 count + up to 20 0x8080185c {opt i16 flag index,
    # opt u2 tri-state value (bits = value + 1; 2 = true, 1 = false)}: universe flag overrides.
    w.put(1 if universe_values else 0, 1)
    if universe_values:
        w.put(len(universe_values), 5)
        for index, value in universe_values:
            w.put(1, 1); w.put((index + 0x8000) & 0xFFFF, 16)
            w.put(1, 1); w.put(value + 1, 2)
    for _ in range(2):
        w.put(0, 1)
    w.put(0, 1)
    return w.data()


def read_bits(data, start, count):
    """Unsigned MSB-first value of `count` bits at bit offset `start`."""
    value = 0
    for i in range(start, start + count):
        value = (value << 1) | ((data[i >> 3] >> (7 - (i & 7))) & 1)
    return value


def family_fetch_request(payload):
    """Service 206 request (type 0x80801774): u4 family type (biased +1), u64 root soid."""
    return read_bits(payload, 16, 4) - 1, read_bits(payload, 20, 64)


def family_fetch_response(family_type, root, version=1, blob=None):
    """Service 206 response (type 0x80801869, just {u4 result}) plus the ws blob that the
    client posts to its queuez handler (0xbb87b8 -> 0x51c7ec): a qz update payload holding
    the requested family, here with no objects."""
    blob = blob if blob is not None else qz_update([(family_type, root, version, 0, [])])
    w = BitWriter()
    w.put(206, 16)
    w.put(0 + 1, 4)          # result error_none, biased
    w.put(1, 1)              # blob present
    w.put(len(blob), 16)
    for byte in blob:
        w.put(byte, 8)
    return w.data()


def qz_update(families):
    """qz->c update payload (decoder 0x4198d8): u32 count, then per family
    u32 type, u64 root soid, u32 version, u8 flags, u32 object count, objects.
    Objects are (16-byte header, payload); the header's u32 at +0xc is the payload length."""
    out = struct.pack(">I", len(families))
    for family_type, root, version, flags, objects in families:
        out += struct.pack(">IQIBI", family_type, root, version, flags, len(objects))
        for header, payload in objects:
            out += header[:12] + struct.pack(">I", len(payload)) + payload
    return out
# The client's reply queue (0x7c6bc0) expects exactly four reply types: shello rsp, echo
# rsp, qz_reg rsp and rrc rsp. The last three decode with 0xa62a0, which ignores the
# payload, so an empty status-200 reply satisfies them. Request -> reply type.
UNSUB_REQ, UNSUB_RSP = 0x0E, 0x0F
UPO_REQ, UPO_RSP = 0x15, 0x16
USER_MSG_REQ, USER_MSG_RSP = 0x20, 0x21
# 01.24 sends user-msg then UPO when profile setup completes. Their response
# codecs (0x47bf9c / 0x477ad8) decode protobuf. UPO has only repeated result
# entries, so an empty body is valid. User-msg has optional strings plus REQUIRED
# integer field 4; omitting it failed decoding in FINAL124_PROFILE2 (weasel).
# Answer both in order before later WS replies.
# 01.24 also sends "c->b mm req" (0x2a, protobuf) after the prologue, before subscribing to the
# peer and roster families. Its reply "mm rsp" (0x2b, decoder 0x477d9c, descriptor 0x01306900)
# has only optional fields (1 int32, 2 int64, 3-6 strings), so an empty body decodes. Left
# unanswered, the client closed with weasel right after the roster push (FINAL124_UNIVERSE2).
MM_REQ, MM_RSP = 0x2A, 0x2B
# The poll (request field 2 = 4) is consumed by 0x490a30 (manager 0x5b5358, kind 4): with field 4
# present it copies that string (<= 0x418 bytes) to the poll object +0x34, sets +0x30 ("match
# found", status 2 from 0x490b08) and stops polling; without it the poll repeats (state 5).
# A non-submessage field 4 fails decoding: weasel (FINAL124_MM2).


def mm_rsp_body(spec):
    """mm rsp from "N=VALUE,..." - fields 1-2 are integers, 3-6 submessages given as hex (empty
    for a present but empty submessage). In this nanopb build type 6 is a submessage: the
    descriptor (0x01306900) nests 3 = {1: repeated {1: {84-byte blob}, 2: char[32], 3, 5:
    fixed32}, 2: int}, 4 = {1: 11 ints, 2: {1: 7 ints, 2: repeated 7 ints}}, 5 = {1: u64},
    6 = {1: char[32], 2: 8 ints}. Commas separate fields (-BapExtra splits on semicolons)."""
    out = b""
    for part in filter(None, spec.split(",")):
        number, _, value = part.partition("=")
        number = int(number)
        if number in (1, 2):
            out += signon_proto.field(number, signon_proto.VARINT, int(value, 0) & (1 << 64) - 1)
        elif 3 <= number <= 6:
            out += signon_proto.field(number, signon_proto.BYTES, bytes.fromhex(value))
        else:
            raise ValueError(f"mm rsp has no field {number}")
    return out
EMPTY_REPLIES = {QZ_REG_REQ: QZ_REG_RSP, ECHO_REQ: ECHO_RSP, RRC_REQ: RRC_RSP, SUB_REQ: SUB_RSP,
                 UNSUB_REQ: UNSUB_RSP, UPO_REQ: UPO_RSP, MM_REQ: MM_RSP}
PROTO_REPLIES = {USER_MSG_REQ: (USER_MSG_RSP, signon_proto.field(4, signon_proto.VARINT, 0))}
NAMES = {0x03: "goodbye not", 0x06: "c->ah mgr req", 0x07: "c->ah mgr rsp", 0x08: "c->ah not",
         0x09: "b->c not", 0x0A: "c->ws req", 0x0B: "c->ws rsp", 0x0C: "sub req", 0x0D: "sub rsp",
         0x0E: "unsubscribe req", 0x0F: "unsubscribe rsp", 0x10: "ahp req", 0x11: "ahp rsp",
         0x12: "client config req", 0x13: "client config rsp", 0x14: "shutdown not",
         0x15: "upo req", 0x16: "upo rsp", 0x17: "acct id trans req", 0x18: "acct id trans rsp",
         SHELLO_REQ: "shello req", SHELLO_RSP: "shello rsp", START_REQ: "start req",
         START_RSP: "start rsp", QZ_REG_REQ: "qz_reg req",
         USER_MSG_REQ: "user msg req", USER_MSG_RSP: "user msg rsp", MM_REQ: "mm req", MM_RSP: "mm rsp",
         QZ_REG_RSP: "qz_reg rsp", 0x7B: "qz->c update not", 0x7C: "qz->c sub lost not",
         ECHO_REQ: "echo req", ECHO_RSP: "echo rsp", 0x12C: "c->b irc not", 0x12D: "b->c rrc not",
         RRC_REQ: "rrc req", RRC_RSP: "rrc rsp"}


def seal(key, mac_key, plaintext, iv):
    pad = 16 - len(plaintext) % 16
    padded = plaintext + bytes([pad]) * pad
    encryptor = Cipher(algorithms.AES(key), modes.CBC(iv)).encryptor()
    body = iv + encryptor.update(padded) + encryptor.finalize()
    head = struct.pack(">I", len(body) + 32)
    return head + body + hmac.new(mac_key, head + body, hashlib.sha256).digest()


def unseal(key, mac_key, blob):
    (length,) = struct.unpack_from(">I", blob)
    if length + 4 != len(blob) or length < 0x30:
        raise ValueError("bad sealed length")
    mac = blob[-32:]
    if not hmac.compare_digest(mac, hmac.new(mac_key, blob[:-32], hashlib.sha256).digest()):
        raise ValueError("MAC mismatch")
    decryptor = Cipher(algorithms.AES(key), modes.CBC(blob[4:20])).decryptor()
    padded = decryptor.update(blob[20:-32]) + decryptor.finalize()
    pad = padded[-1]
    if not 1 <= pad <= 16 or padded[-pad:] != bytes([pad]) * pad:
        raise ValueError("bad padding")
    return padded[:-pad]


def shello_rsp_body(key, mac_key, nonce, session_key, iv):
    if len(nonce) != 12 or len(session_key) != 16:
        raise ValueError("nonce is 12 bytes, session key 16")
    blob = seal(key, mac_key, nonce + session_key, iv)
    if len(blob) - 4 > 0x5C:
        raise ValueError("sealed shello reply exceeds the client's 0x5c limit")
    return blob


class GcmChannel:
    """One direction of the post-shello encrypted stream."""

    def __init__(self, key, nonce):
        self.aead, self.nonce = AESGCM(key), bytearray(nonce)

    def _advance(self):
        for i in range(12):
            self.nonce[i] = (self.nonce[i] + 1) & 0xFF
            if self.nonce[i]:
                break

    def open(self, body):
        plain = self.aead.decrypt(bytes(self.nonce), body[16:] + body[:16], None)
        self._advance()
        return plain

    def seal(self, plain):
        out = self.aead.encrypt(bytes(self.nonce), plain, None)
        self._advance()
        return out[-16:] + out[:-16]


def channels(nonce, session_key):
    """(server receive, server send) channels for a session."""
    client_send = nonce[:11] + bytes([nonce[11] ^ 1])
    return GcmChannel(session_key, client_send), GcmChannel(session_key, nonce)


def encrypted_frame(channel, msg_type, msg_id, status, payload):
    body = channel.seal(struct.pack(">HIH", msg_type, msg_id, status) + payload)
    return struct.pack(">BBI", 1, 1, len(body)) + body


def encrypted_frame_kind(channel, msg_type, msg_id, payload):
    """Server push with a 6-byte header (header kinds 1 and 4: type, id; no status)."""
    body = channel.seal(struct.pack(">HI", msg_type, msg_id) + payload)
    return struct.pack(">BBI", 1, 1, len(body)) + body


def plain_frame(msg_type, msg_id, status, body):
    inner = struct.pack(">HIH", msg_type, msg_id, status) + body
    return struct.pack(">BBI", 1, 2, len(inner)) + inner


def exact(conn, count):
    data = bytearray()
    while len(data) < count:
        chunk = conn.recv(count - len(data))
        if not chunk:
            raise EOFError(f"closed with {len(data)}/{count} bytes")
        data.extend(chunk)
    return bytes(data)


# Structured run log: each connection writes <capture folder>/events.jsonl, one JSON object per
# line - every log line ({"t", "log"}), every client frame ({"ev": "frame"}) and every client
# activity-host message ({"ev": "ah"}), with full hex bodies. `server/run_log.py` reads it.
_events = threading.local()


def record_event(**fields):
    sink = getattr(_events, "sink", None)
    if sink is not None:
        fields = {"t": datetime.datetime.now().isoformat(timespec="milliseconds"), **fields}
        sink.write(json.dumps(fields) + "\n")


def log(message):
    print(f"{datetime.datetime.now().isoformat(timespec='milliseconds')} {message}", flush=True)
    record_event(log=message)


def handle(conn, peer, args):
    tag = datetime.datetime.now().strftime("%Y%m%d-%H%M%S-%f")
    folder = args.capture_dir / tag
    folder.mkdir(parents=True)
    _events.sink = open(folder / "events.jsonl", "w", encoding="utf-8", buffering=1)
    conn.settimeout(args.timeout)
    log(f"[{tag}] connection from {peer[0]}:{peer[1]}")
    rx = tx = None
    account_id = character_id = character_profile = active_id = account_data = None
    # one database handle per connection (thread); state is scoped by account/character ids
    player_db = PlayerDB(args.player_db) if getattr(args, "player_db", None) else None
    self_version = 1
    heartbeat_at = None  # next activity-host heartbeat push (--ah-heartbeat), set by the join
    sim_repeat = None  # simulation packet resent with each heartbeat (--ah-roster)
    sim_build = None  # (peer, slot, generation, objects, game-mode position, join time) for rebuilds
    baselines_stamp = None  # modification time of --ah-baselines-watch when last read
    objects_applied = 0  # --ah-object-at entries already sent (they are sorted by time)
    reactions_fired = set()  # --ah-on rules already triggered (each fires once)
    # Mission clock (per connection = per activity instance until instances are shared: debt in
    # notes/architecture-multiplayer.md); with "ready" the timed steps wait for the client
    client_ready_at = None
    incident_echoed = False
    quiet_notifications = 0

    def mission_t0(world_anchor):
        return world_anchor if args.mission_clock == "world" else client_ready_at
    reactions_due = ReactionQueue()  # phase-scoped changes from --ah-on and live control
    control_offset = args.ah_control.stat().st_size if args.ah_control and args.ah_control.exists() else 0
    game_mode_now = None  # game-mode state currently replicated (--ah-game-mode-schedule)
    world_params = None  # (join request, peer byte, group peer) of the --ah-world-state delta
    zone_now = zone_joined = None  # zone-set index last sent in the world state, and when joined
    switch_target = None  # --ah-group-switch: zone set published in group record 1
    # World-state version counter of the activity instance this connection hosts. The stand-in
    # hosts one instance per activity-host connection; a multi-client host will own one counter
    # per world instance and share it between that instance's peers.
    world_version = itertools.count(1)
    world_version_now = 0  # last version sent (heartbeat resends reuse it)
    teleport_now = None  # --ah-teleport: (state, index, hash) in the world-state group request
    teleport_targeted = False  # the client has requested the teleport's slice set
    teleport_finish_at = None  # when to send the finishing state 3 (--ah-teleport-finish-delay)
    teleport_finished_at = None  # when state 3 was first published
    teleport_drop_at = None  # when to stop publishing the finished request (TELEPORT_SETTLE_SECONDS)
    # timed host teleports of this activity session: [(slice set, seconds after the join)] from
    # --ah-teleport and the mission script's {"at": S, "teleport": SET} steps, in order
    teleport_plan = sorted(args.ah_teleports, key=lambda entry: entry[1])
    slice_hashes = {}  # slice-set index -> name hash, as the client reported it
    client_slice_fields = {}  # type 21 carries deltas; retain explicit fields for this connection
    confirm_repeat = None  # boot list resent with each heartbeat (--ah-boot)
    bubble_resets = tuple(getattr(args, "ah_bubble_reset", ()))  # generations belong to this connection

    def make_sim_packet(peer, slot, generation, **values):
        return sim_packet(args.activity_schema, peer, slot, generation,
                          bubble_resets=bubble_resets, **values)

    def host_tick():
        # --ah-sim-clock: host simulation ticks since the first simulation packet (0 without it)
        return int((time.time() - sim_build[5]) * args.ah_sim_clock) if sim_build and args.ah_sim_clock else 0

    def mission_context():
        return {"player": args.ah_sim_player[2] if args.ah_sim_player else 0, "now": host_tick()}

    def push_self(number, reason):
        # Delta for the live self subscription: versions must advance by one (0x41b75c).
        nonlocal self_version
        if account_id is None:
            return
        self_version += 1
        push = encrypted_frame_kind(tx, QZ_UPDATE, 0, family_blob(
            args.schema, 4, account_id, character_id, character_profile,
            version=self_version, flags=0, active_id=active_id, self_flags=args.self_flags,
            extras=args.character_extras, account_data=account_data))
        conn.sendall(push)
        folder.joinpath(f"{number:03}-server-self-{reason}.bin").write_bytes(push)
        log(f"[{tag}] pushed self v{self_version} ({reason}): character "
            f"{character_id:#x}, active {active_id:#x}" if active_id else
            f"[{tag}] pushed self v{self_version} ({reason}), no active character")
    try:
        for number in itertools.count(1):
            head = exact(conn, 6)
            version, kind, length = struct.unpack(">BBI", head)
            if version != 1 or length > 1 << 20:
                raise ValueError(f"unexpected outer header {head.hex()}")
            body = exact(conn, length)
            folder.joinpath(f"{number:03}-client-kind{kind}.bin").write_bytes(head + body)
            if kind == 1 and rx is not None:
                body = rx.open(body)
                folder.joinpath(f"{number:03}-client-plain.bin").write_bytes(body)
            elif kind != 2 or length < 6:
                log(f"[{tag}] frame {number}: kind {kind}, {length} bytes (captured)")
                continue
            msg_type, msg_id = struct.unpack_from(">HI", body)
            payload = body[6:]
            record_event(ev="frame", n=number, kind=kind, type=msg_type, name=NAMES.get(msg_type, hex(msg_type)),
                   id=msg_id, payload=payload.hex())
            quiet_frame = (kind == 1 and msg_type == AH_NOT
                           and not getattr(args, "verbose_wire", False)
                           and payload[8:] == ROUTINE_TYPE15_PAYLOAD)
            if quiet_frame:
                quiet_notifications += 1
            else:
                log(f"[{tag}] frame {number} (kind {kind}): {NAMES.get(msg_type, hex(msg_type))} "
                    f"id={msg_id} payload={len(payload)} bytes {payload[:48].hex()}")
            if kind == 1 and msg_type == WS_REQ and payload[:2] == bytes.fromhex("01f7") and args.ws_signin:
                account_id = int.from_bytes(payload[38:46], "big")
                if args.ws_markers:
                    body = signin_response(0x3333333333333333, word1=0x44444444, word2=0x55555555,
                                           opt_values=(0x1111111111111111, 0x2222222222222222),
                                           schema=args.schema)
                else:
                    body = signin_response(account_id, optional_ids=not args.ws_no_optional,
                                           opt_values=(UNIVERSE_SOID, universe_time(args.universe_time)),
                                           universe_flags=args.universe_flags,
                                           universe_values=args.universe_values, schema=args.schema)
                reply = encrypted_frame(tx, WS_RSP, msg_id, 200, body)
                conn.sendall(reply)
                folder.joinpath(f"{number:03}-server-ws-rsp.bin").write_bytes(reply)
                log(f"[{tag}] sent sign-in rsp (account id {account_id:#x})")
                saved = load_store(args.store).get(f"{account_id:#x}") if args.store else None
                if player_db is not None:
                    # Durable store: the account's characters by slot (multiplayer-safe ids);
                    # the stand-in still presents one profile, the first character.
                    player_db.ensure_account(account_id, f"{account_id:#x}")
                    owned = player_db.characters(account_id)
                    stored = player_db.account(account_id)
                    saved = {}
                    if owned:
                        saved.update(id=owned[0]["character_id"], request=owned[0]["create_request"].hex())
                    if stored and stored["account_data"]:
                        saved["account_data"] = stored["account_data"].hex()
                if saved and args.schema and "id" in saved:
                    character_id = saved["id"]
                    character_profile = character_from_request(args.schema, character_id,
                                                               bytes.fromhex(saved["request"]))
                    log(f"[{tag}] restored stored character {character_id:#x}")
                if saved and args.schema and "account_data" in saved and account_data_offset(args.schema):
                    account_data = account_data_values(args.schema, bytes.fromhex(saved["account_data"]))
                    log(f"[{tag}] restored stored account user data (701)")
            elif kind == 1 and msg_type == WS_REQ and payload[:2] == bytes.fromhex("01f5") and args.ws_signin:
                # Service 501 creates a character; its response carries the new character soid.
                # Keep the id stable for this local account during this connection.
                character_id = (account_id or 1) + 1
                if player_db is not None and account_id is not None:
                    # database-allocated, globally unique; a resent request (same connection and
                    # message id) returns the same character instead of creating another
                    character_id = player_db.create_character(account_id, payload[2:],
                                                              f"501:{tag}:{msg_id}")
                character_profile = character_from_request(args.schema, character_id, payload[2:])
                if args.store and account_id is not None and player_db is None:
                    save_character(args.store, account_id, character_id, payload[2:])
                reply = encrypted_frame(tx, WS_RSP, msg_id, 200,
                                        default_ws_response(args.schema, 501, {8: character_id}))
                conn.sendall(reply)
                folder.joinpath(f"{number:03}-server-ws-501.bin").write_bytes(reply)
                log(f"[{tag}] sent create-character rsp (id {character_id:#x})")
                # Creation changes the subscribed self profile: publish the new character
                # and make it the active one (the client does not send 504 after 501).
                active_id = character_id
                push_self(number, "after-501")
            elif kind == 1 and msg_type == WS_REQ and payload[:2] == bytes.fromhex("01f8") and args.ws_signin:
                # Service 504 selects a character: u64 character id, then a flag byte.
                # Reply error_none and publish it as the self root's active character.
                active_id = int.from_bytes(payload[2:10], "big")
                reply = encrypted_frame(tx, WS_RSP, msg_id, 200, default_ws_response(args.schema, 504))
                conn.sendall(reply)
                folder.joinpath(f"{number:03}-server-ws-504.bin").write_bytes(reply)
                log(f"[{tag}] sent select-character rsp (id {active_id:#x}, flag {payload[10:11].hex()})")
                push_self(number, "after-504")
            elif kind == 1 and msg_type == WS_REQ and payload[:2] == bytes.fromhex("00ce") and args.ws_signin:
                family_type, root = family_fetch_request(payload)
                reply = encrypted_frame(tx, WS_RSP, msg_id, 200, family_fetch_response(
                    family_type, root, blob=family_blob(args.schema, family_type, root, character_id,
                                                       character_profile, extras=args.character_extras)))
                conn.sendall(reply)
                folder.joinpath(f"{number:03}-server-ws-206.bin").write_bytes(reply)
                log(f"[{tag}] sent family fetch rsp: type {family_type} root {root:#x}, "
                    f"character {character_id:#x}" if character_id is not None else
                    f"[{tag}] sent family fetch rsp: type {family_type} root {root:#x}, no character")
            elif (kind == 1 and msg_type == WS_REQ and len(payload) >= 2 and args.ws_signin
                  and int.from_bytes(payload[:2], "big") in args.ws_no_reply):
                # Experiment switch: leaving 701 unanswered shows "Contacting Destiny Servers"
                # and fails sooner (comparison 059), so replies are the default.
                log(f"[{tag}] no reply for service {int.from_bytes(payload[:2], 'big')} (one-way)")
            elif (kind == 1 and msg_type == WS_REQ and len(payload) >= 2 and args.ws_signin
                  and default_ws_response(args.schema, int.from_bytes(payload[:2], "big"))):
                wire = int.from_bytes(payload[:2], "big")
                reply = encrypted_frame(tx, WS_RSP, msg_id, 200, default_ws_response(
                    args.schema, wire, ws_reply_values(args, wire)))
                conn.sendall(reply)
                folder.joinpath(f"{number:03}-server-ws-{wire}.bin").write_bytes(reply)
                log(f"[{tag}] sent default reply for service {wire} (result error_none)")
                if (wire == 701 and player_db is not None and account_id is not None
                        and account_data_offset(args.schema)):
                    player_db.set_account_data(account_id, payload[2:])
                    account_data = account_data_values(args.schema, payload[2:])
                elif wire == 701 and args.store and account_id is not None and account_data_offset(args.schema):
                    # Keep the latest account user data (settings) for the next sign-in.
                    if payload[2:].hex() != (load_store(args.store).get(f"{account_id:#x}") or {}).get("account_data"):
                        save_account_data(args.store, account_id, payload[2:])
                        log(f"[{tag}] stored account user data from service 701")
                    account_data = account_data_values(args.schema, payload[2:])
            elif kind == 1 and msg_type == WS_REQ and len(payload) >= 2 and args.ws_zero_bytes:
                body = bytearray(args.ws_zero_bytes)
                body[0] = (args.ws_result & 0xF) << 4  # u4 result code, MSB-first (0 = error_none)
                reply = encrypted_frame(tx, WS_RSP, msg_id, 200, payload[:2] + bytes(body))
                conn.sendall(reply)
                folder.joinpath(f"{number:03}-server-ws-rsp.bin").write_bytes(reply)
                log(f"[{tag}] sent c->ws rsp for service {int.from_bytes(payload[:2], 'big')} "
                    f"(result {args.ws_result}, {args.ws_zero_bytes} bytes)")
                if args.qz_push:
                    families = []
                    for spec in args.qz_push:
                        name, version, flags = (spec.split(":") + ["1", "0"])[:3]
                        families.append((FAMILY_TYPES[name], args.qz_root, int(version, 0), int(flags, 0), []))
                    push = encrypted_frame_kind(tx, QZ_UPDATE, 0, qz_update(families))
                    conn.sendall(push)
                    folder.joinpath(f"{number:03}-server-qz-update.bin").write_bytes(push)
                    log(f"[{tag}] pushed qz update {args.qz_push} root={args.qz_root:#x}")
            if kind == 1 and msg_type == ACCT_ID_TRANS_REQ and args.ws_signin:
                # Replies must answer the oldest pending request first (0x7c3458); leaving
                # this unanswered made the next ws replies unmatched: BAP failure, centipede.
                # Entries are 36-byte names; the local test has one account.
                names = [payload[4 + 36 * i:40 + 36 * i].split(b"\0")[0].decode("latin1")
                         for i in range(struct.unpack_from(">H", payload)[0])]
                body = acct_id_trans_reply(payload, [account_id or 0] * len(names))
                reply = encrypted_frame(tx, ACCT_ID_TRANS_RSP, msg_id, 200, body)
                conn.sendall(reply)
                folder.joinpath(f"{number:03}-server-18.bin").write_bytes(reply)
                log(f"[{tag}] sent acct id trans rsp {names} -> {account_id or 0:#x}")
            if kind == 1 and msg_type == AH_NOT and len(payload) >= 21:
                host_id, _flag, ah_type, ah_len = struct.unpack_from(">QBII", payload)
                body = payload[21:21 + ah_len]
                record_event(ev="ah", n=number, host=f"{host_id:#x}", ah_type=ah_type, body=body.hex())
                if not quiet_frame:
                    log(f"[{tag}] c->ah not host {host_id:#x} type {ah_type} len {ah_len} body {body.hex()}")
                if ah_type == AH_CLIENT_PROGRESS and client_ready_at is None and client_in_world(body):
                    client_ready_at = time.time()
                    log(f"[{tag}] client in world (type 17 {CLIENT_IN_WORLD:#x})"
                        + ("; mission clock started" if args.mission_clock == "ready" else ""))
                if ah_type == AH_INCIDENT and args.ah_echo_incidents and tx is not None:
                    # EXPERIMENT (JOIN83): the client re-sends an incident (e.g. "player entered
                    # trigger") about 4/s and leaves with pumpkin after ~100 s (JOIN81/82); hosts
                    # may send incidents too (handler 0xe2e9ec), so try broadcasting it back.
                    conn.sendall(encrypted_frame_kind(tx, BC_NOT, 0, bc_notification(AH_INCIDENT, body, host_id)))
                    if not incident_echoed:
                        log(f"[{tag}] incident echoed to the client (type {AH_INCIDENT})")
                    incident_echoed = True
                if ah_type == AH_CLIENT_STATE:
                    try:
                        record = client_slice_record(args.activity_schema, body)
                    except ValueError:
                        record = None
                    if record:
                        log(f"[{tag}] client slice set: {client_slice_state(args.activity_schema, body)}")
                        client_slice_fields.update(record)
                        for name in ("target", "current"):
                            index, name_hash = record.get(name, (None, None))
                            if index is not None and index >= 0 and name_hash not in (None, 0, 0x811C9DC5):
                                slice_hashes[index] = name_hash  # named sets need it in teleports
                        # Mirror the client's switch in the world state's group record, as the
                        # host that tracks the fireteam's slice set would (opt-in experiment)
                        wanted = requested_client_zone(record)
                        # Arrival needs a current field from this received delta. A retained
                        # current can describe the area before the host request (JOIN92:
                        # cached 24, then target-only 24, then current -1 during the load).
                        # Keep the target cache for fresh state-4 arrival deltas (JOIN88).
                        current = requested_client_zone({"current": record.get("current", (None, None))})
                        routing_current = requested_client_zone({
                            "current": client_slice_fields.get("current", (None, None))})
                        if (args.ah_group_switch and world_params and "peer_byte" in record
                                and record["peer_byte"] != world_params[1]):
                            world_params = (world_params[0], record["peer_byte"], *world_params[2:])
                            data = world_state_delta(args.activity_schema, world_params[0], zone_now,
                                                     *world_params[1:], target=switch_target, teleport=teleport_now, version=(world_version_now := next(world_version)))
                            conn.sendall(encrypted_frame_kind(tx, BC_NOT, 0,
                                                              bc_notification(AH_ESTABLISH, data, host_id)))
                            log(f"[{tag}] world state: per-peer byte -> {world_params[1]} (client slice record)")
                        if teleport_now is not None and teleport_now[0] == 1:
                            # A state-only delta still refers to the last target, which can have
                            # been sent before a live teleport was queued (JOIN88). An explicit
                            # clear/change must invalidate it rather than leave a latched match.
                            teleport_targeted = requested_client_zone({
                                "target": client_slice_fields.get("target", (None, None))}) == teleport_now[1]
                            if not teleport_targeted:
                                teleport_finish_at = None
                        if (teleport_now is not None and teleport_now[0] == 1 and teleport_finish_at is None
                                and teleport_targeted and record.get("state") == 4):
                            # finish after the configured settle time (sent from the timer below)
                            teleport_finish_at = time.time() + args.ah_teleport_finish_delay
                        if (teleport_now is not None and teleport_now[0] == 1 and world_params
                                and (current == teleport_now[1]
                                     or (teleport_finish_at is not None and time.time() >= teleport_finish_at))):
                            # The client reports the arrival (record state 4) but only reports the
                            # set as current after the request completes, which needs state 3
                            # (JOIN42: copy waiting in state 3 with the engine already in 112).
                            teleport_now = (3, teleport_now[1], teleport_now[2])
                            teleport_finished_at = time.time()
                            # Completion must expire even without a group-record switch (e.g.
                            # follow mode, or a teleport to the already-current slice set).
                            teleport_drop_at = teleport_finished_at + TELEPORT_SETTLE_SECONDS
                            # same version: the simulation copies the world state (and so the
                            # request) on every message, but the world controller re-reads the group
                            # records only on a new version - re-reading record 1 (the switch to
                            # the set the engine is already in) restarted the switch phases and
                            # left the screen black (JOIN43-50)
                            data = world_state_delta(args.activity_schema, world_params[0], zone_now,
                                                     *world_params[1:], target=switch_target, teleport=teleport_now,
                                                     version=world_version_now)
                            conn.sendall(encrypted_frame_kind(tx, BC_NOT, 0,
                                                              bc_notification(AH_ESTABLISH, data, host_id)))
                            log(f"[{tag}] world state: teleport to {teleport_now[1]} reached (state 3)")
                        if args.ah_group_switch and world_params:
                            # The host side of a slice-set switch: publish the requested set in
                            # group record 1 while record 0 keeps the current one; once the client
                            # reports the target as current, move record 0 and clear record 1.
                            if (switch_target is not None and current == switch_target):
                                zone_now, switch_target = switch_target, None
                                if teleport_now is not None and teleport_now[1] == zone_now:
                                    # the teleport is complete: stop publishing the request (JOIN39,
                                    # the only run that showed 112 after a hand-off, never had it in
                                    # the host's world state once the client had arrived) - but not
                                    # before state 3 has been published for a while: its handling
                                    # respawns the player (0x72932c), and a fast arrival (JOIN61:
                                    # state 3 and the drop 50 ms apart) left the screen black
                                    if (teleport_finished_at is not None
                                            and time.time() - teleport_finished_at >= TELEPORT_SETTLE_SECONDS):
                                        teleport_now = None
                                    else:
                                        teleport_now = (3, teleport_now[1], teleport_now[2])
                                        teleport_finished_at = teleport_finished_at or time.time()
                                        teleport_drop_at = teleport_finished_at + TELEPORT_SETTLE_SECONDS
                                data = world_state_delta(args.activity_schema, world_params[0], zone_now,
                                                         *world_params[1:], teleport=teleport_now, version=(world_version_now := next(world_version)))
                                conn.sendall(encrypted_frame_kind(tx, BC_NOT, 0,
                                                                  bc_notification(AH_ESTABLISH, data, host_id)))
                                log(f"[{tag}] world state: zone set {zone_now} reached, switch cleared")
                            elif (wanted is not None and wanted >= 0 and wanted != zone_now
                                  and wanted != switch_target and routing_current != wanted):
                                switch_target = wanted
                                data = world_state_delta(args.activity_schema, world_params[0], zone_now,
                                                         *world_params[1:], target=switch_target, teleport=teleport_now, version=(world_version_now := next(world_version)))
                                conn.sendall(encrypted_frame_kind(tx, BC_NOT, 0,
                                                                  bc_notification(AH_ESTABLISH, data, host_id)))
                                log(f"[{tag}] world state: switch {zone_now} -> {switch_target} (group record 1)")
                        elif (args.ah_follow_client_zone and world_params and wanted is not None
                                and wanted >= 0 and wanted != zone_now):
                            zone_now = wanted
                            data = world_state_delta(args.activity_schema, world_params[0], zone_now,
                                                     *world_params[1:], teleport=teleport_now, version=(world_version_now := next(world_version)))
                            conn.sendall(encrypted_frame_kind(tx, BC_NOT, 0,
                                                              bc_notification(AH_ESTABLISH, data, host_id)))
                            log(f"[{tag}] world state: zone set -> {zone_now} (following the client)")
                for rule_index, (on_type, on_body, delay, spec) in enumerate(args.ah_on):
                    # Script steps driven by what the client reports (sense state), not by time
                    if (rule_index not in reactions_fired and on_type in (ah_type, ANY_MESSAGE)
                            and (on_body is None or on_body.search(body.hex()))):
                        reactions_fired.add(rule_index)
                        cancelled = reactions_due.schedule(
                            time.time() + delay, spec, f"type {on_type} rule {rule_index}",
                            getattr(args.ah_on[rule_index], "scope", None))
                        if cancelled:
                            log(f"[{tag}] --ah-on rule {rule_index}: cancelled {cancelled} obsolete phase changes")
                        action = (f"teleport {spec.zone[0]}" if isinstance(spec, TeleportReaction) else
                                  f"object {spec[0][0]:#x}:{spec[0][4]:#x}:{spec[0][6]}")
                        log(f"[{tag}] --ah-on rule {rule_index} matched type {ah_type}: "
                            f"{action} in {delay:g} s")
            if (kind == 1 and msg_type == AH_NOT and len(payload) >= 21 and ah_type == AH_JOIN_REQUEST
                    and args.ah_join and args.activity_schema):
                request = join_request(args.activity_schema, body)
                log(f"[{tag}] host join request {request.get(0, 0):#x} / {request.get(8, 0):#x} "
                    f"ids {request.get(0x10)}")
                data = join_response(args.activity_schema, body, args.ah_session, extra=dict(args.ah_join_fields))
                push = encrypted_frame_kind(tx, BC_NOT, 0, bc_notification(AH_JOIN_RESPONSE, data, host_id))
                conn.sendall(push)
                folder.joinpath(f"{number:03}-server-bcnot.bin").write_bytes(push)
                log(f"[{tag}] pushed host join response from host {host_id:#x} ({len(data)} bytes)")
                if args.ah_entities:
                    first, _, last = args.ah_entities.partition(":")
                    data = entity_indices(args.activity_schema, int(first, 0), int(last, 0))
                    push = encrypted_frame_kind(tx, BC_NOT, 0, bc_notification(AH_ENTITY_INDICES, data, host_id))
                    conn.sendall(push)
                    log(f"[{tag}] pushed entity indices {args.ah_entities} from host {host_id:#x}")
                if args.ah_state:
                    establish = b""
                    if args.ah_world_state:
                        parts = args.ah_world_state.split(":")
                        if len(parts) > 3:
                            raise ValueError("--ah-world-state expects ZONE[:PEERBYTE[:GROUPPEER]]")
                        zone = parts[0]
                        world_params = (request, int(parts[1] or "1", 0) if len(parts) > 1 else 1,
                                        int(parts[2], 0) if len(parts) > 2 else 0)
                        zone_now = None if zone == "peer" else int(zone, 0)
                        establish = world_state_delta(args.activity_schema, world_params[0], zone_now,
                                                      *world_params[1:], teleport=teleport_now, version=(world_version_now := next(world_version)))
                        zone_joined = time.time()
                    for subtype, data in ((AH_ESTABLISH, establish), (AH_STATE, activity_state(args.activity_schema, args.ah_map, args.ah_world, args.ah_checksum,
                                                                      args.ah_bubbles, args.ah_zone,
                                                                      args.ah_bubble_states))):
                        push = encrypted_frame_kind(tx, BC_NOT, 0, bc_notification(subtype, data, host_id))
                        conn.sendall(push)
                    log(f"[{tag}] pushed host flag (type 11) and activity state (type 1) from host {host_id:#x}")
                if args.ah_heartbeat:
                    heartbeat_at = time.time() + args.ah_heartbeat[0]
                if args.ah_boot:
                    confirm_repeat = machine_list(args.activity_schema, [request[0x10][0]])
                    conn.sendall(encrypted_frame_kind(tx, BC_NOT, 0, bc_notification(AH_MACHINES, confirm_repeat, host_id)))
                    log(f"[{tag}] pushed boot list (type 43): client")
                if args.ah_roster:
                    slot, _, gen = args.ah_roster.partition(":")
                    peer = request[0x10][0]
                    objects = []
                    if args.ah_sim_player:
                        group, index, owner = args.ah_sim_player[:3]
                        spawn = args.ah_sim_player[3] if len(args.ah_sim_player) > 3 else None
                        objects.append(sim_player(args.activity_schema, group, index, owner, spawn))
                        log(f"[{tag}] simulation player {group:#x}:{index} owner {owner:016x}")
                    game_mode_at = None
                    if args.ah_game_mode:
                        group, index, state = args.ah_game_mode
                        if args.ah_game_mode_schedule:
                            state = args.ah_game_mode_schedule[0][0]
                        game_mode_at, game_mode_now = len(objects), state
                        objects.append(game_mode(args.activity_schema, group, index, state))
                        log(f"[{tag}] game mode {group:#x}:{index} state {state}")
                    for spec in args.ah_object:
                        objects.append(spec)
                        log(f"[{tag}] object {spec[0][0]:#x}:{spec[0][4]:#x}:{spec[0][6]} type {spec[1]:#x} state {spec[2]}")
                    sent = {(o[0][0], o[0][4], o[0][6]) for o in objects}
                    for key, kind, index, has_b in args.ah_baselines:
                        if (key, kind, index) not in sent:
                            objects.append(default_baseline(key, kind, index, has_b))
                    data = sim_repeat = make_sim_packet(peer, int(slot, 0),
                                                   int(gen or "1", 0), objects=objects)
                    sim_build = (peer, int(slot, 0), int(gen or "1", 0), objects, game_mode_at, time.time())
                    conn.sendall(encrypted_frame_kind(tx, BC_NOT, 0, bc_notification(AH_SIMULATION, data, host_id)))
                    log(f"[{tag}] pushed simulation packet (type 5): roster slot {slot} = client")
                    if getattr(args, "ah_bubble_reset", ()):
                        log(f"[{tag}] simulation bubble reset experiment: {args.ah_bubble_reset}")
                for spec in args.ah_after_join:
                    subtype, _, data = spec.partition(":")
                    push = encrypted_frame_kind(tx, BC_NOT, 0, bc_notification(int(subtype, 0), bytes.fromhex(data),
                                                                               host_id))
                    conn.sendall(push)
                    log(f"[{tag}] pushed b->c not subtype {subtype} ({len(data) // 2} bytes) after the join")
            if kind == 1 and msg_type == AH_NOT and heartbeat_at is not None and time.time() >= heartbeat_at:
                # In the activity (host session status 6, 0x308b94) the client declares the host
                # crashed (status 36, reason 0x67, "baboon") once the time since the host's last
                # message (0x25b210, host +0xba8) exceeds the configured timeout (config +0x2d10).
                seconds, subtype = args.ah_heartbeat
                if subtype == WORLD_HEARTBEAT:
                    # the current world state, unchanged version: keeps the group request
                    # (@0x2e48+0x21c8) alive, which an empty type-11 message resets to "none"
                    # Without an owned world state, retain the original empty keepalive.
                    data = (world_state_delta(args.activity_schema, world_params[0], zone_now,
                                              *world_params[1:], target=switch_target, teleport=teleport_now,
                                              version=world_version_now) if world_params else b"")
                    conn.sendall(encrypted_frame_kind(tx, BC_NOT, 0, bc_notification(AH_ESTABLISH, data, host_id)))
                elif subtype >= 0:  # -1: only the machines/simulation repeats below
                    conn.sendall(encrypted_frame_kind(tx, BC_NOT, 0, bc_notification(subtype, b"", host_id)))
                if confirm_repeat:
                    conn.sendall(encrypted_frame_kind(tx, BC_NOT, 0, bc_notification(AH_MACHINES, confirm_repeat, host_id)))
                if sim_build and args.ah_baselines_watch and args.ah_baselines_watch.exists():
                    # objects of a bubble that only loads after the join: pick up the list the
                    # dump script writes during the run and add baselines for the new ones
                    stamp = args.ah_baselines_watch.stat().st_mtime
                    if stamp != baselines_stamp:
                        baselines_stamp = stamp
                        peer_, slot_, gen_, objects_, at_, joined_ = sim_build
                        have = {(o[0][0], o[0][4], o[0][6]) for o in objects_}
                        new = [b for b in baseline_list("@" + str(args.ah_baselines_watch)) if b[:3] not in have]
                        if new:
                            objects_.extend(default_baseline(*b) for b in new)
                            sim_repeat = make_sim_packet(peer_, slot_, gen_, objects=objects_)
                            log(f"[{tag}] {len(new)} new baselines from {args.ah_baselines_watch}")
                if (teleport_now is not None and teleport_now[0] == 1 and teleport_finish_at is not None
                        and time.time() >= teleport_finish_at and world_params):
                    teleport_now = (3, teleport_now[1], teleport_now[2])
                    teleport_finished_at = time.time()
                    teleport_drop_at = teleport_finished_at + TELEPORT_SETTLE_SECONDS
                    data = world_state_delta(args.activity_schema, world_params[0], zone_now,
                                             *world_params[1:], target=switch_target, teleport=teleport_now,
                                             version=world_version_now)  # same version (see above)
                    conn.sendall(encrypted_frame_kind(tx, BC_NOT, 0, bc_notification(AH_ESTABLISH, data, host_id)))
                    log(f"[{tag}] world state: teleport to {teleport_now[1]} finished (state 3, after "
                        f"{args.ah_teleport_finish_delay:g} s)")
                if teleport_drop_at is not None and time.time() >= teleport_drop_at and world_params:
                    teleport_now, teleport_drop_at = None, None
                    data = world_state_delta(args.activity_schema, world_params[0], zone_now,
                                             *world_params[1:], target=switch_target, teleport=None,
                                             version=(world_version_now := next(world_version)))
                    conn.sendall(encrypted_frame_kind(tx, BC_NOT, 0, bc_notification(AH_ESTABLISH, data, host_id)))
                    log(f"[{tag}] world state: finished teleport request dropped")
                if (world_params and teleport_plan and mission_t0(zone_joined) is not None
                        and teleport_now is None):
                    (zone_t, zone_hash), after = teleport_plan[0]
                    if time.time() - mission_t0(zone_joined) >= after:
                        teleport_plan.pop(0)
                        teleport_targeted = requested_client_zone({
                            "target": client_slice_fields.get("target", (None, None))}) == zone_t
                        teleport_finish_at, teleport_finished_at = None, None
                        teleport_now = (1, zone_t, zone_hash or slice_hashes.get(zone_t, 0x811C9DC5))
                        data = world_state_delta(args.activity_schema, world_params[0], zone_now,
                                                 *world_params[1:], target=switch_target, teleport=teleport_now, version=(world_version_now := next(world_version)))
                        conn.sendall(encrypted_frame_kind(tx, BC_NOT, 0, bc_notification(AH_ESTABLISH, data, host_id)))
                        log(f"[{tag}] world state: teleport request -> slice set {zone_t}:{teleport_now[2]:#x} (state 1)")
                if world_params and args.ah_zone_schedule:
                    # A zone change the way the real host moves a player between bubbles: a new
                    # world-state delta (type 11) naming another zone set in group record 0.
                    # The target zone set needs its base entry, i.e. a bubble state >= 0.
                    due = [z for z, sec in args.ah_zone_schedule if time.time() - zone_joined >= sec]
                    if due and due[-1] != zone_now:
                        zone_now = due[-1]
                        data = world_state_delta(args.activity_schema, world_params[0], zone_now, *world_params[1:], teleport=teleport_now, version=(world_version_now := next(world_version)))
                        conn.sendall(encrypted_frame_kind(tx, BC_NOT, 0, bc_notification(AH_ESTABLISH, data, host_id)))
                        log(f"[{tag}] world state: zone set -> {zone_now}")
                if sim_build and objects_applied < len(args.ah_object_at):
                    # Object state changes over time, the way the host advances script state
                    # (e.g. a bubble's objective sensor, kind 3): replace the object's state.
                    peer_, slot_, gen_, objects_, at_, joined_ = sim_build
                    changed = False
                    while (objects_applied < len(args.ah_object_at) and mission_t0(joined_) is not None
                           and time.time() - mission_t0(joined_) >= args.ah_object_at[objects_applied][0]):
                        # (not `seconds`: that is the heartbeat interval used below - reusing it
                        # delayed the next heartbeat by the object's time, TEST361 baboon)
                        due_at, spec = args.ah_object_at[objects_applied]
                        spec = resolve_spec(spec, mission_context())
                        ident = replace_object(objects_, spec)
                        objects_applied += 1
                        changed = True
                        log(f"[{tag}] object {ident[0]:#x}:{ident[1]:#x}:{ident[2]} -> {spec[2]} (at {due_at:g} s)")
                    if changed:
                        at_ = next((i for i, o in enumerate(objects_) if o[0][4] == 0x10), at_) if at_ is not None else None
                        sim_build = (peer_, slot_, gen_, objects_, at_, joined_)
                        sim_repeat = make_sim_packet(peer_, slot_, gen_, objects=objects_)
                if args.ah_control and sim_build:
                    specs, teleports, control_offset, errors = control_steps(
                        args.ah_control, control_offset, args.mission_objects, args.activity_schema)
                    for error in errors:
                        log(f"[{tag}] control: ignored {error}")
                    for reset_changes in specs.bubble_resets:
                        try:
                            bubble_resets = update_bubble_resets(bubble_resets, reset_changes, sim_build[1])
                        except ValueError as exc:
                            log(f"[{tag}] control: ignored bubble reset {reset_changes}: {exc}")
                            continue
                        peer_, slot_, gen_, objects_, _, _ = sim_build
                        sim_repeat = make_sim_packet(peer_, slot_, gen_, tick=host_tick(), objects=objects_)
                        log(f"[{tag}] control: simulation bubble reset experiment -> {bubble_resets}")
                    for zone_t in teleports:
                        teleport_plan.insert(0, (zone_t, 0.0))  # due at once (after any running one)
                        log(f"[{tag}] control: teleport to slice set {zone_t[0]} queued")
                    for spec in specs:
                        reactions_due.schedule(time.time(), spec, "control")
                if sim_build and reactions_due.has_due(time.time()):
                    peer_, slot_, gen_, objects_, at_, joined_ = sim_build
                    for entry in reactions_due.pop_due(time.time()):
                        if isinstance(entry[1], TeleportReaction):
                            teleport_plan.insert(0, (entry[1].zone, 0.0))
                            log(f"[{tag}] teleport to slice set {entry[1].zone[0]} queued ({entry[2]})")
                            continue
                        entry = (entry[0], resolve_spec(entry[1], mission_context()), entry[2])
                        ident = replace_object(objects_, entry[1])
                        log(f"[{tag}] object {ident[0]:#x}:{ident[1]:#x}:{ident[2]} -> {entry[1][2]} ({entry[2]})")
                    at_ = next((i for i, o in enumerate(objects_) if o[0][4] == 0x10), at_) if at_ is not None else None
                    sim_build = (peer_, slot_, gen_, objects_, at_, joined_)
                    sim_repeat = make_sim_packet(peer_, slot_, gen_, objects=objects_)
                if sim_build and sim_build[4] is not None and args.ah_game_mode_schedule:
                    peer_, slot_, gen_, objects_, at_, joined_ = sim_build
                    due = [st for st, sec in args.ah_game_mode_schedule if time.time() - joined_ >= sec]
                    if due and due[-1] != game_mode_now:
                        game_mode_now = due[-1]
                        group, index, _ = args.ah_game_mode
                        objects_[at_] = game_mode(args.activity_schema, group, index, game_mode_now)
                        sim_repeat = make_sim_packet(peer_, slot_, gen_, objects=objects_)
                        log(f"[{tag}] game mode {group:#x}:{index} -> state {game_mode_now}")
                if sim_repeat and sim_build and args.ah_sim_clock:
                    # an advancing host tick: the client's simulation time (0x1c94e0) is the last
                    # host tick plus the time since, so with a constant tick it never advances
                    peer_, slot_, gen_, objects_, at_, joined_ = sim_build
                    sim_repeat = make_sim_packet(peer_, slot_, gen_,
                                            tick=host_tick(), objects=objects_)
                if sim_repeat:
                    # the roster packet is dropped until the client's host session exists
                    conn.sendall(encrypted_frame_kind(tx, BC_NOT, 0, bc_notification(AH_SIMULATION, sim_repeat, host_id)))
                heartbeat_at = time.time() + seconds
                log(f"[{tag}] pushed heartbeat subtype {subtype} from host {host_id:#x}")
            if kind == 1 and msg_type == AH_NOT and args.ah_not_push is not None and len(payload) >= 17:
                subtype, _, data = args.ah_not_push.partition(":")
                push = encrypted_frame_kind(tx, BC_NOT, 0, bc_notification(int(subtype, 0),
                                                                           bytes.fromhex(data), host_id))
                conn.sendall(push)
                folder.joinpath(f"{number:03}-server-bcnot.bin").write_bytes(push)
                log(f"[{tag}] pushed b->c not from host {host_id:#x} subtype {subtype} ({len(data) // 2} bytes)")
            if kind == 1 and msg_type == AH_MGR_REQ and args.ah_mgr_reply is not None:
                reply = encrypted_frame(tx, AH_MGR_RSP, msg_id, 200, args.ah_mgr_reply)
                conn.sendall(reply)
                folder.joinpath(f"{number:03}-server-07.bin").write_bytes(reply)
                log(f"[{tag}] sent c->ah mgr rsp ({len(args.ah_mgr_reply)} bytes)")
            if kind == 1 and msg_type == AHP_REQ and args.ahp_reply is not None:
                response = ahp_response(payload, args.ahp_reply)
                reply = encrypted_frame(tx, AHP_RSP, msg_id, 200, response)
                conn.sendall(reply)
                folder.joinpath(f"{number:03}-server-11.bin").write_bytes(reply)
                log(f"[{tag}] sent ahp rsp {response.hex()} (request {payload.hex()})")
                if args.ahp_push is not None:
                    subtype, _, data = args.ahp_push.partition(":")
                    push = encrypted_frame_kind(tx, BC_NOT, 0, bc_notification(int(subtype, 0),
                                                                               bytes.fromhex(data)))
                    conn.sendall(push)
                    folder.joinpath(f"{number:03}-server-bcnot.bin").write_bytes(push)
                    log(f"[{tag}] pushed b->c not subtype {subtype} data {data}")
            if kind == 1 and msg_type == CLIENT_CONFIG_REQ and args.ws_signin:
                reply = encrypted_frame(tx, CLIENT_CONFIG_RSP, msg_id, 200, client_config_reply(*args.client_config))
                conn.sendall(reply)
                folder.joinpath(f"{number:03}-server-13.bin").write_bytes(reply)
                log(f"[{tag}] sent client config rsp (request {payload.hex()})")
            if kind == 1 and (msg_type in EMPTY_REPLIES or msg_type in PROTO_REPLIES):
                reply_type, reply_body = (PROTO_REPLIES[msg_type] if msg_type in PROTO_REPLIES
                                          else (EMPTY_REPLIES[msg_type], b""))
                if msg_type == MM_REQ and args.mm_rsp:
                    reply_body = mm_rsp_body(args.mm_rsp)
                if msg_type == USER_MSG_REQ and args.user_msg:
                    reply_body = user_msg_body(args.user_msg)
                reply = encrypted_frame(tx, reply_type, msg_id, 200, reply_body)
                conn.sendall(reply)
                folder.joinpath(f"{number:03}-server-{reply_type:02x}.bin").write_bytes(reply)
                detail = f"{len(reply_body)}-byte body" if reply_body else "empty"
                log(f"[{tag}] sent {NAMES[reply_type]} ({detail}, status 200)")
                if msg_type == SUB_REQ and len(payload) >= 9 and args.ws_signin and not args.no_sub_push:
                    # sub req payload: u8 family type, u64 root soid. Answer with that family
                    # (version 1, no objects) as a queuez update push.
                    family_type, root = payload[0], int.from_bytes(payload[1:9], "big")
                    push = encrypted_frame_kind(tx, QZ_UPDATE, 0,
                                                family_blob(args.schema, family_type, root, character_id,
                                                            character_profile, active_id=active_id,
                                                            self_flags=args.self_flags, peer=args.peer_family,
                                                            extras=args.character_extras,
                                                            account_data=account_data))
                    conn.sendall(push)
                    folder.joinpath(f"{number:03}-server-qz-update.bin").write_bytes(push)
                    log(f"[{tag}] pushed family type {family_type} root {root:#x} "
                        f"({len(push)} bytes, root object {'yes' if args.schema and family_type in ROOT_OBJECTS else 'no'})")
            if msg_type == START_REQ and kind == 2:
                magic = payload[:8]
                body = payload if args.start_rsp_body is None else args.start_rsp_body
                reply = plain_frame(START_RSP, msg_id, 200, body)
                log(f"[{tag}] start req magic {magic!r} ({len(payload)} bytes); sending start rsp "
                    f"({len(body)}-byte body{', echo' if args.start_rsp_body is None else ''})")
                folder.joinpath(f"{number:03}-server-start-rsp.bin").write_bytes(reply)
                conn.sendall(reply)
            if msg_type == SHELLO_REQ:
                fields = [(n, w, v if w == 0 else len(v)) for n, w, v in signon_proto.decode(payload)]
                log(f"[{tag}] shello req fields {fields}")
                nonce, session_key = secrets.token_bytes(12), secrets.token_bytes(16)
                reply = plain_frame(SHELLO_RSP, msg_id, 200, shello_rsp_body(
                    args.key, args.mac_key, nonce, session_key, secrets.token_bytes(16)))
                conn.sendall(reply)
                rx, tx = channels(nonce, session_key)
                folder.joinpath(f"{number:03}-server-shello-rsp.bin").write_bytes(reply)
                folder.joinpath("session.json").write_text(json.dumps(
                    {"nonce": nonce.hex(), "session_key": session_key.hex()}), encoding="utf-8")
                log(f"[{tag}] sent shello rsp ({len(reply)} bytes)")
    except (OSError, EOFError, ValueError, InvalidTag) as exc:
        log(f"[{tag}] {exc}")
    finally:
        conn.close()
        sink, _events.sink = _events.sink, None
        sink.close()
        if quiet_notifications:
            log(f"[{tag}] retained full captures for {quiet_notifications} routine type-15 notifications; "
                "per-packet console lines omitted (use --verbose-wire to restore them)")


def user_msg_body(spec):
    """User msg rsp (0x21) from "N=VALUE|...": fields 1-3 are strings, field 4 (required by the
    decoder 0x47bf9c / 0xbfbe0c) an integer that defaults to 0."""
    values = {4: "0"}
    for part in filter(None, spec.split("|")):
        number, _, value = part.partition("=")
        if int(number) not in (1, 2, 3, 4):
            raise ValueError(f"user msg has fields 1-4, not {number}")
        values[int(number)] = value
    body = b""
    for number in sorted(values):
        if number == 4:
            body += signon_proto.field(4, signon_proto.VARINT, int(values[4], 0))
        else:
            body += signon_proto.field(number, signon_proto.BYTES, values[number].encode("utf-8"))
    return body


def parse_args(argv=None):
    """Command-line arguments, with the schemas loaded and the SignOn test keys set (also used
    by the end-to-end tests, which call handle() with them)."""
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--bind", default="127.1.1.127")
    ap.add_argument("--port", type=int, default=12850)
    ap.add_argument("--timeout", type=float, default=120)
    ap.add_argument("--ws-zero-bytes", type=int, default=1024,
                    help="answer c->ws requests with the service id and this many zero bytes (0: no reply)")
    ap.add_argument("--start-rsp-body", type=bytes.fromhex, default=None, metavar="HEX",
                    help="body of the 01.24 start rsp (0x1f) answering start req (0x1e); default: echo "
                    "the 128-byte request payload (the client decodes exactly 0x80 bytes)")
    ap.add_argument("--user-msg", default="", metavar="N=VALUE|...",
                    help="01.24 user msg rsp fields: 1-3 text (descriptor 0x013c4080: 0x200/0x40/0x40 "
                    "bytes), 4 the required integer; default: field 4 = 0 only")
    ap.add_argument("--mm-rsp", default="", metavar="N=VALUE,...",
                    help="01.24 mm rsp fields (1-2 integers, 3-6 submessages as hex); default: empty, which the "
                    "client treats as 'no match yet'")
    ap.add_argument("--ws-signin", action="store_true",
                    help="answer service 503 with a typed sign-in response carrying the request's account id")
    ap.add_argument("--ws-no-optional", action="store_true",
                    help="with --ws-signin, leave the optional u64 fields of the sign-in reply absent")
    ap.add_argument("--ws-markers", action="store_true",
                    help="with --ws-signin, fill the reply with distinct marker values to find copies in memory")
    ap.add_argument("--no-sub-push", action="store_true",
                    help="with --ws-signin, answer sub req without pushing the subscribed family "
                    "(experiment: does the client reject the push?)")
    ap.add_argument("--qz-push", nargs="*", default=[],
                    help="after the ws reply, push families as name[:version[:flags]] with no objects")
    ap.add_argument("--qz-root", type=lambda v: int(v, 0), default=0, help="root soid for pushed families")
    ap.add_argument("--ws-result", type=int, default=0,
                    help="ws result code in the first 4 bits (0 error_none, 1 unknown, 2 no_content, "
                         "3 moved, 4 not_found, 5 service_unavailable, 6 unauthorized, 7 timeout)")
    ap.add_argument("--capture-dir", type=Path, default=CAPTURE_DIR)
    ap.add_argument("--verbose-wire", action="store_true",
                    help="also print each routine type-15 packet; full binary/event captures are always retained")
    ap.add_argument("--ws-no-reply", type=lambda v: {int(x) for x in v.split(",") if x},
                    default=set(),
                    help="comma-separated ws services to leave unanswered (experiments only)")
    ap.add_argument("--client-config", type=lambda v: tuple(int(x, 0) for x in v.split(",")), default=(1, 1),
                    metavar="V1,V2", help="client config rsp fields 1 (uint64) and 2 (uint32); meaning unknown")
    ap.add_argument("--universe-time", choices=sorted(UNIVERSE_TIME_UNITS), default="one",
                    help="value sent in universe +8 (suspected server clock): 1 or now in a unit")
    ap.add_argument("--universe-flags", type=lambda v: [int(x, 0) for x in v.split(",") if x], default=[],
                    help="experiment: i16 list in the universe object's +0x10 field (service 104)")
    ap.add_argument("--universe-flag-values", dest="universe_values", default=[],
                    type=lambda v: [tuple(int(y, 0) for y in x.split("=")) for x in v.split(",") if x],
                    help="universe flag overrides INDEX=VALUE,... (tri-state: 2 true, 1 false) in the "
                         "universe object's +0x3c list (sign-in and service 104)")
    ap.add_argument("--peer-family", action="store_true",
                    help="fill the peer family (0) subscription with the local account and character "
                         "summaries (the player's appearance and blueprint in activities come from it)")
    ap.add_argument("--character-gear", type=gear_spec, metavar="starter:[01.24:]CLASS|SLOT=ITEM,...|@FILE",
                    help="equip the stored character (peer/select summaries and self inventory): "
                         "starter:titan|hunter|warlock for 01.00; starter:01.24:CLASS for 01.24; "
                         "or investment item indexes per equipment slot 0-19 (scripts/d1_pkg.py items); "
                         "@FILE accepts an optional build-specific arrangement model index per slot. "
                         "01.24 starter armour includes these indexes; missing model selections "
                         "can leave the world guardian as a see-through placeholder")
    ap.add_argument("--character-overlay", type=overlay_spec,
                    metavar="JSON|@FILE",
                    help='raw field values merged into the character objects: {"peer"|"select"|"self": '
                         '{offset: value}} (peer = 0x808017fd, select = 0x80801954, self = 0x80801714)')
    ap.add_argument("--self-flags",
                    help='account flags: "2" (all), "2@A0-511,B0-255" (ranges), layered with ";" '
                         '(e.g. "1;2@B0": all false, B0 true)')
    ap.add_argument("--ahp-reply", type=bytes.fromhex,
                    help="16-byte hex ahp rsp (0x11): valid 8-byte requests replace the first "
                         "u64 with their request id; the configured endpoint stays unchanged. "
                         "Other lengths remain raw experiment payloads")
    ap.add_argument("--ahp-push", help="experiment: SUBTYPE:HEX b->c not pushed after the ahp rsp")
    ap.add_argument("--ah-mgr-reply", type=bytes.fromhex,
                    help="experiment: hex payload for c->ah mgr rsp (0x07), normally 137 bytes")
    ap.add_argument("--ah-not-push",
                    help="experiment: SUBTYPE:HEX b->c not pushed (from that host) after c->ah not")
    ap.add_argument("--ah-join", action="store_true",
                    help="answer host join requests (c->ah not type 3) with a joined response")
    ap.add_argument("--ah-entities", metavar="FIRST:LAST",
                    help="with --ah-join, then grant entity indices FIRST..LAST-1 (b->c not type 0)")
    ap.add_argument("--ah-state", action="store_true",
                    help="with --ah-join, then send host type 11 and a minimal activity state (type 1)")
    ap.add_argument("--ah-map", help="with --ah-state, map name in the activity state (e.g. orbit)")
    ap.add_argument("--ah-world", type=int, help="with --ah-state, world id (i16) in the activity state")
    ap.add_argument("--ah-bubbles", type=int,
                    help="with --ah-state, bubble count of the scenario (states left at default)")
    ap.add_argument("--ah-bubble-states", metavar="DEFAULT[,BUBBLE=STATE...]",
                    type=lambda v: {(int(k, 0) if k else None): int(x, 0)
                                    for k, _, x in (("", "", t) if "=" not in t else t.partition("=")
                                                    for t in v.split(","))},
                    help="with --ah-bubbles, per-bubble state indices (default -1 = none loaded)")
    ap.add_argument("--ah-checksum", type=lambda v: int(v, 0),
                    help="with --ah-state, activity checksum (u32 @0x5c) matching the scenario")
    ap.add_argument("--ah-zone", type=lambda v: tuple(int(x, 0) for x in v.split(":")), metavar="INDEX:HASH",
                    help="with --ah-state, zone record (u32 @0x54, i32 @0x58); -1:0x811c9dc5 = none")
    ap.add_argument("--ah-world-state", nargs="?", const="peer", metavar="ZONE[:PEERBYTE[:GROUPPEER]]",
                    help="with --ah-state, the type-11 body carries a world-state delta: the client "
                         "in peer slot 0 and, with ZONE, group record 0 for that zone-set index; "
                         "GROUPPEER sets the record's 5-bit peer field (default 0, valid 0-19)")
    ap.add_argument("--ah-zone-schedule", default=[], metavar="ZONE@SECONDS,...",
                    type=lambda v: [tuple(int(y, 0) for y in x.split("@")) for x in v.split(",")],
                    help="with --ah-world-state ZONE and --ah-heartbeat, send a new world-state delta "
                         "naming each ZONE (zone-set index, 8*bubble for a bubble's base) SECONDS after "
                         "the join (e.g. 16@120,24@240); the bubbles need states >= 0 (--ah-bubble-states)")
    ap.add_argument("--ah-boot", action="store_true",
                    help="experiment: send host type 43 (boot list) naming the client's machine -> error bass")
    ap.add_argument("--ah-roster", metavar="SLOT[:GEN]",
                    help="with --ah-join, send a type-5 simulation packet with the client in roster SLOT")
    ap.add_argument("--ah-bubble-reset", type=bubble_reset_spec, default=(),
                    metavar="SLOT[:GEN],...",
                    help="EXPERIMENT: with --ah-roster, change these inactive roster slots to the "
                         "given generation (default 1), testing bubble reset/baseline readiness. "
                         "The client slot stays active; generations remain fixed across heartbeats")
    ap.add_argument("--ah-sim-player", type=lambda v: tuple(int(x, 0) for x in v.split(":")),
                    metavar="GROUP:INDEX:OWNER[:SPAWN]",
                    help="with --ah-roster, the type-5 packets also replicate simulation player INDEX "
                         "of entity group GROUP owned by player id OWNER (the local pla +0x18, e.g. "
                         "0x4786c0e0:5:0x0a3d4a2c8fbea6ba); SPAWN=1 sets the state's bool +0x48 (entity +0x18c), "
                         "which the spawner waits for")
    ap.add_argument("--ah-game-mode", type=lambda v: tuple(int(x, 0) for x in v.split(":")),
                    metavar="GROUP:INDEX:STATE",
                    help="with --ah-roster, replicate the game-mode object (kind 0x10) with this state "
                         "(non-zero ends the loading screen, 3 lets the spawner continue), e.g. 0x4786c0e0:3:3")
    ap.add_argument("--ah-game-mode-schedule", default=[], metavar="STATE@SECONDS,...",
                    type=lambda v: [tuple(int(y, 0) for y in x.split("@")) for x in v.split(",")],
                    help="with --ah-game-mode, step the game-mode state over time: the first state is sent "
                         "at the join, each later one from SECONDS after it (e.g. 1@0,2@20,3@40)")
    ap.add_argument("--ah-object", action="append", default=[], type=object_spec,
                    metavar="GROUP:KIND:INDEX:TYPE[:B]=JSON",
                    help="with --ah-roster, replicate this object with an explicit state of TYPE (fields "
                         "by offset, e.g. 0x992bb471:0x20:7:0x80802a1b={\"0\": 1}); repeatable")
    ap.add_argument("--ah-object-at", action="append", default=[], type=timed_object_spec,
                    metavar="SECONDS@GROUP:KIND:INDEX:TYPE[:B]=JSON",
                    help="with --ah-roster and --ah-heartbeat, replicate this object state from SECONDS "
                         "after the join, replacing any earlier state of the object (repeatable; e.g. "
                         "step an objective sensor: 60@G:3:I:0x808004fa:1={\"0\": {\"0\": 1}})")
    ap.add_argument("--ah-teleport", type=lambda v: (parse_slice_set(v.split("@")[0]), float(v.split("@")[1])),
                    metavar="ZONE[:HASH]@SECONDS",
                    help="with --ah-world-state, SECONDS after the join ask the group to move to slice set "
                         "ZONE (world-state group block @0x21c8, state 1; state 3 once the client reports it) - "
                         "the host's hand-off after the intro cinematic (113 -> 112)")
    ap.add_argument("--ah-teleport-finish-delay", type=float, default=0.0, metavar="SECONDS",
                    help="with --ah-teleport, wait this long after the client reports its arrival (slice "
                         "record state 4) before sending the finishing state 3")
    ap.add_argument("--ah-group-switch", action="store_true",
                    help="with --ah-world-state, answer the client's slice-set requests (type 21 target) "
                         "the way the host does: group record 1 = {target, state 1, local peer} while "
                         "record 0 stays current; record 0 moves when the client reports the target")
    ap.add_argument("--ah-follow-client-zone", action="store_true",
                    help="with --ah-world-state, send a new world-state delta naming the zone set the "
                         "client reports in its type-21 state (target, else current) whenever it changes")
    ap.add_argument("--ah-on", action="append", default=[], type=reactive_object_spec,
                    metavar="TYPE[/REGEX]=>SECONDS@GROUP:KIND:INDEX:TYPE[:B]=JSON",
                    help="with --ah-roster and --ah-heartbeat, replicate this object state SECONDS after "
                         "the client first sends activity-host message TYPE whose body hex matches REGEX "
                         "(once per rule; repeatable), e.g. step the next objective when a trigger "
                         "volume reports: 15/^41=>0@G:3:I:0x808004fa={\"0\": {\"0\": 2}}")
    ap.add_argument("--ah-mission", type=Path, metavar="FILE",
                    help="mission script (JSON, e.g. missions/a_guardian_rises.json): named objects "
                         "and steps that set their states at a time or when the client first reports "
                         "an object (expanded into --ah-object-at / --ah-on); \"$player\" and \"$now\" "
                         "in states are the local player's machine id and the host tick")
    ap.add_argument("--ah-control", type=Path, metavar="FILE",
                    help="DEBUG: apply mission steps appended to FILE live, one JSON line each, e.g. "
                         "{\"set\": {\"obj_enter_wall\": {\"0\": 1}}} (names from --ah-mission)")
    ap.add_argument("--ah-sim-clock", type=float, default=0, metavar="HZ",
                    help="send an advancing host tick (HZ ticks a second since the first simulation "
                         "packet; the client's sequences and timers run on it; 30 matches the "
                         "client's default rate) instead of a constant 0")
    ap.add_argument("--ah-baselines", default=[], metavar="KEY/KIND/INDEX[/B],...|@FILE", type=baseline_list,
                    help="with --ah-roster, default baselines for these replicated objects (B = 1 when the "
                         "record has a state B type); @FILE reads the list from a file written by "
                         "scripts/rpcs3_replicated_objects.py")
    ap.add_argument("--ah-baselines-watch", type=Path, metavar="FILE",
                    help="with --ah-heartbeat, re-read this --ah-baselines list whenever it changes and "
                         "add default baselines for objects not sent yet (bubbles loaded after the join)")
    ap.add_argument("--ah-echo-incidents", action="store_true",
                    help="EXPERIMENT: send every client incident (type 18) straight back to the client")
    ap.add_argument("--ah-heartbeat", type=lambda v: (float(v.split(":")[0]),
                                                      WORLD_HEARTBEAT if (v.split(":") + ["world"])[1] == "world"
                                                      else int((v.split(":") + ["world"])[1], 0)),
                    metavar="SECONDS[:SUBTYPE|world]",
                    help="with --ah-join, resend the current world state (default; empty type 11 when absent) "
                         "plus machines/simulation every SECONDS; an explicit numeric subtype sends an empty "
                         "message, -1 sends none, and 'world' selects the default state-preserving mode")
    ap.add_argument("--ah-after-join", nargs="*", default=[], metavar="SUBTYPE:HEX",
                    help="experiment: further b->c not pushes after the join response")
    ap.add_argument("--ah-join-fields", default=[],
                    type=lambda v: [tuple(int(y, 0) for y in x.split("=")) for x in v.split(",") if x],
                    help="experiment: OFFSET=VALUE,... for the join response's trailing fields "
                         "(i32 @0x17c/0x184, biased i32 @0x180/0x188, u8 @0x18c, u16 @0x18e/0x190, i32 @0x194)")
    ap.add_argument("--ah-session", default="d1local-activity",
                    help="activity session id sent in the host join response")
    ap.add_argument("--activity-schema", type=Path, default=ACTIVITY_SCHEMA,
                    help="activity host message schema from a runtime dump or EBOOT type export")
    ap.add_argument("--player-db", type=Path,
                    help="durable player database (SQLite, server/player_db.py; replaces --store): "
                         "accounts, several characters per account, settings")
    ap.add_argument("--store", type=Path,
                    help="JSON file remembering created characters per account (outside Git)")
    ap.add_argument("--schema", type=Path, default=investment.DEFAULT_SCHEMA,
                    help="investment type schema from a runtime dump or EBOOT type export (optional)")
    ap.add_argument("--services-schema", type=Path, default=investment.DEFAULT_SCHEMA,
                    help="saved service ID table used when --schema is an EBOOT-only type export")
    args = ap.parse_args(argv)
    if args.ah_bubble_reset:
        if args.ah_roster is None:
            ap.error("--ah-bubble-reset requires --ah-roster")
        try:
            slot, _, generation = args.ah_roster.partition(":")
            simulation_roster(0, int(slot, 0), int(generation or "1", 0), args.ah_bubble_reset)
        except ValueError as exc:
            ap.error(str(exc))
    args.schema = investment.load_schema(args.schema) if args.schema.exists() else None
    if args.schema and not args.schema.get("services"):
        if not args.services_schema.exists():
            ap.error(f"--schema has no service IDs; missing --services-schema {args.services_schema}")
        try:
            args.schema = with_service_map(args.schema, investment.load_schema(args.services_schema))
        except ValueError as exc:
            ap.error(str(exc))
    args.character_extras = character_extras(args.character_gear, args.character_overlay,
                                             character_layout(args.schema))
    args.activity_schema = (investment.load_schema(args.activity_schema)
                            if args.activity_schema.exists() else None)
    args.ah_teleports = [args.ah_teleport] if args.ah_teleport else []
    args.mission_clock = "world"
    args.mission_objects = {}
    if args.ah_mission:
        if not args.activity_schema:
            ap.error("--ah-mission needs the activity schema (--activity-schema)")
        timed, reactive = expand_mission(json.loads(args.ah_mission.read_text(encoding="utf-8")),
                                         args.activity_schema)
        try:
            validate_mission_states(args.activity_schema, timed, reactive)
        except ValueError as exc:
            ap.error(f"--ah-mission: {exc}")
        args.ah_object_at += timed
        args.ah_on += reactive
        args.mission_objects = json.loads(args.ah_mission.read_text(encoding="utf-8")).get("objects", {})
        args.ah_teleports += mission_teleports(json.loads(args.ah_mission.read_text(encoding="utf-8")))
        try:
            args.mission_clock = mission_clock(json.loads(args.ah_mission.read_text(encoding="utf-8")))
        except ValueError as exc:
            ap.error(f"--ah-mission: {exc}")
    args.ah_object_at.sort(key=lambda entry: entry[0])
    if not ipaddress.IPv4Address(args.bind).is_loopback:
        ap.error("listener must be loopback")
    args.key, args.mac_key = signon_proto.TEST_SESSION_KEYS[1], signon_proto.TEST_SESSION_KEYS[2]
    return args


def main():
    args = parse_args()
    with socket.socket() as server:
        server.bind((args.bind, args.port))
        server.listen(8)
        log(f"BAP stand-in listening on {args.bind}:{args.port}")
        while True:
            conn, peer = server.accept()
            threading.Thread(target=handle, args=(conn, peer, args), daemon=True).start()


if __name__ == "__main__":
    main()
