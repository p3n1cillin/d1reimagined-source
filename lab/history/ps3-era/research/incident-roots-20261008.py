import json
import sys
from pathlib import Path
sys.path.insert(0, r"F:\d1\d1xenia\scripts")
from d1_eboot import Eboot
from d1_pkg import PackageSet, Oodle
root = Path(__file__).parent
rt = root.parent
out = root / "incident-roots-20261008"
out.mkdir(exist_ok=False)
e = Eboot(root / "ps3_updates/01.24/USRDIR/EBOOT.elf")
def fnv(s):
    v = 0x811c9dc5
    for b in s.encode():
        v = ((v ^ b) * 0x1000193) & 0xffffffff
    return hex(v)
refs = []
for address in (0x18dedc4, 0x18dee3c, 0x18ded3c, 0x18ded40):
    v = e.u32(address)
    code = e.code_refs(address) + e.toc_refs(address)
    refs.append({"address": hex(address), "elf_value": hex(v) if v is not None else None,
                 "code": [{"pc": hex(pc), "function": hex(e.function_of(pc)[0])} for pc, _ in code]})
    for pc, _ in code:
        lo, hi = e.function_of(pc)
        (out / f"{lo:08x}.asm").write_text("\n".join(e.disasm(lo, min((hi-lo)//4, 1600))) + "\n")
ps = PackageSet(list((root/'ps3_updates/final/packages').glob('*.pkg')) +
                list((rt/'01_WORKING/ps3_destiny/PS3_GAME/USRDIR/packages').glob('*.pkg')),
                Oodle(rt/'TOOLS/oodle/oo2core_3_win64_x86-64/oo2core_3_win64.dll'))
named = [{"pkg": str(p.path), **n} for p in ps.by_id.values() for n in p.named
         if any(s in n['name'].lower() for s in ('incident', 'client_globals', 'client globals'))]
result = {"hashes": {s:fnv(s) for s in ('item_loot', 'actor_killed', 'player_entered_trigger')},
          "corrected_resource_globals": refs, "named": named}
(out/'roots.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result))
