import json
import sys
from pathlib import Path
sys.path.insert(0, r"F:\d1\d1xenia\scripts")
from d1_eboot import Eboot
root = Path(__file__).parent
out = root / "action-tables-20261008"
out.mkdir(exist_ok=False)
e = Eboot(root / "ps3_updates/01.24/USRDIR/EBOOT.elf")
functions = set()
tables = []
for start, end in ((0x13ECEE0, 0x13ED040), (0x189ACF0, 0x189ADB0)):
    rows = []
    for address in range(start, end, 4):
        value = e.u32(address)
        row = {"address": hex(address), "value": hex(value)}
        if e.u32(value + 4) == e.toc:
            row["function"] = hex(e.u32(value))
        code = e.code_refs(address) + e.toc_refs(address)
        if code:
            row["code"] = [{"pc": hex(pc), "function": hex(e.function_of(pc)[0])} for pc, _ in code]
            functions.update(e.function_of(pc)[0] for pc, _ in code)
        raw = e.read(value, 100)
        if raw:
            s = raw.split(b"\0", 1)[0]
            if len(s) > 5 and all(32 <= c < 127 for c in s):
                row["string"] = s.decode("ascii")
        rows.append(row)
    tables.append(rows)
for function in functions:
    lo, hi = e.function_of(function)
    (out / f"{function:08x}.asm").write_text("\n".join(e.disasm(lo, min((hi-lo)//4, 1400))) + "\n")
(out / "tables.json").write_text(json.dumps(tables, indent=2) + "\n")
print(json.dumps({"interesting": [r for t in tables for r in t if len(r)>2],
                  "functions": [hex(f) for f in sorted(functions)]}))
