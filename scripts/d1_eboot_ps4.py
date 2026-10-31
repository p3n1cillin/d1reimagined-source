# SPDX-License-Identifier: GPL-2.0-or-later
"""Static analysis of your own decrypted PS4 Destiny executable (CUSA00219 build 56578, x86-64 ELF):
strings, references to an address, callers of a function, the function around an address,
disassembly, and the engine's keyboard table. Read-only; it never attaches to or runs the game.

Function bounds come from the executable's own unwind table (.eh_frame_hdr). Addresses are
executable-relative, as in the project's notes and patches.

  python scripts/d1_eboot_ps4.py strings inactivity
  python scripts/d1_eboot_ps4.py xref 0x1623b7f                 (rip-relative loads of an address)
  python scripts/d1_eboot_ps4.py callers 0xad300
  python scripts/d1_eboot_ps4.py func 0x927d1f
  python scripts/d1_eboot_ps4.py disasm 0x926910 [--count 40]
  python scripts/d1_eboot_ps4.py kbm-table                      (engine key -> player control)

The executable: --elf PATH, else D1_EBOOT, else WORKSPACE/dumps/later-56578-CUSA00219/eboot.elf
(tools/ps4_setup/d1paths.py). Needs `pip install capstone`.
"""

import argparse
import bisect
import hashlib
import os
import re
import struct
import sys
from pathlib import Path

SHA_56578 = '672f03411c0503dfbcda804cbb9fb27cdd9a8d73a8bcb71bf0c4f688073b0833'
EH_FRAME_HDR = 0x1869930          # 56578
# The engine's keyboard layer (notes/ps4-56578-pc-controls.md).
KBM_BUILDER = (0x926910, 0x9284C0)
KEY_FRAMES, KEY_HELD = 0xAD300, 0xAD3A0
MOUSE_FRAMES, MOUSE_HELD = 0xAD6A0, 0xAD6C0
CONTROLS_BASE, CONTROL_STRIDE = 0x2A0, 0x14   # player 0's 21 controls, from the input array
KEY_NAMES = (
    'Esc F1 F2 F3 F4 F5 F6 F7 F8 F9 F10 F11 F12 F13 ScrLk F15 ` 1 2 3 4 5 6 7 8 9 0 - = Back Tab Q W E R T Y '
    'U I O P [ ] \\ Caps A S D F G H J K L ; \' Enter LShift Z X C V B N M , . / RShift LCtrl LWin LAlt '
    'Space RAlt RWin Apps RCtrl Up Down Left Right Ins Home PgUp Del End PgDn NumLk Num/ Num* Num0 Num1 '
    'Num2 Num3 Num4 Num5 Num6 Num7 Num8 Num9 Num- Num+ NumEnter Num.').split()
assert len(KEY_NAMES) == 104


def default_elf():
    explicit = os.environ.get('D1_EBOOT')
    if explicit:
        return Path(explicit)
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools' / 'ps4_setup'))
    import d1paths
    return d1paths.workspace() / 'dumps' / 'later-56578-CUSA00219' / 'eboot.elf'


