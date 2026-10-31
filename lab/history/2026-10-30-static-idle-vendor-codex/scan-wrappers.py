exec(open(str(Path(__file__).parent/'trace.py')).read().split('mode=sys.argv[1]')[0])
for target in (0x784e50,):
 print('TARGET',hex(target))
 for va,t,k in s.index()['refs']:
  if t==target:
   lo,hi=s.func(va);d=s.dis(lo,hi-lo)
   if target==0x784e50 and 'esi, 0x12' not in d and 'esi, 0x13' not in d:continue
   print(hex(va),hex(lo),d if target==0x784e50 else '')



