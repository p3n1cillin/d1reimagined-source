"""Hold a handle to the owned emulator process and record its exit code (read-only)."""
import ctypes, json, sys, time, datetime
from ctypes import wintypes
k = ctypes.WinDLL('kernel32', use_last_error=True)
k.OpenProcess.restype = wintypes.HANDLE
manifest, out = sys.argv[1], sys.argv[2]
deadline = time.time() + 600
client = None
while time.time() < deadline and client is None:
    try:
        m = json.load(open(manifest, encoding='utf-8-sig'))
        client = next((p for p in m['processes'] if p['tag'] == 'rpcs3'), None)
    except Exception:
        pass
    if client is None: time.sleep(2)
h = k.OpenProcess(0x00100000 | 0x1000, False, client['id'])  # SYNCHRONIZE | QUERY_LIMITED_INFORMATION
if not h: raise SystemExit(f'open failed {ctypes.get_last_error()}')
# verify identity by creation time
ct, et, kt, ut = (wintypes.FILETIME() for _ in range(4))
k.GetProcessTimes(h, ctypes.byref(ct), ctypes.byref(et), ctypes.byref(kt), ctypes.byref(ut))
ticks = ((ct.dwHighDateTime << 32) | ct.dwLowDateTime) + 504911232000000000
if abs(ticks - int(client['startTimeUtcTicks'])) > 10_000_000: raise SystemExit('identity mismatch')
print('watching', client['id'], flush=True)
k.WaitForSingleObject(h, 0xFFFFFFFF)
code = wintypes.DWORD(); k.GetExitCodeProcess(h, ctypes.byref(code))
rec = {'utc': datetime.datetime.now(datetime.timezone.utc).isoformat(), 'client': client, 'exit_code': code.value, 'exit_code_hex': hex(code.value)}
json.dump(rec, open(out, 'w'), indent=2); print(json.dumps(rec), flush=True)
