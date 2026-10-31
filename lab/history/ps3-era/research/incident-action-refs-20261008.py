import json
import sys
from pathlib import Path
sys.path.insert(0, r"F:\d1\d1xenia\scripts")
from d1_eboot import Eboot
root = Path(__file__).parent
out = root / "incident-action-refs-20261008"
out.mkdir(exist_ok=False)
e = Eboot(root / "ps3_updates/01.24/USRDIR/EBOOT.elf")
rows = []
functions = set()
for target in (0x1a20758, 0x1a207b8, 0x1303868, 0x18eee3c, 0x18eedc4):
    refs = e.code_refs(target) + e.toc_refs(target)
    rows.append({"target": hex(target), "refs": [{"pc": hex(pc), "function": hex(e.function_of(pc)[0])} for pc, _ in refs]})
    functions.update(e.function_of(pc)[0] for pc, _ in refs)
for f in functions:
    lo, hi = e.function_of(f)
    (out / f"{f:08x}.asm").write_text("\n".join(e.disasm(lo, min((hi-lo)//4, 1600))) + "\n")
(out / "references.json").write_text(json.dumps(rows, indent=2) + "\n")
print(json.dumps(rows))
