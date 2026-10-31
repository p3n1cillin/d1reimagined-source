exec(open(__file__.replace('scan.py','trace.py')).read().split('mode=sys.argv[1]')[0])
for target in (0x783ee0,0x783ac0,0x783960,0xa96500,0xa96110,0x58ab80):
 print('TARGET',hex(target))
 for va,t,k in s.index()['refs']:
  if t==target:
   lo,hi=s.func(va);d=s.dis(lo,hi-lo)
   if target==0x783ee0 and 'esi, 0x12' not in d and 'esi, 0x13' not in d:continue
   print(hex(va),hex(lo),d if target==0x783ee0 else '')
