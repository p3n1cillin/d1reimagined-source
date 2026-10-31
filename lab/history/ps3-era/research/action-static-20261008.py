"""Private offline action callback references, no live process attachment."""
import json
import sys
from pathlib import Path
sys.path.insert(0, r"F:\d1\d1xenia\scripts")
from d1_eboot import Eboot

root = Path(__file__).parent
out = root / "action-static-20261008"
out.mkdir(exist_ok=False)
e = Eboot(root / "ps3_updates" / "01.24" / "USRDIR" / "EBOOT.elf")
summary = []
functions = set()
for target in (0x5aea30, 0x5aede4, 0x2b279c, 0x2b2a60):
    refs = []
    for descriptor in e.data_refs(target):
        if e.u32(descriptor + 4) != e.toc:
            continue
        code = e.code_refs(descriptor) + e.toc_refs(descriptor)
        data = e.data_refs(descriptor)
        row = {"descriptor": hex(descriptor),
               "code": [{"pc": hex(pc), "function": hex(e.function_of(pc)[0])} for pc, _ in code],
               "data": [hex(p) for p in data]}
        refs.append(row)
        functions.update(e.function_of(pc)[0] for pc, _ in code)
        for address in data:
            raw = e.read(address - 32, 96)
            if raw:
                (out / f"data-{address:08x}.hex").write_text(raw.hex() + "\n")
            for pc, _ in e.code_refs(address) + e.toc_refs(address):
                functions.add(e.function_of(pc)[0])
    summary.append({"target": hex(target), "callers": [hex(p) for p in e.callers(target)],
                    "descriptors": refs})
for function in functions:
    lo, hi = e.function_of(function)
    (out / f"{function:08x}.asm").write_text("\n".join(e.disasm(lo, min((hi - lo) // 4, 1200))) + "\n")
(out / "references.json").write_text(json.dumps(summary, indent=2) + "\n")
print(json.dumps({"references": summary, "functions": [hex(f) for f in sorted(functions)]}))
