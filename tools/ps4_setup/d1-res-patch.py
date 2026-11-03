"""Write a 56578 client patch that renders at W x H instead of 1920 x 1080 (experimental).

    d1-res-patch.py W H [--out FILE] [--base ps4/patches/CUSA00219-56578-lab-orbit.xml]

The game's resolution (notes/ps4-56578-resolution-2026-11-03.md) is set in three places, all 1080p:
- 0x7ED1C0 reads two 16-bit globals (0x18DCC58 / 0x18DCC5C, 1920 / 1080) and clamps them to
  320..1920 x 240..1080 for the renderer's set-up (0x7E40A0 and four other callers);
- 0x7E40A0 sets the renderer's display size [+0x28F8] / [+0x28FC] to 1920 / 1080: the game registers
  its display buffers at that size;
- the surface slot table 0x18DEA40 (20 slots of 0x1C bytes: +0x14 width, +0x18 height), read by
  0x81AB80, gives every render target its size: 1920x1080, 960x540, 480x270, 480x1080, and 2048x2048
  for shadows (kept). With the slots left at 1080p the game halts ("Attempting to acquire surface
  slot 8, split0 which has already been allocated").
This rewrites the three for W x H (multiples of 4: the 480 x 270 slots become W/4 x H/4). Up to 2048
wide the picture is whole (2048 x 1152, session res-2048-a); wider, it is right only to x = 2048 and
compressed past it (2560 x 1440, res-1440-c), and growing the 2048 x 2048 slots too crashed (res-1440-e).
Wider than 2048 needs the emulator fork's wide views (b194bd3): then 2304 x 1296 is whole.
The surface pool (450 MB) can be set with --pool-mb.
"""
import argparse
import os
import struct

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
BASE = os.path.join(REPO, 'ps4', 'patches', 'CUSA00219-56578-lab-orbit.xml')
BIAS = 0x400000  # the patch files' addresses
SLOT_TABLE = 0x18DEA40
# 0x81A8F0 allocates the surface pool: 0x1C210000 bytes (mov edi, imm32 at 0x81A8F9) with a limit of
# 0x1C200000 ([+0x155DC], the imm32 at 0x81A921), from the engine heap 14 (about 450 MB). Doubling it
# for 2560x1440 or quadrupling it for 4K left the heap short ("cortex: ENGINE OOM! ... with ID 14",
# sessions res-1440-f and res-4k-d): --pool-mb sets it, default unchanged.
POOL, POOL_LIMIT = 0x1C210000, 0x1C200000
# The pool comes from memory arena 2, which 0xB4440 sizes at 2900 MB (mov eax, 0xB5400000 at 0xB45A2);
# at 2560x1440 with a 512 MB pool 568 MB of it was free. --arena-mb sets it.
SLOTS = {0: (1920, 1080), 1: (1920, 1080), 2: (1920, 1080), 3: (480, 1080), 4: (480, 1080), 5: (480, 270),
         6: (480, 270), 7: (960, 540), 8: (1920, 1080), 10: (1920, 1080), 11: (1920, 1080), 12: (1920, 1080),
         13: (1920, 1080), 14: (480, 270), 19: (1920, 1080)}


def line(va, data, comment):
    return f'      <Line Type="bytes" Address="{va + BIAS:X}" Value="{data.hex().upper()}"/>  <!-- {comment} -->'


def block(w, h, pool_mb=None, arena_mb=None):
    u16, i32 = (lambda v: struct.pack('<H', v)), (lambda v: struct.pack('<i', v))
    rows = [
        '      <!-- 0x7ED1C0: the clamp 320..1920 x 240..1080 -->',
        line(0x7ED1DA, b'\x66\xb9' + u16(w), f'mov cx, {w}'),
        line(0x7ED1DE, b'\x66\x3d' + u16(w), f'cmp ax, {w}'),
        line(0x7ED200, b'\x66\xb9' + u16(h), f'mov cx, {h}'),
        line(0x7ED204, b'\x66\x3d' + u16(h), f'cmp ax, {h}'),
        line(0x18DCC58, u16(w), f'resolution width 1920 -> {w}'),
        line(0x18DCC5C, u16(h), f'resolution height 1080 -> {h}'),
        '      <!-- 0x7E40A0: the display size -->',
        line(0x7E40CF, bytes.fromhex('41C785F8280000') + i32(w), f'mov dword [r13+0x28f8], {w}'),
        line(0x7E40DA, bytes.fromhex('41C785FC280000') + i32(h), f'mov dword [r13+0x28fc], {h}'),
        '      <!-- the surface slot table 0x18DEA40 -->',
    ]
    for k, (sw, sh) in sorted(SLOTS.items()):
        nw, nh = sw * w // 1920, sh * h // 1080
        rows.append(line(SLOT_TABLE + k * 0x1C + 0x14, i32(nw) + i32(nh), f'slot {k}: {sw}x{sh} -> {nw}x{nh}'))
    if pool_mb:
        size = pool_mb << 20
        rows.append(f'      <!-- the surface pool, {pool_mb} MB -->')
        rows.append(line(0x81A8F9, bytes.fromhex('BF') + i32(size + POOL - POOL_LIMIT), f'mov edi, {size + POOL - POOL_LIMIT:#x}'))
        rows.append(line(0x81A91B, bytes.fromhex('C783DC550100') + i32(size), f'mov dword [rbx+0x155dc], {size:#x}'))
    if arena_mb:
        rows.append(f'      <!-- memory arena 2, {arena_mb} MB -->')
        rows.append(line(0xB45A2, bytes.fromhex('B8') + struct.pack('<I', arena_mb << 20), f'mov eax, {arena_mb << 20:#x}'))
    return (f'  <Metadata Name="D1 56578: render at {w}x{h} (synthetic, experimental)"\n'
            '            isEnabled="true" AppVer="01.00" AppElf="eboot.bin" Author="d1reimagined" PatchVer="1.0">\n'
            '    <PatchList>\n' + '\n'.join(rows) + '\n    </PatchList>\n  </Metadata>\n')


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('width', type=int)
    ap.add_argument('height', type=int)
    ap.add_argument('--base', default=BASE)
    ap.add_argument('--out')
    ap.add_argument('--pool-mb', type=int, help='surface pool size in MB (default: the game 450)')
    ap.add_argument('--arena-mb', type=int, help='memory arena 2 size in MB (default: the game 2900)')
    a = ap.parse_args()
    if a.width % 4 or a.height % 4 or not (320 <= a.width <= 3840 and 240 <= a.height <= 2160):
        raise SystemExit('width and height: multiples of 4, up to 3840 x 2160')
    src = open(a.base, encoding='utf-8').read()
    i = src.rindex('</Patch>')
    out = a.out or os.path.join(os.getcwd(), f'lab-orbit-{a.width}x{a.height}.xml')
    with open(out, 'w', encoding='utf-8') as f:
        f.write(src[:i] + block(a.width, a.height, a.pool_mb, a.arena_mb) + src[i:])
    print(f'{a.width}x{a.height} -> {out}')


if __name__ == '__main__':
    main()
