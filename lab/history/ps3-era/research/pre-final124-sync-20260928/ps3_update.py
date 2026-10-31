"""PS3 title updates of Destiny (BLES01857 / BLUS31181) from Sony's update server: the patch list,
and selected files out of each update PKG over HTTP range requests, so the 9 GB cumulative patch
never has to be downloaded whole.

A retail PS3 PKG has a 0xC0-byte header (+0x14 file count, +0x20 data offset, +0x30 content id,
+0x70 IV) and an AES-128-CTR encrypted data area (the common PS3 PKG key) that starts with the
file table: 32-byte entries {u32 name offset, u32 name size, u64 data offset, u64 data size,
u32 flags, u32}. Offsets are relative to the data area.

  python scripts/ps3_update.py list BLES01857
  python scripts/ps3_update.py files BLES01857 01.24
  python scripts/ps3_update.py fetch BLES01857 --versions 01.13,01.24 --match EBOOT.BIN,PARAM.SFO --out DIR
  python scripts/ps3_update.py fetch BLES01857 --packages --out DIR   # every game package, in order

`fetch` writes DIR/<version>/<path in the PKG>; with --packages it also writes the final package
set to DIR/final/packages: later updates replace files of the same name, as the console's install
does. Update 01.20 restarted the package patch numbering, so files named _N by an older update
can outlive their replacement; `fetch` lists such files in DIR/final/stale.txt.
"""

import argparse
import hashlib
import json
import re
import struct
import sys
import urllib.request
import xml.etree.ElementTree as ET
from pathlib import Path

from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes

UPDATE_LIST = "https://a0.ww.np.dl.playstation.net/tpl/np/{title}/{title}-ver.xml"
PKG_KEY = bytes.fromhex("2E7B71D7C9C9A14EA3221F188828B8F8")  # the retail PS3 PKG key (public)
CHUNK = 1 << 24


def http_range(url, start, length):
    request = urllib.request.Request(url, headers={"Range": f"bytes={start}-{start + length - 1}"})
    with urllib.request.urlopen(request, timeout=120) as response:
        data = response.read()
    if len(data) != length:
        raise IOError(f"short read from {url}: {len(data)} of {length} bytes")
    return data


def updates(title):
    """[(version, url, size, sha1)] from Sony's update list, oldest first."""
    import ssl
    context = ssl.create_default_context()
    context.check_hostname, context.verify_mode = False, ssl.CERT_NONE  # Sony's own CA chain
    with urllib.request.urlopen(UPDATE_LIST.format(title=title), timeout=60, context=context) as response:
        root = ET.fromstring(response.read())
    out = [(p.get("version"), p.get("url"), int(p.get("size")), p.get("sha1sum")) for p in root.iter("package")]
    return sorted(out)


