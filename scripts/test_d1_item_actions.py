"""d1-item-actions.py's action-block decoder, on synthetic definitions (no game data)."""
import importlib.util
import struct
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('d1_item_actions', ROOT/'tools/ps4_setup/d1-item-actions.py')
actions = importlib.util.module_from_spec(spec)
spec.loader.exec_module(actions)


def definition(action=True, kind=0xB2084C93, reward_set=1117, marker=0x80801916):
    """A 56578-shaped item definition: the action block at +0xA0, its class just before it."""
    d = bytearray(0x110)
    struct.pack_into('<I', d, 0, len(d))
    struct.pack_into('<I', d, 0x78, 0x1CBA8A98)
    d[0x88:0x8C] = bytes([2, 0xFF, 1, 1])
    if action:
        struct.pack_into('<q', d, 0x08, 0xA0 - 0x08)
        struct.pack_into('<I', d, 0x9C, marker)
        struct.pack_into('<I', d, 0xA0 + 0x38, kind)
        struct.pack_into('<Hh', d, 0xA0 + 0x40, reward_set, -1)
    return bytes(d)


class ActionBlock(unittest.TestCase):
    def test_shard_is_fnv1_of_the_dismantle_name(self):
        self.assertEqual(actions.fnv1('shard'), 0xB2084C93)
        self.assertEqual(actions.ACTIONS[0xB2084C93], 'shard')

    def test_a_dismantle_names_its_reward_set(self):
        item_hash, bucket, tier, instanced, act = actions.decode(definition())
        self.assertEqual((item_hash, bucket, tier, instanced), (0x1CBA8A98, 2, 1, 1))
        self.assertEqual(act, (0xB2084C93, 1117, -1, 0))

    def test_no_action_block(self):
        self.assertIsNone(actions.decode(definition(action=False))[4])

    def test_a_wrong_class_before_the_block_is_refused(self):
        with self.assertRaises(ValueError):
            actions.decode(definition(marker=0x80801806))


if __name__ == '__main__':
    unittest.main()
