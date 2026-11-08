"""List every item definition's action block: what dismantling (or using) each item runs.

    d1-item-actions.py OUT.tsv [--items CONTENT_INDEX,...] [--index item-content-index.tsv] [--jobs N]

Each item definition (class `0x80801806`) points at an action block (class `0x80801916`; Blam
Network's 2014 live definitions name it `s_item_action_block`). Its `action_type_hash` is FNV-1 of
the action's name: `0xB2084C93` = "shard", the dismantle. Next to it sit a u16 that the client hands
to its reward resolver with `reward_item` (0xFFFF for none) and `reward_item` itself, so what a
dismantle gives back is named by the client data, per item, not invented by a server.

56578 layout (64-bit offsets; read from the Khvostov, content 3598, definition `0x80A64722`):
    def +0x08  i64 offset (from +0x08) to the action block
    def +0x78  u32 item hash          def +0x88 u8 bucket, +0x8A u8 tier, +0x8B u8 instanced
    action +0x28 progression rewards (u64 count, i64 offset)   action +0x38 u32 action type hash
    action +0x40 u16 reward set       action +0x42 i16 reward item
The item table (class `0x80801933`) holds 24-byte rows {u32 hash, u32 0, u64 0, u32 definition tag,
u32 0}, not in content-index order: the content index comes from capture-rig/item-content-index.tsv
(`d1-oracle item-index`), joined on the item hash.
"""
import argparse
import hashlib
import json
import concurrent.futures
import os
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import d1paths  # sibling: local paths from D1_ROOT or paths.json

ITEM_TABLE = '0x80a5ffbe'
ROW, FIRST_ROW = 24, 0xF30
ACTIONS = {0xB2084C93: 'shard'}


def reward_table_status(root):
    """56578: 26EEF4 -> 26E950 -> 2799E0 -> vfunc 1F0 (25EBB0).

    That getter reads globals +3E8. FFFFFFFF is an absent tag, not zero rewards.
    This is a target-build field, not the unrelated records table at globals +18.
    """
    if len(root) < 0x3EC or struct.unpack_from('<Q', root)[0] != len(root):
        raise ValueError('invalid 56578 investment globals root')
    tag = struct.unpack_from('<I', root, 0x3E8)[0]
    return dict(build='56578.17.04.12.1251.evil', root_tag=0x80A58001,
                root_sha256=hashlib.sha256(root).hexdigest(), field='0x3e8',
                reward_table_tag=None if tag == 0xFFFFFFFF else tag,
                status='absent' if tag == 0xFFFFFFFF else 'present; amounts not decoded')


def fnv1(s):
    h = 0x811C9DC5
    for b in s.encode():
        h = (h * 0x01000193) & 0xFFFFFFFF
        h ^= b
    return h


for name in ('decrypt', 'use', 'equip', 'open', 'claim', 'redeem', 'consume', 'unlock', 'read'):
    ACTIONS.setdefault(fnv1(name), name)


def oracle():
    return os.environ.get('D1_ORACLE') or str(d1paths.workspace() / 'd1-server' / 'target' / 'release' / 'd1-oracle.exe')


def entry(tag, tmp):
    out = os.path.join(tmp, f'{tag}.bin')
    r = subprocess.run([oracle(), 'oodle', 'entry', '--tag', tag, '-o', out, '--hex', '0'], capture_output=True, text=True)
    if r.returncode or not os.path.exists(out):
        raise RuntimeError(f'd1-oracle oodle entry {tag} failed: {r.stdout[-300:]} {r.stderr[-300:]}')
    with open(out, 'rb') as f:
        return f.read()


