"""Keep keyboard input on the shadPS4 window: in-process keybd_event goes to the foreground window."""
import ctypes, ctypes.wintypes as wt
u32 = ctypes.WinDLL('user32'); k32 = ctypes.WinDLL('kernel32')
_hwnd = None
def _find():
    found = []
    @ctypes.WINFUNCTYPE(ctypes.c_bool, wt.HWND, wt.LPARAM)
    def cb(h, _):
        if u32.IsWindowVisible(h):
            pid = wt.DWORD(); u32.GetWindowThreadProcessId(h, ctypes.byref(pid))
            hp = k32.OpenProcess(0x1000, False, pid.value)
            if hp:
                buf = ctypes.create_unicode_buffer(260); n = wt.DWORD(260)
                if k32.QueryFullProcessImageNameW(hp, 0, buf, ctypes.byref(n)) and buf.value.lower().endswith('shadps4.exe'):
                    found.append(h)
                k32.CloseHandle(hp)
        return True
    u32.EnumWindows(cb, 0)
    return found[0] if found else None
import os, time
PAUSE = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'codex.lock')
def ensure():
    """Wait while codex.lock exists (someone is typing into another app), then bring the game window
    to the foreground if something else has it. Returns True when it does."""
    global _hwnd
    t0 = time.time()
    # D1_LAB_DRIVER=1 marks the tool that holds the pause itself (it paused the keep-alive to drive).
    while os.path.exists(PAUSE) and not os.environ.get('D1_LAB_DRIVER') and time.time() - t0 < 120: time.sleep(0.2)
    if not _hwnd or not u32.IsWindow(_hwnd): _hwnd = _find()
    if not _hwnd: return False
    if u32.GetForegroundWindow() == _hwnd: return True
    u32.keybd_event(0x12, 0, 0, None); u32.SetForegroundWindow(_hwnd); u32.keybd_event(0x12, 0, 2, None)
    return u32.GetForegroundWindow() == _hwnd
