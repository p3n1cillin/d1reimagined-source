"""Publish one control list, never overwriting another pending command."""
import sys,time
from pathlib import Path
run=Path(r'F:\d1\d1preservation-main\cfw-staging\2026-10-27-divide16')
control=run.parent/'ah-control.txt'
mode=sys.argv[1]
lines=(run/'obj17-46.txt').read_text().splitlines()
out=[]
for line in lines:
    cols=line.split()
    if len(cols)>=7 and cols[:3]==['0x992bb471','0x20','17']:
        img=bytearray.fromhex(cols[6])
        if mode in ('active','handover','announce','visible'):
            img[0x4c:0x50]=(12).to_bytes(4,'little')
            img[0x66:0x68]=(46).to_bytes(2,'little')
            if mode=='announce': img[0]=1
            if mode=='visible': img[0]=3
        elif mode=='reset':
            img=bytearray(len(img)); img[0x2c0:0x2c4]=b'\xff'*4
        else: raise ValueError(mode)
        cols[6]=img.hex(); line=' '.join(cols)
    elif len(cols)>=7 and cols[:3]==['0x992bb471','0x20','14'] and mode in ('handover','announce','visible','reset'):
        img=bytearray.fromhex(cols[6]); img[0]=0; cols[6]=img.hex(); line=' '.join(cols)
    out.append(line)
target=run/f'codex-obj17-{mode}.txt'
target.write_text('\n'.join(out)+'\n')
with control.open('x') as f: f.write(f'clients 0x4786c0e0 {target.as_posix()}\n')
end=time.monotonic()+30
while control.exists() and time.monotonic()<end: time.sleep(.25)
if control.exists(): raise RuntimeError('control not consumed; do not overwrite')
print(f'{target.name}: control consumed; acceptance/readback still required')
