import json
import sys
from pathlib import Path
sys.path.insert(0, r"F:\d1\d1xenia\scripts")
from d1_eboot import Eboot
root = Path(__file__).parent
out = root / "action-paths-20261008"
out.mkdir(exist_ok=True)  # analysis was interrupted by an unmapped BSS address
e = Eboot(root / "ps3_updates/01.24/USRDIR/EBOOT.elf")
targets = {0x5adcdc, 0x5adb4c, 0x26b25c, 0x5af060, 0x5afabc,
           0x5ad614, 0x2766b8, 0x5ae674, 0x5ae734, 0x5aeb4c}
rows = []
for target in sorted(targets):
    callers = e.callers(target)
    rows.append({"target": hex(target), "callers": [{"pc": hex(p), "function": hex(e.function_of(p)[0])} for p in callers]})
for target in targets | {e.function_of(p)[0] for target in targets for p in e.callers(target)}:
    lo, hi = e.function_of(target)
    (out / f"{target:08x}.asm").write_text("\n".join(e.disasm(lo, min((hi-lo)//4, 1800))) + "\n")
strings = [{"address": hex(a), "text": s} for a, s in e.strings(6)
           if any(k in s.lower() for k in ("reward", "script_action", "loot")) and len(s)<250]
(out / "strings.json").write_text(json.dumps(strings, indent=2) + "\n")
named = []
for addr in (0x1304170, 0x1304188):
    named.append({"address": hex(addr), "text": e.read(addr, 100).split(b"\0")[0].decode("ascii", errors="replace")})
table = [{"address": hex(a), "value": hex(value) if value is not None else None}
         for a in range(0x1a20750, 0x1a20800, 4) for value in (e.u32(a),)]
(out / "references.json").write_text(json.dumps({"rows": rows, "table": table, "named": named}, indent=2) + "\n")
print(json.dumps({"rows": rows, "named": named, "strings": strings}))
