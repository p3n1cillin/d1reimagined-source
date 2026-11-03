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
    if highest > 252:
        raise ValueError('no two spare native byte setting ids')
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
