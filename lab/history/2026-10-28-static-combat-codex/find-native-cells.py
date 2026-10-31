"""Bounded read-only startup-cell inventory. No game control or debugger."""
import importlib.util,json,struct
import static56578 as st
spec=importlib.util.spec_from_file_location('g',r'F:\d1\public-work\d1reimagined-sense\tools\ps4_setup\d1-winguest.py');g=importlib.util.module_from_spec(spec);spec.loader.exec_module(g)
p,base=g.open_client();out=[]
for tag in (0x80800463,0x80802ba7):
 needle=struct.pack('<I',tag)
 for start in range(0x2500000,0x2f60000,0x100000):
  raw=p.read(base+start,0x100000)
  pos=raw.find(needle)
  while pos>=0:
   va=start+pos
   if va%8==0:
    desc=p.u(base+va+8,'Q')-base
    if 0x1200000<=desc<0x1600000:
     slots=st.ptrrefs(va);out.append({'tag':hex(tag),'cell':hex(va),'descriptor':hex(desc),'slots':[hex(x) for x in slots]})
   pos=raw.find(needle,pos+1)
(st.ROOT/'native-cell-inventory.json').write_text(json.dumps(out,indent=2));print(json.dumps(out,indent=2))
