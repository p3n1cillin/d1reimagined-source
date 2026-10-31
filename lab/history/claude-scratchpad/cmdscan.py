import ctypes, ctypes.wintypes as wt, struct, re, sys
src=open(r'F:\d1\public-work\d1reimagined-source\tools\ps4_setup\d1-walk.py').read()
g={'__name__':'lib','__file__':r'F:\d1\public-work\d1reimagined-source\tools\ps4_setup\d1-walk.py'}; exec(compile(src,'d1-walk.py','exec'),g)
c=g['Client']()
k32=ctypes.WinDLL('kernel32',use_last_error=True)
class MBI(ctypes.Structure):
    _fields_=[('BaseAddress',ctypes.c_void_p),('AllocationBase',ctypes.c_void_p),('AllocationProtect',wt.DWORD),('PartitionId',wt.WORD),('RegionSize',ctypes.c_size_t),('State',wt.DWORD),('Protect',wt.DWORD),('Type',wt.DWORD)]
k32.VirtualQueryEx.argtypes=[wt.HANDLE,ctypes.c_void_p,ctypes.POINTER(MBI),ctypes.c_size_t]
pat=re.compile(re.escape(struct.pack('<I',0x8080279a))+b'.{12}'+re.escape(struct.pack('<iif',1,1,1.0)), re.S)
addr,m,hits=0x100000000,MBI(),[]
while addr<0x1000000000 and k32.VirtualQueryEx(c.p.h,ctypes.c_void_p(addr),ctypes.byref(m),ctypes.sizeof(m)):
    b0,sz=m.BaseAddress or 0,m.RegionSize
    if m.State==0x1000 and m.Protect in (0x04,0x40) and sz<(1<<31):
        for o in range(0,sz,1<<24):
            try: buf=c.p.read(b0+o,min(1<<24,sz-o))
            except OSError: continue
            for x in pat.finditer(buf): hits.append(b0+o+x.start()-0x30)
    addr=b0+sz
print(' '.join(hex(h) for h in hits))
