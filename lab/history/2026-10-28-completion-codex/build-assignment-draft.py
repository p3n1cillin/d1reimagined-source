"""Private unpublished diagnostic rows, preserving the existing Dock baseline."""
from pathlib import Path
import json,struct
root=Path(r'F:\d1\d1preservation-main\cfw-staging')
src=root/'agr-dock13-obj26-m52.txt'
lines=src.read_text().splitlines();changed=[]
for i,line in enumerate(lines):
 f=line.split()
 if len(f)<5 or f[0].lower()!='0x062dd95d':continue
 kind,sub=int(f[1],0),int(f[2],0)
 if kind==1 and sub in (11,12):
  assert len(f)>=7 and int(f[5],0)==0x8080071e
  image=bytearray.fromhex(f[6]);assert len(image)==124
  original=bytes(image)
  image[0:8]=struct.pack('<IBxh',0x062dd95d,3,4)
  struct.pack_into('<II',image,0x68,0,0)
  assert image[8:0x68]==original[8:0x68] and image[0x70:]==original[0x70:]
  f[6]=image.hex();lines[i]=' '.join(f);changed.append(sub)
 if kind==3 and sub==4:
  image=bytearray(28);image[0]=1
  lines[i]=' '.join(f[:5]+['0x808004fa',image.hex()]);changed.append('encounter')
assert set(changed)=={11,12,'encounter'}
header='''# PRIVATE UNPUBLISHED DIAGNOSTIC DRAFT; NOT AN AUTHENTIC MISSION ASSIGNMENT.
# Baseline: agr-dock13-obj26-m52.txt. All other rows preserved byte-for-byte.
# Test only in Dock 13 (104); current sense48 is in outro 105. Do not publish there.
# Reconcile current generation/pending clients and authority before lab-owner publication.
# Content-derived: encounter 062dd95d/3/4 has nine tasks; task 0 has one subtask,
# using authored firing-area-set reference 062dd95d/42/44, capacity 4.
# INFERRED EXPERIMENT: assign only squads 11/12 to that encounter and task ordinal 0.
# A+0 reference = 5dd92d0603000400; A+68 = 0 retained from baseline, meaning unresolved;
# A+6c = 0 selects top-level task 0. Desired counts and generation are preserved.
# Encounter A[0] = 1 is a fresh revision/reset counter for task 0, NOT enable/wave 1.
# Other 23 revision bytes and u32 A+18 = 0. No content-derived initial counter exists.
# Native expected diagnostic: squad+3e0 resolves encounter; +3ec = 0; +3f0 becomes 0;
# +274 resolves firing-area set 44. Only then inspect movement; movement is NOT verified.
# Missing authentic information: original server's encounter/task assignment and timing
# for squads 11/12, and the role/value of A+68. The examined task metadata has no
# populated squad-to-task membership table; its ten reference slots are absent refs.
'''
dest=root/'agr-dock13-assign-1112.draft.txt';dest.write_text(header+'\n'.join(lines)+'\n')
print(dest);print('changed',changed)