class Eboot:
    def __init__(self, path):
        from capstone import CS_ARCH_X86, CS_MODE_64, Cs
        self.data = Path(path).read_bytes()
        if hashlib.sha256(self.data).hexdigest() != SHA_56578:
            print(f'warning: {path} is not the 56578 executable these addresses are for', file=sys.stderr)
        b = self.data
        phoff = struct.unpack_from('<Q', b, 0x20)[0]
        size, count = struct.unpack_from('<HH', b, 0x36)
        self.segments = []   # (address, file offset, size, flags)
        for i in range(count):
            kind, flags, off, addr, _, filesz, _, _ = struct.unpack_from('<IIQQQQQQ', b, phoff + i * size)
            if kind == 1:
                self.segments.append((addr, off, filesz, flags))
        self.md = Cs(CS_ARCH_X86, CS_MODE_64)
        self.md.skipdata = True
        n = struct.unpack('<I', self.read(EH_FRAME_HDR + 8, 4))[0]
        self.funcs = sorted(EH_FRAME_HDR + struct.unpack('<i', self.read(EH_FRAME_HDR + 12 + 8 * i, 4))[0]
                            for i in range(n))

    def read(self, va, n):
        for addr, off, size, _ in self.segments:
            if addr <= va and va + n <= addr + size:
                return self.data[off + va - addr:off + va - addr + n]
        raise ValueError(f'{va:#x} is outside the loaded segments')

    def code(self):
        return [(a, self.data[o:o + s]) for a, o, s, f in self.segments if f & 1]

    def cstr(self, va, limit=200):
        try:
            text = self.read(va, limit).split(b'\0', 1)[0].decode('utf-8')
        except (ValueError, UnicodeDecodeError):
            return ''
        return text if text.isprintable() else ''

    def func(self, va):
        i = bisect.bisect_right(self.funcs, va) - 1
        if i < 0:
            return None, None
        return self.funcs[i], self.funcs[i + 1] if i + 1 < len(self.funcs) else None

    def disasm(self, va, count):
        out = []
        for a, size, mnemonic, ops in self.md.disasm_lite(self.read(va, count * 15), va):
            tail = ''
            rip = re.search(r'\[rip ([+-]) (0x[0-9a-f]+)\]', ops)
            if rip:
                target = a + size + int(rip[2], 16) * (1 if rip[1] == '+' else -1)
                text = self.cstr(target)
                tail = f'   ; {target:#x}' + (f' {text!r}' if text else '')
            out.append(f'{a:#010x}  {mnemonic:8} {ops}{tail}')
            if len(out) == count:
                break
        return out

    def callers(self, target):
        """Direct calls and jumps (E8/E9 rel32) to target."""
        hits = []
        for base, blob in self.code():
            for m in re.finditer(rb'[\xe8\xe9]', blob):
                i = m.start()
                if i + 5 <= len(blob) and base + i + 5 + struct.unpack_from('<i', blob, i + 1)[0] == target:
                    hits.append(base + i)
        return hits

    def xref(self, target):
        """rip-relative operands that resolve to target (lea/mov/cmp ... [rip+disp32])."""
        hits = []
        for base, blob in self.code():
            for m in re.finditer(rb'[\x40-\x4f]?[\x8b\x8d\x89\x3b\x39\x80\x81\x83\xc6\xc7]', blob):
                i = m.end()
                if i + 5 > len(blob) or (blob[i] & 0xC7) != 0x05:
                    continue
                # Only accept it if it decodes as one instruction that names this address.
                start = m.start()
                for a, size, mnemonic, ops in self.md.disasm_lite(blob[start:start + 15], base + start, 1):
                    rip = re.search(r'\[rip ([+-]) (0x[0-9a-f]+)\]', ops)
                    if rip and a + size + int(rip[2], 16) * (1 if rip[1] == '+' else -1) == target:
                        hits.append((a, f'{mnemonic} {ops}'))
        return hits

    def strings(self, pattern, minimum=5):
        q = re.compile(pattern, re.I)
        for addr, off, size, _ in self.segments:
            for hit in re.finditer(rb'[\x20-\x7e]{%d,}' % minimum, self.data[off:off + size]):
                text = hit[0].decode()
                if q.search(text):
                    yield addr + hit.start(), text

    def kbm_table(self):
        """Walk the KBM builder: each key query (mov edi, KEY; call KEY_FRAMES/KEY_HELD) is followed
        by stores into one of player 0's controls ([reg + reg + 0x2A0 + 0x14*i]) or a move/look axis."""
        start, end = KBM_BUILDER
        key, mouse, pending, rows, seen = None, None, None, [], set()
        for a, size, mnemonic, ops in self.md.disasm_lite(self.read(start, end - start), start):
            m = re.fullmatch(r'edi, (0x[0-9a-f]+|\d+)', ops)
            if mnemonic == 'mov' and m:
                pending = int(m[1], 0)
                continue
            if mnemonic == 'call':
                target = int(ops, 16) if ops.startswith('0x') else None
                if target in (KEY_FRAMES, KEY_HELD):
                    key = pending
                elif target in (MOUSE_FRAMES, MOUSE_HELD) and pending is not None and pending <= 8:
                    mouse = pending   # a mouse button index (these helpers are also called with key codes)
                continue
            store = re.search(r'word ptr \[\w+ \+ \w+ \+ (0x[0-9a-f]+)\]', ops)
            if mnemonic == 'mov' and store and key is not None:
                off = int(store[1], 16)
                control = (off - CONTROLS_BASE) // CONTROL_STRIDE if (off - CONTROLS_BASE) % CONTROL_STRIDE == 0 else None
                row = (key, mouse, off, control)
                if row not in seen:
                    seen.add(row)
                    rows.append(row)
                key, mouse = None, None
        return rows


def parse(text):
    return int(text, 0)


def describe(eb, va):
    start, _ = eb.func(va)
    return f'{va:#x} (in {start:#x})' if start is not None else f'{va:#x}'


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    ap.add_argument('--elf', type=Path, default=None)
    sub = ap.add_subparsers(dest='cmd', required=True)
    s = sub.add_parser('strings'); s.add_argument('pattern'); s.add_argument('--min', type=int, default=5)
    s = sub.add_parser('xref'); s.add_argument('target', type=parse)
    s = sub.add_parser('callers'); s.add_argument('target', type=parse)
    s = sub.add_parser('func'); s.add_argument('target', type=parse)
    s = sub.add_parser('disasm'); s.add_argument('target', type=parse); s.add_argument('--count', type=int, default=40)
    sub.add_parser('kbm-table')
    a = ap.parse_args(argv)
    eb = Eboot(a.elf or default_elf())
    if a.cmd == 'strings':
        for va, text in eb.strings(a.pattern, a.min):
            print(f'{va:#x}  {text}')
    elif a.cmd == 'xref':
        for va, insn in eb.xref(a.target):
            print(f'{describe(eb, va)}  {insn}')
    elif a.cmd == 'callers':
        for va in eb.callers(a.target):
            print(describe(eb, va))
    elif a.cmd == 'func':
        start, end = eb.func(a.target)
        print(f'{start:#x} .. {end:#x}' if end else f'{start:#x} ..')
    elif a.cmd == 'disasm':
        print('\n'.join(eb.disasm(a.target, a.count)))
    elif a.cmd == 'kbm-table':
        for key, mouse, off, control in eb.kbm_table():
            name = KEY_NAMES[key] if key < len(KEY_NAMES) else {0x69: 'Shift*', 0x6A: 'Ctrl*', 0x6B: 'Alt*', 0x6C: 'Win*'}.get(key, hex(key))
            where = f'control {control}' if control is not None else f'+{off:#x}'
            print(f'{name:8} {"mouse " + str(mouse) if mouse is not None else "":8} -> {where}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