class RemotePkg:
    def __init__(self, url, reader=http_range):
        self.url, self.read = url, reader
        head = self.read(url, 0, 0xC0)
        if head[:4] != b"\x7fPKG":
            raise ValueError(f"{url}: not a PKG")
        self.count = struct.unpack_from(">I", head, 0x14)[0]
        self.data_offset = struct.unpack_from(">Q", head, 0x20)[0]
        self.content_id = head[0x30:0x60].rstrip(b"\0").decode()
        self.iv = int.from_bytes(head[0x70:0x80], "big")
        table = self.decrypt(0, self.count * 32)
        entries = [struct.unpack_from(">IIQQI", table, 32 * i) for i in range(self.count)]
        names_end = max(n + s for n, s, *_ in entries)
        names = self.decrypt(0, names_end)
        self.files = [(names[n:n + s].decode("latin-1"), off, size, flags & 0xFF)
                      for n, s, off, size, flags in entries]

    def decrypt(self, offset, length):
        start = offset - offset % 16
        raw = self.read(self.url, self.data_offset + start, length + offset % 16)
        counter = ((self.iv + start // 16) % (1 << 128)).to_bytes(16, "big")
        plain = Cipher(algorithms.AES(PKG_KEY), modes.CTR(counter)).decryptor().update(raw)
        return plain[offset % 16:offset % 16 + length]

    def extract(self, offset, size, target):
        target.parent.mkdir(parents=True, exist_ok=True)
        with open(target, "wb") as f:
            for pos in range(0, size, CHUNK):
                f.write(self.decrypt(offset + pos, min(CHUNK, size - pos)))


def param_sfo(data):
    if data[:4] != b"\0PSF":
        return {}
    keys, values, count = struct.unpack_from("<III", data, 8)
    out = {}
    for i in range(count):
        key_off, fmt, length, _, value_off = struct.unpack_from("<HHIII", data, 0x14 + 16 * i)
        key = data[keys + key_off:data.index(b"\0", keys + key_off)].decode()
        raw = data[values + value_off:values + value_off + length]
        out[key] = struct.unpack("<I", raw)[0] if fmt == 0x0404 else raw.rstrip(b"\0").decode("latin-1")
    return out


def package_name(path):
    return path.rsplit("/", 1)[-1]


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("list"); s.add_argument("title")
    s = sub.add_parser("files"); s.add_argument("title"); s.add_argument("version")
    s = sub.add_parser("fetch"); s.add_argument("title"); s.add_argument("--out", type=Path, required=True)
    s.add_argument("--versions", help="comma-separated (default: all)")
    s.add_argument("--match", default="EBOOT.BIN,PARAM.SFO", help="comma-separated file names to extract")
    s.add_argument("--packages", action="store_true", help="also every game package (large: ~12 GB)")
    a = ap.parse_args(argv)
    chain = updates(a.title)
    if a.cmd == "list":
        for version, url, size, sha1 in chain:
            print(f"{version}  {size / 1e6:10.1f} MB  {url}")
        return
    if a.cmd == "files":
        url = next(u for v, u, *_ in chain if v == a.version)
        pkg = RemotePkg(url)
        print(pkg.content_id, len(pkg.files), "files")
        for name, off, size, flags in pkg.files:
            print(f"{size:12d}  {name}")
        return
    wanted = set(a.versions.split(",")) if a.versions else None
    match = {m for m in a.match.split(",") if m}
    summary, final = {}, {}
    for version, url, size, sha1 in chain:
        if wanted and version not in wanted and not a.packages:
            continue
        pkg = RemotePkg(url)
        info = {"content_id": pkg.content_id, "files": len(pkg.files)}
        for name, off, fsize, flags in pkg.files:
            base = package_name(name)
            if flags == 4 or fsize == 0:
                continue
            if (wanted is None or version in wanted) and base in match:
                target = a.out / version / name
                pkg.extract(off, fsize, target)
                if base == "PARAM.SFO":
                    info["param_sfo"] = param_sfo(target.read_bytes())
                print(f"{version}: {name} ({fsize} bytes)")
            if a.packages and base.endswith(".pkg"):
                target = a.out / "final" / "packages" / base
                pkg.extract(off, fsize, target)
                final[base] = version
                print(f"{version}: {base} ({fsize} bytes)")
        summary[version] = info
    a.out.mkdir(parents=True, exist_ok=True)
    (a.out / "updates.json").write_text(json.dumps(summary, indent=1), encoding="utf-8")
    if final:
        # an id whose files were renumbered: an older update's _N above the newest update's _M
        stale = []
        by_id = {}
        for base, version in final.items():
            m = re.match(r"(.+)_(\d+)\.pkg$", base)
            if m:
                by_id.setdefault(m.group(1), []).append((int(m.group(2)), version, base))
        for group in by_id.values():
            newest = max(v for _, v, _ in group)
            stale += [b for n, v, b in group if v < newest and n > max(n2 for n2, v2, _ in group if v2 == newest)]
        (a.out / "final" / "stale.txt").write_text("\n".join(sorted(stale)) + "\n", encoding="utf-8")
        # which update supplied each final file: the input for a final-build content manifest
        (a.out / "final" / "packages.json").write_text(json.dumps(final, indent=0, sort_keys=True), encoding="utf-8")
        print(f"{len(final)} packages in {a.out / 'final' / 'packages'}; {len(stale)} stale (see stale.txt)")


if __name__ == "__main__":
    main()
