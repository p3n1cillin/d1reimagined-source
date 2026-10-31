"""Read the local Guardian's current unit transform on PS4 build 56578.

No debugger, scan, game calls or memory writes. The caller must first pass
d1-winguest.open_client's build guard. These offsets are specific to that build.
The player +0x644 unit link was already used by d1-bundle-poke. Object resolution
matches the client's object-data table walk. A forward/backward input control on
28 Oct 2026 moved all three checked transform copies together.
"""
from dataclasses import dataclass
import math
import struct


PLAYER_POOL = 0x273EA80
PLAYER_STRIDE = 0x2AF0
UNIT_LINK = 0x644
OBJECT_TABLE = 0x26897F0
SELF_HANDLE = 0x24
TRANSFORMS = (0x1A0, 0xB00, 0xB50)
FORWARD_VECTORS = (0x230, 0x3A0)
U64_MASK = (1 << 64) - 1


@dataclass(frozen=True)
class UnitPosition:
    handle: int
    address: int
    position: tuple


def read_unit_position(reader, base, player_index=0):
    """Return a consistent current-unit sample, or None during teardown/transition.

    Refuse absent links, mismatched self handles, invalid table geometry, divergent
    transform copies and a player link that changes during the read. Never fall
    back to an earlier unit or heuristic position. OSError means an unavailable
    native read and is returned to the caller as None.
    """
    if not 0 <= player_index < 4:
        raise ValueError('player_index must be 0..3')
    try:
        pool = reader.u(base + PLAYER_POOL, 'Q')
        if not pool:
            return None
        data = reader.u(pool + 8, 'Q')
        if not data:
            return None
        player = data + player_index * PLAYER_STRIDE
        handle = reader.u(player + UNIT_LINK, 'I')
        if handle in (0, 0xFFFFFFFF):
            return None
        table = base + OBJECT_TABLE
        entries = reader.u(table + 8, 'Q')
        stride = reader.u(table + 0x10, 'I')
        if not entries or stride != 32:
            return None
        mask = reader.u(table + 0x28, 'i') & U64_MASK
        entry = entries + (handle & 0x1FFF) * stride
        unit = (entry - (mask & reader.u(entry + 8, 'Q'))) & U64_MASK
        if not unit or reader.u(unit + SELF_HANDLE, 'I') != handle:
            return None
        points = [struct.unpack('<3f', reader.read(unit + offset, 12)) for offset in TRANSFORMS]
        if not all(math.isfinite(v) and abs(v) < 1e6 for point in points for v in point):
            return None
        if any(math.dist(points[0], point) > 0.05 for point in points[1:]):
            return None
        if (reader.u(player + UNIT_LINK, 'I') != handle or
                reader.u(unit + SELF_HANDLE, 'I') != handle):
            return None
        return UnitPosition(handle, unit, points[0])
    except OSError:
        return None


def read_unit_facing(reader, unit):
    """Yaw agreed by the unit's two forward vectors, or None.

    Caller supplies a freshly identity-checked unit address. A native right/left
    200 ms control changed both vectors together (about -0.456/+0.417 radians).
    """
    try:
        vectors = [struct.unpack('<3f', reader.read(unit + offset, 12)) for offset in FORWARD_VECTORS]
        if not all(all(math.isfinite(v) for v in vector) and
                   abs(math.sqrt(sum(v * v for v in vector)) - 1) < 0.02 and
                   math.hypot(*vector[:2]) > 0.01 for vector in vectors):
            return None
        headings = [math.atan2(vector[1], vector[0]) for vector in vectors]
        difference = math.atan2(math.sin(headings[0] - headings[1]), math.cos(headings[0] - headings[1]))
        return headings[0] if abs(difference) < 0.1 else None
    except OSError:
        return None
