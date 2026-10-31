"""One bounded live UI-tree sample, READ+QUERY only. No input/native calls/process writes.
Run while the owner has Advancement selected, BEFORE holding CONTINUE.
Fresh roots from 0x982110's manager/screen list; no historical heap addresses.
All raw evidence stays beside this script. A8 is class-specific: interpret as visibility
only for widgets, not data components whose own fields occupy that offset.
"""
from pathlib import Path
import runpy,struct,json,time,collections
P=Path(__file__).parent
v=runpy.run_path(str(P/'attach-readonly.py'));u,rd,b=v['u'],v['rd'],v['base']
manager=(b+0x275709f)&~15;head=manager+0x108
roots=[];seen_links=set();link=u(head)
while link!=head and link not in seen_links and len(roots)<16:
 seen_links.add(link);root=link-0x510
 if u(root)!=b+0x1951b30:raise RuntimeError('screen-list vtable mismatch')
 roots.append(root);link=u(link)
assert link==head,'screen list changed or cap exceeded'
def typename(vt):
 assert b+0x18d8000<=vt<b+0x1a238a0,'component vtable outside owned image'
 ti=u(vt-8);name=u(ti+8)
 assert b<=name<b+0x1a238a0,'RTTI name outside image'
 return rd(name,180).split(b'\0')[0].decode('ascii')
q=collections.deque((root,0,0) for root in roots);seen=set();rows=[];errors=[]
while q and len(rows)<12000:
 obj,parent,depth=q.popleft()
 if obj in seen:continue
 seen.add(obj)
 try:
  h=rd(obj,0x100);vt=struct.unpack_from('<Q',h)[0];typ=typename(vt)
  row={'object':hex(obj),'parent':hex(parent),'depth':depth,'type':typ,'vtable':hex(vt-b),'definition_tag':hex(struct.unpack_from('<I',h,0x18)[0]),'index':struct.unpack_from('<H',h,0x1c)[0],'flags_1e':hex(struct.unpack_from('<H',h,0x1e)[0]),'name_hash':hex(struct.unpack_from('<I',h,0x20)[0]),'raw_a8':hex(struct.unpack_from('<I',h,0xa8)[0]),'head':h.hex()}
  if vt==b+0x1963c30:
   raw=rd(obj,0x630);row['postgame_raw']=raw.hex();row['identity']=[hex(u(obj+0x90)),hex(u(obj+0x98)),u(obj+0xa0,'B')];row['version']=u(obj+0x628,'i');row['character']={'initial_level':u(obj+0xac,'i'),'initial_fractional_level':u(obj+0xb0,'f'),'final_level':u(obj+0xb4,'i'),'final_fractional_level':u(obj+0xb8,'f'),'gained':u(obj+0xbc,'i'),'remaining':u(obj+0xc0,'i')}
  rows.append(row)
  if depth>=40:continue
  sentinel=obj+0x48;child=u(sentinel);siblings=set()
  while child!=sentinel and child not in siblings and len(siblings)<2048:
   siblings.add(child);next_child=u(child);chobj=child-0x38
   if u(chobj+0x30)==obj:q.append((chobj,obj,depth+1))
   else:errors.append({'object':hex(chobj),'reason':'parent changed/mismatch'})
   child=next_child
  if child!=sentinel:errors.append({'object':hex(obj),'reason':'child cycle/cap'})
 except (AssertionError,RuntimeError,UnicodeDecodeError,struct.error) as e:errors.append({'object':hex(obj),'reason':str(e)})
col=u(b+0x26bc040);collector=None
if col:collector={'state':u(col,'B'),'target':u(col+1,'B'),'activity':u(col+2,'h'),'xp':list(struct.unpack('<III',rd(col+0x10,12))),'prestige':list(struct.unpack('<III',rd(col+0x1c,12))),'upgrade_count':u(col+0x258,'I')}
out={'time':time.time(),'base':hex(b),'roots':[hex(x) for x in roots],'nodes':len(rows),'pending_nodes':len(q),'errors':errors,'collector':collector,'components':rows}
dest=P/f'advancement-ui-readonly-{time.time_ns()}.json';dest.write_text(json.dumps(out,indent=2));print(dest)
print(json.dumps({'roots':out['roots'],'nodes':len(rows),'pending':len(q),'errors':len(errors),'postgame':[{'object':r['object'],'definition_tag':r['definition_tag'],'index':r['index'],'character':r['character']} for r in rows if 'character' in r],'collector':collector},indent=2))
