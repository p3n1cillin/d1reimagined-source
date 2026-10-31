"""Read back the fresh world's clients without retaining addresses from another boot."""
import importlib.util, json, re, struct
from pathlib import Path
s=importlib.util.spec_from_file_location('g',r'F:\d1\public-work\d1reimagined-source\tools\ps4_setup\d1-winguest.py')
g=importlib.util.module_from_spec(s);s.loader.exec_module(g)
p,base=g.open_client()
run=Path(r'F:\d1\d1preservation-main\cfw-staging\2026-10-27-divide14')
table=base+g.LT_TABLE
rows=[];all_divide=[];members=[]
for line in (run/'worldset.txt').read_text().splitlines():
    match=re.search(r'tag (0x[0-9a-f]+) obj (0x[0-9a-f]+)',line)
    if not match:continue
    tag,obj=map(lambda t:int(t,0),match.groups())
    name,kind,sub=struct.unpack('<IBxh',p.read(obj,8))
    if name!=0x9243DFFB:continue
    all_divide.append({'kind':kind,'sub':sub,'pending':p.u(obj+0x15,'B'),'owner':p.u(obj+0x60,'I')})
    if kind==2:
        arena=p.u(table+p.u(table,'q')+p.u(obj+0x40,'H')*p.u(table+0x10,'Q')+0x30,'Q')
        raw=p.read(arena+p.u(obj+0x38,'Q'),2312)
        members.append({'sub':sub,'gen':struct.unpack_from('<I',raw)[0],
                        'binding':raw[0x50:0x58].hex(),'index':struct.unpack_from('<i',raw,0x58)[0]})
    if kind!=1:continue
    arena=p.u(table+p.u(table,'q')+p.u(obj+0x40,'H')*p.u(table+0x10,'Q')+0x30,'Q')
    raw=p.read(arena+p.u(obj+0x38,'Q'),124)
    rows.append({'sub':sub,'gen':struct.unpack_from('<I',raw,0x44)[0],
                 'variants':struct.unpack_from('<I',raw,0x2C)[0],'desired':struct.unpack_from('<h',raw,0x30)[0]})
rows.sort(key=lambda t:t['sub'])
assert [r['sub'] for r in rows]==[4,6,8,10,12,14,15,16,17,18,19,20]
assert all(r['gen']==2 and r['variants']==1 and r['desired']==1 for r in rows)
assert len(all_divide)==34 and not any(r['pending'] for r in all_divide)
report={'divide_clients':len(all_divide),'pending':0,'squads':rows,'members':members}
output=run/('source-members-readback.json' if '--source' in __import__('sys').argv else
            'corrected-members-readback.json' if '--members' in __import__('sys').argv else 'scheduled96-readback.json')
assert not output.exists(), 'Preserve earlier readback; choose a new report filename'
if '--members' in __import__('sys').argv:
    assert len(members)==6 and all(r['gen']==1 and r['binding']=='c59d1c81ff00ffff' and r['index']==-1 for r in members)
    print('PASS: all six native member states have generation 1 and the absent optional binding')
output.write_text(json.dumps(report,indent=2)+'\n')
print('PASS: all 34 Divide clients delivered; all 12 squads have generation 2 and desired count 1')
