# SPDX-License-Identifier: GPL-2.0-or-later
"""Owned 56578 options-definition tags: bounded reader and append-only video-row candidate.

This does not install UI graphs, strings or executable hooks. Outputs contain owned game data
and must stay private. Existing options keep their definitions and choice order.
"""
import struct

ARRAY = 0x80800184


def fnv(text):
    h = 0x811c9dc5
    for c in text.encode('utf8'):
        h = ((h ^ c) * 0x1000193) & 0xffffffff
    return h


def array(data, field, stride):
    if field < 0 or field + 16 > len(data):
        raise ValueError('array field outside tag')
    n, rel = struct.unpack_from('<Qq', data, field)
    if not n and not rel:
        return []
    head = field + 8 + rel
    if n > 4096 or head < 4 or head + 16 + n * stride > len(data):
        raise ValueError('array outside tag')
    if struct.unpack_from('<I', data, head - 4)[0] != ARRAY or struct.unpack_from('<Q', data, head)[0] != n:
        raise ValueError('array marker or repeated count disagrees')
    return [head + 16 + i * stride for i in range(n)]


class Settings:
    def __init__(self, data):
        self.data = bytes(data)
        if len(data) < 0x28 or struct.unpack_from('<Q', data)[0] != len(data):
            raise ValueError('options tag self-length disagrees')
        self.groups = array(data, 8, 40)
        self.rows = array(data, 0x18, 40)
        hashes = [self.u32(o) for o in self.groups]
        ids = [self.u32(o) for o in self.rows]
        if hashes != sorted(set(hashes)) or ids != sorted(ids):
            raise ValueError('options must be sorted for native binary search')
        row_set = set(self.rows)
        for o in self.rows:
            array(data, o + 24, 16)
        for o in self.groups:
            for p in array(data, o + 24, 8):
                if p + struct.unpack_from('<q', data, p)[0] not in row_set:
                    raise ValueError('group references a row outside the active settings array')

    def u32(self, off):
        return struct.unpack_from('<I', self.data, off)[0]

    def summary(self):
        return {'groups': len(self.groups), 'settings': len(self.rows),
                'ids': [self.u32(o) for o in self.rows]}


def video_candidate(source, template_group):
    original = Settings(source)
    # Caller identifies the decoded Helmet group. Native group identifiers are authored hashes;
    # do not assume that hashing its displayed English label reproduces its identifier.
    helmet = [o for o in original.groups if original.u32(o) == template_group]
    if len(helmet) != 1:
        raise ValueError('expected one native helmet group')
    refs = array(source, helmet[0] + 24, 8)
    if len(refs) != 1:
        raise ValueError('helmet must have one setting')
    template = refs[0] + struct.unpack_from('<q', source, refs[0])[0]
    choices = array(source, template + 24, 16)
    if not choices:
        raise ValueError('helmet must have cycling choices')
    highest = max(original.u32(o) for o in original.rows)
    if highest > 125:
        raise ValueError('no two spare native signed-byte setting ids')
    labels = {}

    def label(name, text):
        h = fnv('d1reimagined.video.' + name)
        if h in labels and labels[h] != text:
            raise ValueError('new string hash collision')
        labels[h] = text
        return h

    group_hash = fnv('d1reimagined_video')
    if any(original.u32(o) == group_hash for o in original.groups):
        raise ValueError('video group already present')
    specs = [
        (highest + 1, label('frame_rate', 'Frame Rate'), label('frame_rate_help', 'Applies immediately.'),
         [(i, label('fps_' + str(rate), str(rate) if rate else 'Uncapped'))
          for i, rate in enumerate((30, 60, 100, 144, 240, 0))]),
        (highest + 2, label('resolution', 'Resolution'),
         label('resolution_help', 'Applies when the game next starts.'),
         [(i, label('res_' + str(w), f'{w} x {h}'))
          for i, (w, h) in enumerate(((1920, 1080), (2560, 1440), (3840, 2160)))])]
    out = bytearray(source)

    def append_array(field, count, stride, element_class):
        # Preserve the original file, append a complete typed array, then repoint its owner.
        out.extend(b'\0' * ((-len(out)) % 8))
        head = len(out) + 8
        out.extend(struct.pack('<IIQII', 0, ARRAY, count, element_class, 0))
        start = len(out)
        out.extend(b'\0' * (count * stride))
        struct.pack_into('<Qq', out, field, count, head - (field + 8))
        return [start + i * stride for i in range(count)]

    def clone_record(src, dst):
        out[dst:dst + 40] = source[src:src + 40]
        count, rel = struct.unpack_from('<Qq', source, src + 24)
        if rel:
            struct.pack_into('<q', out, dst + 32, src + 32 + rel - (dst + 32))

    # The native class ids are copied from the verified input's typed array headers.
    def element_class(field):
        _, rel = struct.unpack_from('<Qq', source, field)
        return struct.unpack_from('<I', source, field + 8 + rel + 8)[0]

    rows = append_array(0x18, len(original.rows) + 2, 40, element_class(0x18))
    moved = dict(zip(original.rows, rows))
    for src, dst in moved.items():
        clone_record(src, dst)
    new_rows = rows[-2:]
    for dst, (sid, title, help_, values) in zip(new_rows, specs):
        clone_record(template, dst)
        struct.pack_into('<III', out, dst, sid, title, help_)
        new_choices = append_array(dst + 24, len(values), 16, element_class(template + 24))
        for pos, (value, text) in zip(new_choices, values):
            struct.pack_into('<IIII', out, pos, value, text, help_, fnv(''))

    group_items = sorted([(original.u32(o), o) for o in original.groups] + [(group_hash, None)])
    groups = append_array(8, len(group_items), 40, element_class(8))
    for dst, (hash_, src) in zip(groups, group_items):
        clone_record(helmet[0] if src is None else src, dst)
        if src is None:
            struct.pack_into('<III', out, dst, hash_, label('title', 'Video'),
                             label('help', 'Choose frame rate and render resolution.'))
            targets = new_rows
        else:
            targets = [moved[p + struct.unpack_from('<q', source, p)[0]]
                       for p in array(source, src + 24, 8)]
        if targets:
            members = append_array(dst + 24, len(targets), 8, element_class(helmet[0] + 24))
            for pos, target in zip(members, targets):
                struct.pack_into('<q', out, pos, target - pos)
        else:
            struct.pack_into('<QQ', out, dst + 24, 0, 0)
    struct.pack_into('<Q', out, 0, len(out))
    check = Settings(out)
    if len(check.groups) != len(original.groups) + 1 or len(check.rows) != len(original.rows) + 2:
        raise ValueError('candidate count disagreement')
    return bytes(out), {'settings': check.summary(), 'group_hash': f'{group_hash:08x}',
                        'setting_ids': {'frame_rate': highest + 1, 'resolution': highest + 2},
                        'strings': {f'{k:08x}': v for k, v in labels.items()},
                        'installed': False, 'native_ui_verified': False}


