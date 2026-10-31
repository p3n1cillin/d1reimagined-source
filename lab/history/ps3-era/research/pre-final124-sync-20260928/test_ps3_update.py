import struct
import sys
import unittest
from pathlib import Path

from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes

sys.path.insert(0, str(Path(__file__).resolve().parent))
import ps3_update  # noqa: E402


def build_pkg(files):
    """A synthetic retail PKG: header, then the CTR-encrypted table, names and data."""
    iv = bytes(range(16))
    table, names, blob = b"", b"", b""
    header_len = 32 * len(files)
    name_base = header_len
    name_blob = b"".join(n.encode().ljust((len(n) + 15) // 16 * 16, b"\0") for n, _ in files)
    data_base = name_base + len(name_blob)
    offsets, pos = [], 0
    for name, data in files:
        offsets.append(data_base + pos)
        pos += (len(data) + 15) // 16 * 16
    npos = name_base
    for (name, data), off in zip(files, offsets):
        table += struct.pack(">IIQQII", npos, len(name), off, len(data), 3, 0)
        npos += (len(name) + 15) // 16 * 16
    body = table + name_blob + b"".join(d.ljust((len(d) + 15) // 16 * 16, b"\0") for _, d in files)
    enc = Cipher(algorithms.AES(ps3_update.PKG_KEY), modes.CTR(iv)).encryptor().update(body)
    head = bytearray(0xC0)
    head[:4] = b"\x7fPKG"
    struct.pack_into(">I", head, 0x14, len(files))
    struct.pack_into(">Q", head, 0x20, 0xC0)
    head[0x30:0x30 + 12] = b"EP0002-TEST0"
    head[0x70:0x80] = iv
    return bytes(head) + enc


class RemotePkgTest(unittest.TestCase):
    def test_lists_and_decrypts_files(self):
        sfo_payload = b"x" * 37
        pkg = build_pkg([("PARAM.SFO", sfo_payload), ("USRDIR/EBOOT.BIN", bytes(range(200)))])
        reader = lambda url, start, length: pkg[start:start + length]
        remote = ps3_update.RemotePkg("mem://pkg", reader)
        self.assertEqual([f[0] for f in remote.files], ["PARAM.SFO", "USRDIR/EBOOT.BIN"])
        name, off, size, _ = remote.files[1]
        self.assertEqual(remote.decrypt(off + 5, 20), bytes(range(5, 25)))  # an unaligned range


if __name__ == "__main__":
    unittest.main()
