import sys,struct,json
from pathlib import Path
sys.path.insert(0,r'lab/history\2026-10-28-static-combat-codex')
import static56578 as s
P=Path(__file__).parent
mode=sys.argv[1]
args=sys.argv[2:]
if mode=='func':
 for x in args:
  a,b=s.func(int(x,16));print(f'FUNCTION {a:x}..{b:x}');print(s.dis(a,b-a))
elif mode=='refs':
 for x in args:
  target=int(x,16);print('REFS',x)
  for va,t,k in s.index()['refs']:
   if target==t:print(hex(va),k,'function',hex(s.func(va)[0]))
  print('relocs',*[hex(x) for x in s.ptrrefs(target)])
elif mode=='imm':
 for x in args:
  needle=struct.pack('<I',int(x,16));print('IMM',x)
  for a,o,n,f in s.SEGS:
   start=o
   while True:
    i=s.B.find(needle,start,o+n)
    if i<0:break
    va=a+i-o;start=i+1
    print(hex(va),'function',hex(s.func(va)[0]) if f&1 else 'data')
elif mode=='strings':
 for x in args:
  for a,t in s.strings(x):print(hex(a),t)
