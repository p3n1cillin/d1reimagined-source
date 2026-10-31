"""Extend the stable member-21 correction to all six authored members, keeping squad states."""
import importlib.util, struct
from pathlib import Path
s=importlib.util.spec_from_file_location('r',Path(__file__).with_name('replay-divide14.py'))
r=importlib.util.module_from_spec(s);s.loader.exec_module(r)
data=bytearray(2312)
struct.pack_into('<I',data,0,1)
data[4],data[6]=0xFF,1
struct.pack_into('<IBxH',data,0x50,0x811C9DC5,0xFF,0xFFFF)
struct.pack_into('<I',data,0x58,0xFFFFFFFF)
lines=[];hits=[]
for line in (r.RUN/'divide-squads-gen2.txt').read_text().splitlines():
    fields=line.split()
    if fields[:2]==['0x9243dffb','0x2']:
        line=' '.join(fields[:5]+['0x80800868',data.hex()]);hits.append(int(fields[2]))
    lines.append(line)
assert sorted(hits)==[5,7,9,11,13,21]
out=r.RUN/'divide-six-members-absent-binding.txt'
out.write_text('\n'.join(lines)+'\n')
r.publish(out)
