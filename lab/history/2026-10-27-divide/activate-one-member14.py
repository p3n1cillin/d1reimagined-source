"""Single-member discriminator after static proof of the +0x50/+0x58 reference guard."""
import importlib.util, struct
from pathlib import Path
s=importlib.util.spec_from_file_location('r',Path(__file__).with_name('replay-divide14.py'))
r=importlib.util.module_from_spec(s);s.loader.exec_module(r)
data=bytearray(2312)
struct.pack_into('<I',data,0,1)
data[4],data[6]=0xFF,1
# 0x1BCEC3..0x1BCF34: a present reference and non--1 index are cached for the
# unguarded 0x1BDCC1 lookup. Zero is present, not absent. Leave this optional binding absent.
struct.pack_into('<IBxH',data,0x50,0x811C9DC5,0xFF,0xFFFF)
struct.pack_into('<I',data,0x58,0xFFFFFFFF)
lines=[];hit=0
for line in (r.RUN/'divide-squads-gen2.txt').read_text().splitlines():
    fields=line.split()
    if fields[:3]==['0x9243dffb','0x2','21']:
        line=' '.join(fields[:5]+['0x80800868',data.hex()]);hit+=1
    lines.append(line)
assert hit==1
out=r.RUN/'divide-member21-absent-binding.txt'
out.write_text('\n'.join(lines)+'\n')
print('Member 21 only; optional binding absent at +0x50, index -1 at +0x58',flush=True)
r.publish(out)
