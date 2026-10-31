"""Read-only sampler for the vertical-recenter tests (documents/d1/vertical-recenter-research.md).
Player-0 look state P = *(u64*)(base + 0x2725798); pitch f32 at P+0x3C (radians, 0 = level),
centering latch u8 at P+0xE8, counter i32 at P+0xEC, current hard pitch limits f32 at
P+0x164/P+0x168; auto-centering setting u8 at base+0x27259B0+0x9A.
Opens shadps4.exe with PROCESS_QUERY_INFORMATION | PROCESS_VM_READ only.
The eboot base is checked against the applier prologue at base+0x91EEF0; the right base is the
log line "program entry addr" minus 0xC2B470.
Mouse capture (and with it the KBM recentering gates) is released whenever the game window loses
focus. Started from a terminal, give a delay, click back into the game, and make the look motion
while sampling runs: a recentering pull starts the moment vertical mouse motion stops.
Usage: py pitch_watch.py [seconds=4] [interval_ms=100] [eboot_base=0x5380000] [delay_s=0]
The verdict compares the last sample with the sample farthest from level (the end of the look
motion): HOLDS when they differ by under 0.002 rad, otherwise DRIFTS toward or away from level."""
import ctypes, ctypes.wintypes as wt, struct, subprocess, sys, time
k32 = ctypes.WinDLL("kernel32", use_last_error=True)
k32.ReadProcessMemory.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_size_t, ctypes.POINTER(ctypes.c_size_t)]
k32.OpenProcess.restype = wt.HANDLE
secs = float(sys.argv[1]) if len(sys.argv) > 1 else 4.0
step = float(sys.argv[2]) / 1000 if len(sys.argv) > 2 else 0.1
base = int(sys.argv[3], 16) if len(sys.argv) > 3 else 0x5380000
delay = float(sys.argv[4]) if len(sys.argv) > 4 else 0.0
APPLIER_PROLOGUE = bytes.fromhex("554889e541574156415541545348")
o = subprocess.run(["tasklist", "/FI", "IMAGENAME eq shadps4.exe", "/FO", "CSV", "/NH"], capture_output=True, text=True).stdout.strip()
if "shadps4" not in o.lower():
    sys.exit("shadps4.exe is not running")
pid = int(o.splitlines()[0].strip('"').split('","')[1])
h = k32.OpenProcess(0x0400 | 0x0010, False, pid)
def rd(a, n):
    b = (ctypes.c_char * n)(); got = ctypes.c_size_t()
    if not k32.ReadProcessMemory(h, ctypes.c_void_p(a), b, n, ctypes.byref(got)): raise OSError(f"read {a:#x}")
    return bytes(b)
try:
    prologue = rd(base + 0x91EEF0, len(APPLIER_PROLOGUE))
except OSError:
    prologue = b""
if prologue != APPLIER_PROLOGUE:
    sys.exit(f"eboot base {base:#x} is wrong for pid {pid}: no applier prologue at base+0x91EEF0; "
             "pass the base (program entry addr - 0xC2B470) as the third argument")
if delay > 0:
    print(f"sampling starts in {delay:g} s: click into the game now", flush=True)
    time.sleep(delay)
P = struct.unpack("<Q", rd(base + 0x2725798, 8))[0]
setting = rd(base + 0x27259B0 + 0x9A, 1)[0]
print(f"pid {pid} base={base:#x} P={P:#x} auto_centering_setting={setting}")
t0 = time.time(); first = pitch = peak = None
while time.time() - t0 < secs:
    t = time.time() - t0
    pitch = struct.unpack("<f", rd(P + 0x3C, 4))[0]
    latch = rd(P + 0xE8, 1)[0]; cnt = struct.unpack("<i", rd(P + 0xEC, 4))[0]
    lo, hi = struct.unpack("<ff", rd(P + 0x164, 8))
    first = pitch if first is None else first
    if peak is None or abs(pitch) >= abs(peak[1]):
        peak = (t, pitch)
    print(f"t={t:5.2f}s pitch={pitch:+.4f} rad ({pitch*57.2958:+6.1f} deg) d={pitch-first:+.4f} "
          f"latch={latch} cnt={cnt} limits=[{lo:+.3f},{hi:+.3f}]", flush=True)
    time.sleep(step)
change = pitch - peak[1]
verdict = "HOLDS" if abs(change) < 0.002 else (
    "DRIFTS toward level" if abs(pitch) < abs(peak[1]) else "DRIFTS away from level")
print(f"{verdict}: farthest {peak[1]:+.4f} rad at t={peak[0]:.2f}s, end {pitch:+.4f} rad "
      f"(change {change:+.4f}, {change*57.2958:+.1f} deg)")
