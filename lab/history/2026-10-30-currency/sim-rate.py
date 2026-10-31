"""Read-only: the engine's game-time tick count (0x2416180 -> 0x5916F0: *(*(base+0x26BC880)+0xC)) and tick period
(+0x8, seconds per tick) sampled over wall time -> ticks per wall second and simulated seconds per wall second.
Full speed is 1.00 whatever the frame rate. Usage: sim-rate.py [seconds]"""
import runpy, sys, time, struct
from pathlib import Path
v = runpy.run_path(str(Path(r'lab/history\2026-10-30-vendor-gate') / 'attach-readonly.py')); u, b = v['u'], v['base']
secs = float(sys.argv[1]) if len(sys.argv) > 1 else 10
g = u(b + 0x26bc880)
t0, n0 = time.perf_counter(), u(g + 0xc, 'i'); time.sleep(secs); t1, n1 = time.perf_counter(), u(g + 0xc, 'i')
period = u(g + 8, 'f')
rate = (n1 - n0) / (t1 - t0)
print(f'tick period {period * 1000:.3f} ms; {n1 - n0} ticks in {t1 - t0:.2f} s = {rate:.2f} ticks/s; simulated s per wall s = {rate * period:.3f}')
