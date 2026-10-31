# SPDX-License-Identifier: GPL-2.0-or-later
"""Compile the enabled patches of launcher patch manifests into the native loader's patch file.

    python3 native/guest_patches.py <manifest.xml> [<manifest.xml> ...] --eboot native/out/eboot.elf \
        --out native/out/patches.bin [--only "patch name" ...] [--list]

The first manifest is the one the launcher gives the emulator (apps/launcher/resources/
CUSA00219-lab-manifest-T1.xml and its variants; the reasons for each patch are written in it).
Further manifests in the same format add what the emulator fork installs from its own modules
and the native loader takes as writes (native/guest/). A line there can carry `Original`, the
bytes it replaces: they are compared with the eboot before anything is written.
The same client modifications apply to the native client, which runs the same eboot: without
"accept UNSIGNED cdn content manifest" the game downloads the private server's manifest and
refuses it, and without the two diagnostics patches it prints none of its own state.

Address convention, the manifest's own: Address = eboot virtual address + 0x400000. The native
image places eboot virtual address 0 at image offset 0 (bbport's own patch compiler, upstream,
says the same), so a write lands at Address - 0x400000. The output is that loader's BBPATCH2
format, applied by `probe --patches`.

What is checked here:
  - the plaintext eboot is the pinned 01.29 one. The patches were derived for exactly those
    bytes; nothing else in the manifest says what they replace;
  - a patch entry is for application version 01.29 and is a literal byte write;
  - every write lies inside the file-backed part of one loadable segment, and no two overlap.
The native linker rewrites the game's `mov rax, fs:[0]` to `gs:[0]`. A patch that carries that
instruction is rewritten the same way and reported, since its code would otherwise read the
wrong thread pointer.
"""
import argparse
import hashlib
import struct
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

EBOOT_BASE = 0x400000
EBOOT_SHA256 = '7271fcb926401df8defb126cb8eb2b247134138b81e401bbacb2ab60791b7954'  # as native/prepare.py
APP_VERSION = '01.29'
FS_THREAD_POINTER = bytes.fromhex('64488b042500000000')
GS_THREAD_POINTER = bytes.fromhex('65488b042500000000')


def loadable_segments(elf):
    """(virtual address, file offset, file size) of each PT_LOAD segment."""
    if elf[:4] != b'\x7fELF':
        sys.exit('The eboot is not a plaintext ELF')
    phoff, = struct.unpack_from('<Q', elf, 0x20)
    phentsize, phnum = struct.unpack_from('<HH', elf, 0x36)
    segments = []
    for index in range(phnum):
        kind, _flags, offset, vaddr, _paddr, filesz, _memsz, _align = struct.unpack_from(
            '<IIQQQQQQ', elf, phoff + index * phentsize)
        if kind == 1 and filesz:
            segments.append((vaddr, offset, filesz))
    return segments


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    parser.add_argument('manifest', type=Path, nargs='+')
    parser.add_argument('--eboot', type=Path, required=True)
    parser.add_argument('--out', type=Path)
    parser.add_argument('--only', action='append', default=[],
                        help='apply only the enabled patch with this exact name (repeatable)')
    parser.add_argument('--skip-containing', action='append', default=[], metavar='TEXT',
                        help='leave out enabled patches whose name contains this text (repeatable); '
                             'at least one patch must match each')
    parser.add_argument('--list', action='store_true', help='print the writes and stop')
    args = parser.parse_args()

    elf = args.eboot.read_bytes()
    digest = hashlib.sha256(elf).hexdigest()
    if digest != EBOOT_SHA256:
        sys.exit(f'{args.eboot} is not the pinned 01.29 eboot (SHA-256 {digest})')
    segments = loadable_segments(elf)

    entries = [entry for manifest in args.manifest
               for entry in ET.parse(manifest).getroot().iter('Metadata')]
    names = [entry.get('Name') for entry in entries]
    manifests = ', '.join(manifest.name for manifest in args.manifest)
    for wanted in args.only:
        if wanted not in names:
            sys.exit(f'No patch named {wanted!r} in {manifests}')
    for text in args.skip_containing:
        left_out = [name for entry, name in zip(entries, names)
                    if entry.get('isEnabled') == 'true' and text in name]
        if not left_out:
            sys.exit(f'No enabled patch in {manifests} has {text!r} in its name')
        for name in left_out:
            print(f'left out: {name}')
    writes = []
    for entry in entries:
        name = entry.get('Name')
        if entry.get('isEnabled') != 'true' or (args.only and name not in args.only):
            continue
        if any(text in name for text in args.skip_containing):
            continue
        if entry.get('AppVer') != APP_VERSION:
            sys.exit(f'Patch {name!r} is for application version {entry.get("AppVer")}, not {APP_VERSION}')
        for line in entry.iter('Line'):
            if line.get('Type') != 'bytes':
                sys.exit(f'Patch {name!r} has a {line.get("Type")!r} line; only literal bytes are supported')
            offset = int(line.get('Address'), 16) - EBOOT_BASE
            data = bytes.fromhex(line.get('Value'))
            rewritten = data.replace(FS_THREAD_POINTER, GS_THREAD_POINTER)
            home = [segment for segment in segments
                    if segment[0] <= offset and offset + len(data) <= segment[0] + segment[2]]
            if not data or not home:
                sys.exit(f'Patch {name!r}: {len(data)} bytes at {offset:#x} are not inside one loadable segment')
            vaddr, file_offset, _size = home[0]
            start = file_offset + offset - vaddr
            original = elf[start:start + len(data)]
            if line.get('Original') is not None and original != bytes.fromhex(line.get('Original')):
                sys.exit(f'Patch {name!r}: the eboot has {original.hex()} at {offset:#x}, '
                         f'not {line.get("Original")}')
            writes.append((offset, rewritten, name, original, rewritten != data))
    writes.sort(key=lambda write: write[0])
    for (first, first_data, first_name, _, _), (second, _, second_name, _, _) in zip(writes, writes[1:]):
        if first + len(first_data) > second:
            sys.exit(f'Patches {first_name!r} and {second_name!r} overlap at {second:#x}')
    if not writes:
        sys.exit(f'{manifests} has no enabled patch to apply')

    for offset, data, name, original, tls in writes:
        shown = data.hex() if len(data) <= 16 else f'{data[:16].hex()}... ({len(data)} bytes)'
        was = original.hex() if len(original) <= 16 else f'{original[:16].hex()}...'
        print(f'{offset:#9x}  {was} -> {shown}  {name}' + ('  [fs: thread pointer rewritten to gs:]' if tls else ''))
    if args.list:
        return
    if not args.out:
        sys.exit('Give --out, or --list to only print')
    blob = struct.pack('<8sQQ', b'BBPATCH2', EBOOT_BASE, len(writes))
    for offset, data, _name, _original, _tls in writes:
        blob += struct.pack('<QQ', offset, len(data)) + data
    args.out.write_bytes(blob)
    print(f'{len(writes)} writes from {manifests} -> {args.out}')


if __name__ == '__main__':
    main()
