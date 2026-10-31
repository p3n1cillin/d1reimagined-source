"""Private six-member discriminator; preserve the twelve squad states and authority."""
import importlib.util, struct
from pathlib import Path
s = importlib.util.spec_from_file_location('replay',Path(__file__).with_name('replay-divide12.py'))
r = importlib.util.module_from_spec(s)
s.loader.exec_module(r)
b = bytearray(2312)
struct.pack_into('<I', b, 0, 1)
b[4], b[6] = 0xFF, 1
lines, hits = [], []
for line in (r.RUN/'divide-squads-gen2.txt').read_text().splitlines():
    fields = line.split()
    if len(fields)>=5 and fields[0]=='0x9243dffb' and fields[1]=='0x2':
        hits.append(int(fields[2]))
        line = ' '.join(fields[:5]+['0x80800868',b.hex()])
    lines.append(line)
assert sorted(hits)==[5,7,9,11,13,21]
out = r.RUN/'divide-members-gen1.txt'
out.write_text('\n'.join(lines)+'\n')
print('Six authored members:', sorted(hits),flush=True)
r.publish(out)
