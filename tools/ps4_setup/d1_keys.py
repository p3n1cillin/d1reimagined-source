# SPDX-License-Identifier: GPL-2.0-or-later
"""Send pad keys to the shadPS4 window from Python: what tools/d1-pad.ps1 does, without a PowerShell
start per press.

    press('Hold:W:700', 'NumPad2', gap=150)

Key names are System.Windows.Forms.Keys names as d1-pad.ps1 takes them (W/A/S/D left stick, I/J/K/L
camera, NumPad2 cross, NumPad6 circle, NumPad4 square, NumPad8 triangle, Return options, Up/Down/
Left/Right d-pad, Q/U L1/R1, E/O L2/R2). "Hold:<key>:<ms>" holds a key (default 120 ms). The window is
brought to the front first when it isn't, as d1-pad.ps1 does; the emulator ignores keys sent to an
unfocused window. Each press stamps d1-pad-last.txt beside this file for the idle guard.

d1-walk.py pressed every key through `powershell -File d1-pad.ps1`: a PowerShell start of 0.3-0.6 s
for each turn and each forward pulse, about doubling the time of a walk.
"""
import ctypes
import ctypes.wintypes as wt
import os
import re
import time

_SPECIAL = {'Return': 0x0D, 'Escape': 0x1B, 'Space': 0x20, 'Up': 0x26, 'Down': 0x28, 'Left': 0x25,
            'Right': 0x27, 'Tab': 0x09, 'Back': 0x08}
_HOLD = re.compile(r'^Hold:(\w+(?:\+\w+)*):(\d+)$')


def vk(name):
    """The virtual-key code of a System.Windows.Forms.Keys name used by the pad."""
    if name in _SPECIAL:
        return _SPECIAL[name]
    if re.fullmatch(r'NumPad[0-9]', name):
        return 0x60 + int(name[-1])
    if re.fullmatch(r'D[0-9]', name):
        return 0x30 + int(name[-1])
    if re.fullmatch(r'F([1-9]|1[0-2])', name):
        return 0x6F + int(name[1:])
    if re.fullmatch(r'[A-Z]', name):
        return ord(name)
    raise ValueError(f'unknown pad key {name!r}')


def parse(keys):
    """[(names, vks, hold_ms)] for keys given as d1-pad.ps1 takes them (comma-joined is fine too).
    A chord is keys joined by '+' ('W+NumPad2', 'Hold:W+NumPad2:900'); names and vks are tuples."""
    out = []
    for item in keys:
        for k in (x for x in str(item).split(',') if x):
            m = _HOLD.match(k)
            spec, hold = (m.group(1), int(m.group(2))) if m else (k, 120)
            names = tuple(spec.split('+'))
            out.append((names, tuple(vk(n) for n in names), hold))
    return out


def key_flags(code):
    """The arrow cluster is E0-prefixed; without it SDL sees the matching numpad key.

    Windows maps both Down and NumPad2 to scan 0x50, even with MAPVK_VK_TO_VSC_EX on
    some layouts. Preserve the distinction explicitly for the arrow keys this driver supports.
    See Microsoft's Keyboard Input Overview, Extended-Key Flag.
    """
    return 1 if 0x25 <= code <= 0x28 else 0  # KEYEVENTF_EXTENDEDKEY


def _window():
    user32 = ctypes.windll.user32
    found = []

    @ctypes.WINFUNCTYPE(ctypes.c_bool, wt.HWND, wt.LPARAM)
    def each(h, _):
        title = ctypes.create_unicode_buffer(256)
        user32.GetWindowTextW(h, title, 256)
        if user32.IsWindowVisible(h) and title.value.startswith('shadPS4'):
            found.append(h)
        return True

    user32.EnumWindows(each, 0)
    return found[0] if found else None


def _focus(h):
    user32 = ctypes.windll.user32
    if user32.GetForegroundWindow() == h:
        return True
    user32.keybd_event(0x12, 0, 0, 0)          # an Alt tap lets SetForegroundWindow through
    user32.keybd_event(0x12, 0, 2, 0)
    user32.ShowWindow(h, 9)
    user32.SetForegroundWindow(h)
    time.sleep(0.25)
    return user32.GetForegroundWindow() == h


def post_lparam(code, scan, down):
    """The WM_KEYDOWN/WM_KEYUP lParam d1-pad.ps1 -Post builds: repeat 1, scan code, the extended
    flag (bit 24) for the arrow cluster, and the previous-state and transition bits on key-up."""
    lparam = 1 | (scan << 16) | (0x01000000 if key_flags(code) else 0)
    return lparam if down else lparam | 0xC0000000


def post_mode(post=None):
    """Explicit post wins; None reads D1_KEYS_POST ("1" = posted)."""
    return os.environ.get('D1_KEYS_POST') == '1' if post is None else bool(post)


def press(*keys, gap=150, post=None):
    """Press the keys in order; raises RuntimeError when there is no shadPS4 window.

    Posted mode (post=True, or D1_KEYS_POST=1) sends WM_KEYDOWN/WM_KEYUP with PostMessage: no focus,
    no Alt tap, no keybd_event, so it works with the desktop locked (the emulator reads the
    messages from its own queue)."""
    posted = post_mode(post)
    plan = parse(keys)
    user32 = ctypes.windll.user32
    h = _window()
    if not h:
        raise RuntimeError('No shadPS4 window')
    stamp = None
    # Beside this file, where d1-pad.ps1 stamps it and d1-idle-guard.sh reads it (the guard presses
    # keys after 300 s without a stamp).
    tools = os.path.dirname(os.path.abspath(__file__))
    for names, codes, hold in plan:
        if not posted and not _focus(h):
            continue
        scans = [user32.MapVirtualKeyW(code, 0) for code in codes]
        if posted:
            for code, scan in zip(codes, scans):
                user32.PostMessageW(h, 0x100, code, post_lparam(code, scan, True))
            time.sleep(hold / 1000)
            for code, scan in reversed(list(zip(codes, scans))):
                user32.PostMessageW(h, 0x101, code, post_lparam(code, scan, False))
        else:
            for code, scan in zip(codes, scans):
                user32.keybd_event(code, scan, key_flags(code), 0)
            time.sleep(hold / 1000)
            for code, scan in reversed(list(zip(codes, scans))):
                user32.keybd_event(code, scan, key_flags(code) | 2, 0)
        stamp = int(time.time())
        time.sleep(gap / 1000)
    if stamp and tools:
        try:
            with open(os.path.join(tools, 'd1-pad-last.txt'), 'w') as f:
                f.write(str(stamp))
        except OSError:
            pass
