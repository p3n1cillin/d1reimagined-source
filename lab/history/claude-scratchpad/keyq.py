import sys, struct, collections
sys.path.insert(0, r'lab/history\2026-10-28-static-combat-codex')
import static56578 as s
B=s.B
a0,o0,sz0=[(a,o,sz) for a,o,sz,f in s.SEGS if f & 1][0]
seg=B[o0:o0+sz0]
NAMES=('Esc F1 F2 F3 F4 F5 F6 F7 F8 F9 F10 F11 F12 F13 ScrLk F15 ` 1 2 3 4 5 6 7 8 9 0 - = Back Tab Q W E R T Y U I O P [ ] BSL Caps '
       'A S D F G H J K L ; QUOTE Enter LShift Z X C V B N M , . / RShift LCtrl LWin LAlt Space RAlt RWin Apps RCtrl Up Down Left Right '
       'Ins Home PgUp Del End PgDn NumLk Num/ Num* Num0 Num1 Num2 Num3 Num4 Num5 Num6 Num7 Num8 Num9 Num- Num+ NumEnter Num.').split()
assert len(NAMES)==104, len(NAMES)
def callers(t):
    out=[];i=seg.find(b'\xe8')
    while i!=-1 and i+5<=len(seg):
        if a0+i+5+struct.unpack_from('<i',seg,i+1)[0]==t: out.append(a0+i)
        i=seg.find(b'\xe8',i+1)
    return out
def keyname(k):
    if isinstance(k,int): return NAMES[k] if k<104 else {0x69:'Shift*',0x6a:'Ctrl*',0x6b:'Alt*',0x6c:'Win*'}.get(k,hex(k))
    return k
target=int(sys.argv[1],0) if len(sys.argv)>1 else 0xAD300
byf=collections.defaultdict(list)
for c in callers(target):
    f,_=s.func(c)
    key=None
    for x in s.MD.disasm(s.read(c-24,24),c-24):
        if x.mnemonic=='mov' and x.op_str.startswith('edi, '):
            v=x.op_str.split(', ')[1]
            try: key=int(v,0)
            except ValueError: key=v
    byf[f].append(f'{c:x}:{keyname(key)}')
for f,v in sorted(byf.items()):
    print(hex(f), ' '.join(v))
