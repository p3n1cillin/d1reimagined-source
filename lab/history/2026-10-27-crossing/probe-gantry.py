"""Diagnostic AH activation from this run's native prop defaults; no guest writes."""
exec(compile(open(__file__.replace('probe-gantry.py','inspect-namespace.py')).read().split('groups=[]')[0], 'inspect-namespace.py','exec'))
import time
run=Path(r'F:\d1\d1preservation-main\cfw-staging\2026-10-27-divide17')
rows=json.loads(Path(__file__).with_name('namespace-04.json').read_text())['rows']
r=next(r for r in rows if r['dx']==209)
obj=int(r['object'],16)
assert p.u(obj,'I')==0x80cb844e, 'not the gantry'
old=p.read(obj+0x1b0,0xf0)
assert old[8]==0 and p.u(obj+0x390,'I')==0xffffffff, 'gantry not in expected uncreated state'
Path(__file__).with_name('gantry-original-state.bin').write_bytes(old)
new=bytearray(old); struct.pack_into('<I',new,0,1); new[8]=1
lines=[]; hits=0
for line in (run/'divide-gen2-objective17.txt').read_text().splitlines():
    c=line.split()
    if len(c)>=5 and c[:3]==['0x9243dffb','0x4','0']:
        line=' '.join(c[:5]+['0x808007d9',new.hex()]+c[7:]); hits+=1
    lines.append(line)
assert hits==1
target=run/'codex-gantry-default-create.txt'; target.write_text('\n'.join(lines)+'\n')
with (run.parent/'ah-control.txt').open('x') as f: f.write(f'clients 0x4786c0e0 {target.as_posix()}\n')
print('queued native-default gantry creation, gen1/exist1; acceptance and entity readback required',flush=True)
time.sleep(7)
print('state',p.read(obj+0x1b0,12).hex(),'entity',hex(p.u(obj+0x390,'I')))
