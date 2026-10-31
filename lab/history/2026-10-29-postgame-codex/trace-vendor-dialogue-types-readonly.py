"""Owned-image RTTI continuation. Disk-only; no live attachment or input."""
from pathlib import Path
import json,sys,datetime
sys.path.insert(0,r'lab/history\2026-10-28-static-combat-codex')
import static56578 as st
P=Path(__file__).parent
rows=[]
for nameva in (0x15df6b0,0x15e0400,0x167c410):
    for nameptr in st.ptrrefs(nameva):
        typeinfo=nameptr-8
        for typeptr in st.ptrrefs(typeinfo):
            vt=typeptr+8
            methods=[]
            for i in range(16):
                try:fn=st.pointer(vt+i*8)
                except ValueError:break
                if not 0<fn<0x1500000:break
                lo,hi=st.func(fn)
                methods.append({'slot':i,'va':hex(fn),'function':hex(lo)})
            rows.append({'name':st.cstr(nameva),'typeinfo':hex(typeinfo),'candidate_vtable':hex(vt),'methods':methods})
out={'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'owned_image_sha256':st.SHA,
     'method':'RTTI relocation references; candidate boundaries stop at first non-code value',
     'scope':'static lead only; no proof of Approach sender or vendor-screen acceptance','rows':rows}
destination=P/'vendor-dialogue-rtti-continuation.json'
if destination.exists():raise FileExistsError(destination)
destination.write_text(json.dumps(out,indent=2));print(json.dumps(out,indent=2))
