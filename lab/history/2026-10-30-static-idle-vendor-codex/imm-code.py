from pathlib import Path
exec((Path(__file__).parent/'trace.py').read_text().split('mode=sys.argv[1]')[0])
for a,o,n,f in s.SEGS:
 if not f&1:continue
 for va,sz,m,op in s.MD.disasm_lite(s.B[o:o+n],a):
  if m=='mov' and op.endswith((', 0x12',', 0x13')) and va>0x600000:
   print(hex(va),hex(s.func(va)[0]),m,op)
