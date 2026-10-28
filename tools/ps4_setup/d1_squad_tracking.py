"""Read active squad members on PS4 56578; no game calls, debugger or writes.

Use after d1-winguest.open_client(). Membership is the native creation counter's
entity+0x50 == -1 test, with the sense predicate checked separately. It is not a
health measurement. Unknown/missing bindings are diagnostics, never dead enemies.
The type map locates physical components, so no object-table or position scan is
needed. Every poll resolves current content, serials, parent and component links.
"""
import math
import struct
import time

REGISTRY = 0x1A238A0
PAIR_REGISTRY = 0x1A23898
LIFETIME_TABLE = 0x1AD5000
CURRENT_CONTENT = 0x25D6FF8
MEMBER_TABLE = 0x25D79A0
ENTITY_TABLE = 0x2692220
OBJECT_TABLE = 0x26897F0
ALL_COMPONENTS_KEY = 0x2B1BB31B
BODY_TAGS = {0x809DCEF6, 0x809DCCE5}
TORSO_TAGS = {0x809DE137, 0x809DDC8B}
MASK64 = (1 << 64) - 1


class Unavailable(RuntimeError):
    """A native binding is missing, inconsistent or changed during the sample."""


def _require(ok, message):
    if not ok:
        raise Unavailable(message)


def _point(raw):
    point = struct.unpack('<3f', raw)
    _require(all(math.isfinite(v) and abs(v) < 1e6 for v in point), 'invalid position')
    return point