def video_graph_candidate(source, template_group, video_group):
    """Append a named group binding to the same native list as the template group.

    The graph's named-property table supplies type-4 path tokens. Bindings point to path
    descriptors through self-relative pointers; moving either array must fix those pointers.
    No imported template or existing node id is changed.
    """
    if len(source) < 0x158 or struct.unpack_from('<Q', source)[0] != len(source):
        raise ValueError('graph self-length disagrees')
    names = array(source, 0xc8, 4)
    hashes = [struct.unpack_from('<I', source, o)[0] for o in names]
    if hashes.count(template_group) != 1 or video_group in hashes:
        raise ValueError('template name missing/ambiguous, or video already present')
    paths = array(source, 0x48, 16)
    bindings = array(source, 0x138, 64)

    def pointer(field):
        target = field + struct.unpack_from('<q', source, field)[0]
        if not 0 <= target <= len(source) - 16:
            raise ValueError('binding path outside graph')
        return target

    matches = []
    for b in bindings:
        path = pointer(b + 8)
        tokens = array(source, path, 8)
        if len(tokens) == 1 and struct.unpack_from('<HHI', source, tokens[0]) == (4, hashes.index(template_group), 0):
            matches.append((b, path))
    if len(matches) != 1:
        raise ValueError('expected one direct template-group binding')
    template, template_path = matches[0]
    if template_path not in paths:
        raise ValueError('template path absent from active path array')
    target_node = struct.unpack_from('<Q', source, template + 24)[0]
    slots = [struct.unpack_from('<Q', source, b + 40)[0] for b in bindings
             if struct.unpack_from('<Q', source, b + 24)[0] == target_node]
    if not slots or any(v & 0xffff for v in slots):
        raise ValueError('target is not a flat indexed native list')
    new_slot = max(slots) + 0x10000
    out = bytearray(source)

    def append(field, count, stride, old_field):
        _, rel = struct.unpack_from('<Qq', source, old_field)
        cls = struct.unpack_from('<I', source, old_field + 8 + rel + 8)[0]
        out.extend(b'\0' * ((-len(out)) % 8))
        head = len(out) + 8
        out.extend(struct.pack('<IIQII', 0, ARRAY, count, cls, 0))
        start = len(out)
        out.extend(b'\0' * (count * stride))
        struct.pack_into('<Qq', out, field, count, head - field - 8)
        return [start + i * stride for i in range(count)]

    new_names = append(0xc8, len(names) + 1, 4, 0xc8)
    for pos, value in zip(new_names, hashes + [video_group]):
        struct.pack_into('<I', out, pos, value)
    new_paths = append(0x48, len(paths) + 1, 16, 0x48)
    moved_paths = dict(zip(paths, new_paths))
    for old, pos in moved_paths.items():
        count, rel = struct.unpack_from('<Qq', source, old)
        struct.pack_into('<Qq', out, pos, count, old + 8 + rel - pos - 8 if rel else 0)
    token, = append(new_paths[-1], 1, 8, template_path)
    struct.pack_into('<HHI', out, token, 4, len(names), 0)
    new_bindings = append(0x138, len(bindings) + 1, 64, 0x138)
    for old, pos in zip(bindings + [template], new_bindings):
        out[pos:pos + 64] = source[old:old + 64]
        for off in (8, 32):
            target = pointer(old + off)
            struct.pack_into('<q', out, pos + off, moved_paths.get(target, target) - pos - off)
    struct.pack_into('<q', out, new_bindings[-1] + 8, new_paths[-1] - new_bindings[-1] - 8)
    struct.pack_into('<Q', out, new_bindings[-1] + 40, new_slot)
    struct.pack_into('<Q', out, 0, len(out))
    # Re-read all moved descriptors and every binding path before producing a candidate.
    for pos in array(out, 0x48, 16):
        array(out, pos, 8)
    for pos in array(out, 0x138, 64):
        for off in (8, 32):
            target = pos + off + struct.unpack_from('<q', out, pos + off)[0]
            array(out, target, 8)
    return bytes(out), {'source_node': struct.unpack_from('<Q', source, template)[0],
                        'target_list_node': target_node, 'target_slot': new_slot >> 16,
                        'named_property_index': len(names), 'bindings': len(new_bindings),
                        'installed': False, 'native_ui_verified': False}
