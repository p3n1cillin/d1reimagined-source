from pathlib import Path
exec((Path(__file__).parent/'trace.py').read_text().split('mode=sys.argv[1]')[0])
for target in (0x784e50,0x783ee0):
 for va,t,k in s.index()['refs']:
  if t==target:
   lo,hi=s.func(va);lines=s.dis(lo,hi-lo).splitlines();i=next(i for i,l in enumerate(lines) if l.startswith(f'{va:#010x} '));print('\nCALL',hex(va),'\n'+'\n'.join(lines[max(0,i-9):i+1]))