class SquadReader:
    def __init__(self, reader, base):
        self.p, self.base = reader, base

    def resolve(self, handle):
        """The registry's signed self-relative object-data lookup."""
        _require(handle not in (0, 0xFFFFFFFF), 'absent handle')
        p = self.p
        reg = p.u(p.u(self.base + REGISTRY, 'Q'), 'Q')
        signed = struct.unpack('<i', struct.pack('<I', handle))[0] >> 13
        slot = (((signed & 0xFFFFFFFF) >> 18) | 0x3FF) & signed & 0xFFFFFFFF
        rec = reg + slot * 64
        data, stride = p.u(rec + 16, 'Q'), p.u(rec + 48, 'i')
        _require(data and 0 < stride <= 0x100000, 'invalid registry geometry')
        entry = data + (handle & 0x1FFF) * stride
        mask = p.u(rec + 52, 'i') & MASK64
        obj = (entry - (mask & p.u(entry + 8, 'Q'))) & MASK64
        _require(obj, 'null object')
        return obj

    def object(self, handle):
        p, table = self.p, self.base + OBJECT_TABLE
        _require(handle not in (0, 0xFFFFFFFF), 'absent object')
        data, stride = p.u(table + 8, 'Q'), p.u(table + 16, 'I')
        _require(data and stride == 32, 'invalid object table')
        entry = data + (handle & 0x1FFF) * stride
        mask = p.u(table + 40, 'i') & MASK64
        obj = (entry - (mask & p.u(entry + 8, 'Q'))) & MASK64
        _require(obj and p.u(obj + 0x24, 'I') == handle, 'recycled object')
        return obj

    def pair(self, raw):
        """0x71F60: validate the serial in a squad's {serial, datum} pair."""
        p = self.p
        serial, handle = struct.unpack('<II', raw)
        _require(handle != 0xFFFFFFFF, 'absent member pair')
        slot = (handle >> 13) & (0x3FF | (0x3C00 if handle & 0x80000000 else 0))
        registry = p.u(self.base + PAIR_REGISTRY, 'Q')
        stride, data = p.u(registry + 16, 'I'), p.u(registry, 'Q')
        _require(data and 0 < stride <= 0x1000, 'invalid pair registry')
        pool = p.u(data + slot * stride + 0x18, 'Q')
        _require(pool, 'missing member pool')
        capacity = p.u(p.u(pool, 'Q') + 0x1C, 'H')
        _require((handle & 0x1FFF) < capacity, 'member outside pool')
        entries = p.u(pool + 8, 'Q')
        offset, size = p.u(pool + 0x1C, 'I'), p.u(pool + 0x20, 'I')
        _require(entries and 0 < size <= 0x100000, 'invalid member pool')
        _require(p.u(entries + offset + (handle & 0x1FFF) * size, 'I') == serial,
                 'recycled member pair')
        return handle

    def content(self):
        p, table = self.p, self.base + LIFETIME_TABLE
        index = p.u(self.base + CURRENT_CONTENT, 'H')
        _require(index != 0xFFFF, 'no current content')
        stride = p.u(table + 16, 'Q')
        _require(0 < stride <= 0x100000, 'invalid lifetime table')
        cell = table + p.u(table, 'q') + index * stride + 0x30
        content = p.u(cell, 'Q')
        _require(content, 'null content')
        return index, content

    def component_offsets(self, root):
        """Native 0x324870 lookup of the common component base type."""
        p = self.p
        definition = self.resolve(p.u(root + 8, 'I')) + p.u(root + 16, 'Q')
        c = definition + 0x38
        size = p.u(c + 8, 'I')
        _require(0 < size <= 4096 and size & (size - 1) == 0, 'invalid component map')
        key = ALL_COMPONENTS_KEY
        h = ((key << 4) + (p.u(c, 'I') ^ key)) & 0xFFFFFFFF
        h ^= h >> 10
        h = (h * 0x81) & 0xFFFFFFFF
        h ^= h >> 13
        slot = h & (size - 1)
        slots = c + 0x20 + p.u(c + 0x10, 'q')
        records = c + 0x30 + p.u(c + 0x20, 'q')
        for _ in range(size):
            found, start, flags = struct.unpack('<Ihh', p.read(slots + slot * 8, 8))
            if found == key:
                count = flags & 0x7FFF
                _require(0 <= start and 0 < count <= 512, 'invalid component range')
                offsets = [p.u(records + (start + j) * 0x28 + 4, 'i') for j in range(count)]
                _require(all(0 <= o <= 0x100000 for o in offsets), 'invalid component offset')
                return tuple(dict.fromkeys(offsets))
            if flags >= 0:
                break
            slot = (slot + 1) & (size - 1)
        raise Unavailable('missing component base type')

    def member(self, pair):
        p = self.p
        handle = self.pair(pair)
        table = self.base + MEMBER_TABLE
        data, stride = p.u(table + 8, 'Q'), p.u(table + 16, 'I')
        _require(data and 0x80 <= stride <= 0x100000, 'invalid entity table')
        ent = data + (handle & 0x1FFF) * stride
        raw = p.read(ent, 0x58)
        _require(struct.unpack_from('<I', raw, 0x38)[0] == handle, 'member identity mismatch')
        active = struct.unpack_from('<i', raw, 0x50)[0] == -1
        result = {'member_handle': handle, 'active': active, 'parts': [], 'health': None}
        if not active:
            return result
        # 0x138190, the udt-6 squad-count predicate. The retail missing-state
        # fallback returns true; do not promote that permissive path to a target.
        state_handle = struct.unpack_from('<I', raw, 0x28)[0]
        _require(state_handle != 0xFFFFFFFF, 'missing member sense state')
        state = self.resolve(state_handle) + struct.unpack_from('<Q', raw, 0x30)[0]
        sense_table = self.base + 0x25D7A20
        sense_stride = p.u(sense_table + 16, 'I')
        sense_data = p.u(sense_table + 8, 'Q')
        _require(sense_data and 0x150 <= sense_stride <= 0x100000, 'invalid sense entity table')
        sense_at = sense_data + (handle & 0x1FFF) * sense_stride + 0x14C
        state_value, sense_value = p.u(state + 0x274, 'I'), p.u(sense_at, 'I')
        result['sense_matches'] = state_value == sense_value
        if not result['sense_matches']:
            return result
        transform = struct.unpack_from('<I', raw, 0x3C)[0]
        _require(transform not in (0, 0xFFFFFFFF), 'no transform entity')
        t = self.base + ENTITY_TABLE
        data2, stride2 = p.u(t + 8, 'Q'), p.u(t + 16, 'I')
        _require(data2 and stride2 == 240, 'invalid transform entity table')
        entity = data2 + (transform & 0x1FFF) * stride2
        parent = p.u(entity + 0x54, 'I')
        # Parent is an allocation root, not a component with self-handle +0x24.
        root = self.resolve(parent)
        result.update(transform_handle=transform, parent_handle=parent)
        for offset in self.component_offsets(root):
            component = root + offset
            tag = p.u(component, 'I')
            if tag not in BODY_TAGS | TORSO_TAGS:
                continue
            unit_handle = p.u(component + 0x24, 'I')
            _require(self.object(unit_handle) == component, 'physical component identity mismatch')
            _require(p.u(component + 0x2C, 'I') == transform, 'physical transform mismatch')
            position = _point(p.read(component + 0x1A0, 12))
            result['parts'].append({'role': 'body' if tag in BODY_TAGS else 'torso',
                                    'handle': unit_handle, 'address': component, 'position': position})
        _require(result['parts'], 'no supported physical component')
        # Positions can move while reading. Only identity and liveness links must
        # remain unchanged; requiring byte-identical transforms rejects motion.
        for part in result['parts']:
            _require(self.object(part['handle']) == part['address'], 'physical object changed')
            _require(p.u(part['address'] + 0x2C, 'I') == transform, 'physical entity changed')
        current = p.read(ent + 0x28, 0x2C)
        _require(current == raw[0x28:0x54], 'member links/state changed')
        _require(p.u(state + 0x274, 'I') == state_value and p.u(sense_at, 'I') == sense_value,
                 'member sense state changed')
        _require(p.u(entity + 0x54, 'I') == parent and self.resolve(parent) == root, 'parent changed')
        _require(self.pair(pair) == handle, 'member pair changed')
        return result

    def snapshot(self, group=None):
        """Return squad diagnostics and physical targets, or raise Unavailable.

        Missing members/bindings stay in errors. Never use an empty target list as
        a kill gate. `active` is native membership, not proof of positive health.
        """
        p = self.p
        identity = self.content()
        content = identity[1]
        start, count = struct.unpack('<hh', p.read(content + 0x2BD1C + 4, 4))
        _require(0 <= start < 8192 and 0 <= count <= 512, 'invalid squad component list')
        squads, targets = [], []
        for j in range(count):
            dx = p.u(content + 0x2A51C + 2 * (start + j), 'H')
            _require(dx != 0xFFFF, 'absent squad component entry')
            rec = content + 0x510 + dx * 0x38
            comp = self.resolve(p.u(rec + 16, 'I')) + p.u(rec + 24, 'Q')
            sensor_handle = p.u(comp + 0x1A8, 'I')
            sensor = self.resolve(sensor_handle)
            name = p.read(sensor, 16)
            h, kind, index, tb, ta = struct.unpack('<IBxhII', name)
            _require(kind == 1 and tb == 0x80800357 and ta == 0x8080071E, 'wrong squad schema')
            if group is not None and h != group:
                continue
            n = p.u(comp + 0x278, 'i')
            _require(0 <= n <= 32, 'invalid squad member count')
            pairs = p.read(comp + 0x27C, n * 8) if n else b''
            row = {'group': h, 'index': index, 'member_count': n, 'members': [], 'errors': []}
            for i in range(n):
                pair = pairs[i * 8:(i + 1) * 8]
                try:
                    member = self.member(pair)
                    row['members'].append(member)
                except (OSError, Unavailable) as exc:
                    row['errors'].append({'pair_index': i, 'reason': str(exc)})
            _require(p.u(comp + 0x1A8, 'I') == sensor_handle and p.read(sensor, 16) == name,
                     'squad identity changed')
            _require(p.u(comp + 0x278, 'i') == n and
                     (p.read(comp + 0x27C, n * 8) if n else b'') == pairs, 'squad list changed')
            squads.append(row)
            for member in row['members']:
                if member['active'] and member.get('sense_matches') and member['parts']:
                    targets.append({'group': h, 'squad': index, **member})
        _require(self.content() == identity, 'current content changed')
        return {'unix': time.time(), 'content_index': identity[0], 'squads': squads, 'targets': targets}
