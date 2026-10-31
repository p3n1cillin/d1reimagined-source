import sys,struct
from pathlib import Path
sys.path.insert(0,'F:/d1/d1xenia/scripts')
from d1_eboot import Eboot
root=Path(__file__).parent;eb=Eboot(root/'ps3_updates/01.24/USRDIR/EBOOT.elf')
for pc,count in ((0x123c028,64),(0x11958c4,64),(0x11cc490,64)):
    raw=eb.read(pc,count*4)
    print('DATA',hex(pc),raw.hex() if raw else None)
data=(root/'JOIN183-cinematic-static/80aa9a07.bin').read_bytes()
for off in range(0,len(data),16):print(f'{off:04x}: {data[off:off+16].hex()}')
for h in (0x80802b49,0x80802b63):
    addr=list(eb.data_refs(h))[0]
    print('TYPE',hex(h),hex(addr),eb.read(addr-16,112).hex())
