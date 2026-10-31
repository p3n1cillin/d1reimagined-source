"""Release every key bound to the pad (default.ini), so no tool leaves a button or stick held.

A tool killed mid-press (timeout, crash) leaves its key down; held L1 plus a later R1 casts the Super,
and orphaned walkers keep driving alongside the next tool. Run between tools:  python release.py
"""
import ctypes

u32 = ctypes.WinDLL('user32')
# sticks w a s d / i j k l; L1 q, R1 u, L2 e, R2 o, L3 x, R3 m; face c b n v and keypad 2 4 6 8;
# options Enter, touchpad Space, D-pad (emotes) arrows.
KEYS = [ord(ch) for ch in 'WASDIJKLQUEOXMCBNV'] + [0x62, 0x64, 0x66, 0x68, 0x0D, 0x20, 0x25, 0x26, 0x27, 0x28]


def release_all():
    for vk in KEYS:
        u32.keybd_event(vk, u32.MapVirtualKeyW(vk, 0), 2, None)


if __name__ == '__main__':
    release_all()
