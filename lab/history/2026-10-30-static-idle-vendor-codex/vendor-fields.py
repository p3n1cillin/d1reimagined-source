from pathlib import Path
exec((Path(__file__).parent/'trace.py').read_text().split('mode=sys.argv[1]')[0])
for va,sz,m,op in s.MD.disasm_lite(s.read(0x5c4000,0x6000),0x5c4000):
 if any(f'+ {x}]' in op for x in ('0x280','0x284','0x288')):print(hex(va),hex(s.func(va)[0]),m,op)
