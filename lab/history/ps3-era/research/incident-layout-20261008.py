import json
import sys
from pathlib import Path
sys.path.insert(0, r"F:\d1\d1xenia\scripts")
from d1_eboot import Eboot
root = Path(__file__).parent
e = Eboot(root / "ps3_updates/01.24/USRDIR/EBOOT.elf")
schema = json.loads(Path(r"F:\d1\d1xenia-data\ps3_updates\types_01.24.json").read_text())
rows = []
for address in (0x1aabbf0, 0x1aabfb0, 0x1aaac30, 0x1aaac60, 0x189ad48, 0x189ad9c):
    row = {"address":hex(address), "words":[]}
    for pos in range(address,address+0x24,4):
        value = e.u32(pos)
        row['words'].append({"off":hex(pos-address),"value":hex(value) if value is not None else None,
                             "schema":schema.get(hex(value)) if value else None})
    rows.append(row)
(root/'incident-layout-20261008.json').write_text(json.dumps(rows,indent=2)+'\n')
print(json.dumps(rows))
