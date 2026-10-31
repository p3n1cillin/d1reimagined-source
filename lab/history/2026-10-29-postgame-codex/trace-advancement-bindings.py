"""Owned UI assets only: reproduce direct post-game data connections; no process APIs."""
from pathlib import Path
import struct,json
P=Path(__file__).parent
TYPE=0x879669a748d56de4

def array(b,o,stride):
 n,r=struct.unpack_from('<Qq',b,o);a=o+8+r+16
 assert 0<=n<=4096 and 0<=a and a+n*stride<=len(b)
 return [a+i*stride for i in range(n)]

out=[]
for tag in ('80aa2ed2','80aa2ed4'):
 b=(P/f'owned-{tag}.bin').read_bytes()
 imports=[struct.unpack_from('<IHH',b,o) for o in array(b,8,8)]
 first=1+sum(n for _,_,n in imports)
 nodes=[struct.unpack_from('<QI',b,o) for o in array(b,0x18,16)]
 ids=[first+i for i,(t,_) in enumerate(nodes) if t==TYPE]
 assert len(ids)==1
 source=ids[0];links=[]
 for i,o in enumerate(array(b,0x138,64)):
  v=struct.unpack_from('<8Q',b,o)
  if v[0]==source or v[3]==source:links.append({'index':i,'source':v[0],'source_property':hex(v[2]),'target':v[3],'target_property':hex(v[5])})
 char_links=[r for r in links if r['source']==source and 4<=int(r['source_property'],16)<=15]
 out.append({'graph':tag,'post_data_node':source,'links':links,'direct_character_xp_links':char_links})
assert out[0]['post_data_node']==120 and out[1]['post_data_node']==1237
assert not any(x['direct_character_xp_links'] for x in out)
assert [x['source_property'] for x in out[1]['links'] if x['source']==1237]==['0x18']
(P/'advancement-direct-bindings.json').write_text(json.dumps(out,indent=2))
print(json.dumps(out,indent=2))
