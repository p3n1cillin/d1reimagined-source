from pathlib import Path
exec((Path(__file__).parent/'trace.py').read_text().split('mode=sys.argv[1]')[0])
for lo,hi in zip(s.FUNCS,s.FUNCS[1:]):
 if not 0x750000<=lo<0x7b0000:continue
 d=s.dis(lo,hi-lo)
 if any(x in d for x in ('esi, 0x12\n','edx, 0x12\n','esi, 0x13\n','edx, 0x13\n')):print(d)
