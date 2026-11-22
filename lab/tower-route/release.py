"""Release every key bound to the pad (default.ini), so no tool leaves a button or stick held.

A tool killed mid-press (timeout, crash) leaves its key down; held L1 plus a later R1 casts the Super,
and orphaned walkers keep driving alongside the next tool. Run between tools:  python release.py
"""
import ctypes
import os
import sys

_SETUP = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..', 'tools', 'ps4_setup'))
if _SETUP not in sys.path:
    sys.path.insert(0, _SETUP)
import d1_keys  # noqa: E402

u32 = ctypes.WinDLL('user32')
# sticks w a s d / i j k l; L1 q, R1 u, L2 e, R2 o, L3 x, R3 m; face c b n v and keypad 2 4 6 8;
# options Enter, touchpad Space, D-pad (emotes) arrows.
KEYS = [ord(ch) for ch in 'WASDIJKLQUEOXMCBNV'] + [0x62, 0x64, 0x66, 0x68, 0x0D, 0x20, 0x25, 0x26, 0x27, 0x28]


def _posted_release_all():
    hwnd = d1_keys._window()
    if not hwnd:
        raise RuntimeError('posted release did not find the emulator window')
    failed = []
    for vk in KEYS:
        lp = d1_keys.post_lparam(vk, u32.MapVirtualKeyW(vk, 0), False)
        for _ in range(2):
            if not hwnd:
                hwnd = d1_keys._window()
            if hwnd and u32.PostMessageW(hwnd, 0x101, vk, lp):
                break
            hwnd = None
        else:
            failed.append(hex(vk))
    if failed:
        raise RuntimeError('posted release (%s) did not reach the emulator window' % ', '.join(failed))


def release_all():
    if d1_keys.post_mode():
        _posted_release_all()
        return
    for vk in KEYS:
        u32.keybd_event(vk, u32.MapVirtualKeyW(vk, 0), 2, None)


if __name__ == '__main__':
    release_all()