def decode(d):
    """The action block's fields from one item definition's bytes."""
    off = struct.unpack_from('<q', d, 0x08)[0]
    item_hash = struct.unpack_from('<I', d, 0x78)[0]
    bucket, _, tier, instanced = d[0x88:0x8C]
    if not off:
        return item_hash, bucket, tier, instanced, None
    a = 0x08 + off
    if struct.unpack_from('<I', d, a - 4)[0] != 0x80801916:
        raise ValueError(f'no action-block class before +{a:#x}')
    rewards = struct.unpack_from('<Q', d, a + 0x28)[0]
    kind = struct.unpack_from('<I', d, a + 0x38)[0]
    reward_set, reward_item = struct.unpack_from('<Hh', d, a + 0x40)
    return item_hash, bucket, tier, instanced, (kind, reward_set, reward_item, rewards)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('out', nargs='?')
    ap.add_argument('--reward-table-status', action='store_true',
                    help='report the optional 56578 reward table reference, without scanning item definitions')
    ap.add_argument('--items', help='content indices (comma-separated); default: every row')
    ap.add_argument('--index', default=str(d1paths.workspace() / 'capture-rig' / 'item-content-index.tsv'))
    ap.add_argument('--jobs', type=int, default=8)
    a = ap.parse_args()
    if a.reward_table_status:
        pinned = json.loads((Path(__file__).resolve().parents[2] / 'ps4/capture-rig/builds.json').read_text())
        eboot = os.environ.get('D1_EBOOT')
        if not eboot or hashlib.sha256(Path(eboot).read_bytes()).hexdigest() != pinned['builds']['later-56578']['eboot_sha256']:
            ap.error('set D1_EBOOT to the pinned owned 56578 executable')
        os.environ['D1_BUILD'] = 'later-56578'
        with tempfile.TemporaryDirectory() as tmp:
            print(json.dumps(reward_table_status(entry('0x80a58001', tmp)), sort_keys=True))
        return
    if not a.out:
        ap.error('OUT.tsv is required unless --reward-table-status is used')
    content = {}
    with open(a.index, encoding='utf-8') as f:
        for line in f:
            p = line.split('	')
            if len(p) >= 2 and p[0].startswith('0x'):
                content[int(p[1])] = int(p[0], 16)
    with tempfile.TemporaryDirectory() as tmp:
        table = entry(ITEM_TABLE, tmp)
        by_hash = {}
        for i in range((len(table) - FIRST_ROW) // ROW):
            h, _, _, tag = struct.unpack_from('<IIQI', table, FIRST_ROW + ROW * i)
            by_hash[h] = f'{tag:#x}'
        tags = {ci: (h, by_hash[h]) for ci, h in content.items() if h in by_hash}
        want = [int(x) for x in a.items.split(",") if int(x) in tags] if a.items else sorted(tags)

        def one(ci):
            h, tag = tags[ci]
            item_hash, bucket, tier, instanced, act = decode(entry(tag, tmp))
            if item_hash != h:
                raise ValueError(f'content {ci}: definition {tag} names {item_hash:#x}, the table {h:#x}')
            if act is None:
                return f'{ci}\t{h:#010x}\t{tag}\t{bucket}\t{tier}\t{instanced}\t-\t-\t-\t-'
            kind, reward_set, reward_item, rewards = act
            return (f'{ci}\t{h:#010x}\t{tag}\t{bucket}\t{tier}\t{instanced}\t{ACTIONS.get(kind, f"{kind:#010x}")}'
                    f'\t{reward_set if reward_set != 0xFFFF else "-"}\t{reward_item}\t{rewards}')

        with concurrent.futures.ThreadPoolExecutor(a.jobs) as pool:
            lines = list(pool.map(one, want))
    with open(a.out, 'w', encoding='utf-8', newline='\n') as f:
        f.write('# GENERATED by tools/ps4_setup/d1-item-actions.py - each item definition\'s action block\n')
        f.write('content_index\titem_hash\tdefinition_tag\tbucket\ttier\tinstanced\taction\treward_set\treward_item\tprogression_rewards\n')
        f.write('\n'.join(lines) + '\n')
    print(f'{len(lines)} items -> {a.out}')


if __name__ == '__main__':
    main()
