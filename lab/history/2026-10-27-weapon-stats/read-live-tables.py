import importlib.util,struct,json
from pathlib import Path
s=importlib.util.spec_from_file_location('g',r'F:\d1\public-work\d1reimagined-source\tools\ps4_setup\d1-winguest.py')
g=importlib.util.module_from_spec(s);s.loader.exec_module(g)
p,base=g.open_client()
players=p.u(p.u(base+g.PLAYER_POOL,'Q')+8,'Q')
result={'pid':10544,'base':hex(base),'players':hex(players),'tables':[]}
for off in (0x1460,0x14e0,0x1560,0x15e0):
 raw=p.read(players+off,0x80)
 pairs=[struct.unpack_from('<b3xI',raw,i*8) for i in range(16)]
 result['tables'].append({'offset':hex(off),'address':hex(players+off),'pairs':pairs})
 print(hex(off),hex(players+off),pairs)
Path(__file__).with_name('player-stat-tables.json').write_text(json.dumps(result,indent=2))
