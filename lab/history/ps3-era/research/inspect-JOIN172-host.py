from pathlib import Path
import ctypes as c
from ctypes import wintypes as w
import json
import struct
import bisect

rt=Path(r'F:\d1\<pc>-import\d1xenia\local')
raw=(rt/'RESEARCH/JOIN172-host-stall.dmp').read_bytes()
assert raw[:4]==b'MDMP'
n,offset=struct.unpack_from('<II',raw,8)
streams={}
for i in range(n):
    kind,size,rva=struct.unpack_from('<III',raw,offset+i*12)
    streams[kind]=(size,rva)
def string(rva):
    size,=struct.unpack_from('<I',raw,rva)
    return raw[rva+4:rva+4+size].decode('utf-16-le')
_,rva=streams[4]
count,=struct.unpack_from('<I',raw,rva)
modules=[]
for i in range(count):
    at=rva+4+i*108
    base,size,checksum,stamp,name=struct.unpack_from('<QIIII',raw,at)
    modules.append({'base':base,'size':size,'name':string(name)})
def module(addr):
    return next((m for m in modules if m['base']<=addr<m['base']+m['size']),None)
dbg=c.WinDLL(r'F:\VS\BuildTools\Common7\IDE\Extensions\TestPlatform\Extensions\Cpp\x64\dbghelp.dll',use_last_error=True)
kernel=c.WinDLL('kernel32',use_last_error=True)
kernel.GetCurrentProcess.restype=w.HANDLE
h=kernel.GetCurrentProcess()
dbg.SymInitializeW.argtypes=[w.HANDLE,w.LPCWSTR,w.BOOL];dbg.SymInitializeW.restype=w.BOOL
dbg.SymSetOptions.argtypes=[w.DWORD];dbg.SymSetOptions.restype=w.DWORD
dbg.SymLoadModuleExW.argtypes=[w.HANDLE,w.HANDLE,w.LPCWSTR,w.LPCWSTR,c.c_uint64,w.DWORD,c.c_void_p,w.DWORD];dbg.SymLoadModuleExW.restype=c.c_uint64
dbg.SymCleanup.argtypes=[w.HANDLE];dbg.SymCleanup.restype=w.BOOL
dbg.SymSetOptions(0x00000004|0x00000002|0x00000200|0x00080000) # deferred, undecorate, fail critical errors, no prompts
assert dbg.SymInitializeW(h,r'F:\d1\<pc>-import\dev\rpcs3\bin',False),c.get_last_error()
loaded=[]
for m in modules:
    path=m['name']
    if Path(path).name.lower()=='rpcs3.exe':
        path=r'F:\d1\<pc>-import\dev\rpcs3\bin\rpcs3.exe'
    if Path(path).exists():
        loaded.append({'name':path,'result':dbg.SymLoadModuleExW(h,None,path,None,m['base'],m['size'],None,0)})
class Symbol(c.Structure):
    _fields_=[('SizeOfStruct',w.ULONG),('TypeIndex',w.ULONG),('Reserved',c.c_uint64*2),('Index',w.ULONG),('Size',w.ULONG),
              ('ModBase',c.c_uint64),('Flags',w.ULONG),('Value',c.c_uint64),('Address',c.c_uint64),('Register',w.ULONG),
              ('Scope',w.ULONG),('Tag',w.ULONG),('NameLen',w.ULONG),('MaxNameLen',w.ULONG),('Name',c.c_char*1024)]
dbg.SymFromAddr.argtypes=[w.HANDLE,c.c_uint64,c.POINTER(c.c_uint64),c.POINTER(Symbol)];dbg.SymFromAddr.restype=w.BOOL
class Line(c.Structure):
    _fields_=[('SizeOfStruct',w.DWORD),('Key',c.c_void_p),('LineNumber',w.DWORD),('FileName',c.c_char_p),('Address',c.c_uint64)]
dbg.SymGetLineFromAddr64.argtypes=[w.HANDLE,c.c_uint64,c.POINTER(w.DWORD),c.POINTER(Line)];dbg.SymGetLineFromAddr64.restype=w.BOOL
def symbol(addr):
    m=module(addr)
    out={'address':hex(addr),'module':Path(m['name']).name if m else None}
    if m:
        out['module_offset']=hex(addr-m['base'])
        s=Symbol();s.SizeOfStruct=88;s.MaxNameLen=1024;displacement=c.c_uint64()
        if dbg.SymFromAddr(h,addr,c.byref(displacement),c.byref(s)):
            out['symbol']=bytes(s.Name).decode('utf-8',errors='replace');out['displacement']=hex(displacement.value)
        line=Line();line.SizeOfStruct=c.sizeof(Line);delta=w.DWORD()
        if dbg.SymGetLineFromAddr64(h,addr,c.byref(delta),c.byref(line)):
            out['source_file']=line.FileName.decode('utf-8',errors='replace');out['source_line']=line.LineNumber
    return out
infos={}
memory=[]
_,mem_rva=streams[5]
mem_count,=struct.unpack_from('<I',raw,mem_rva)
for i in range(mem_count):
    start,size,file_rva=struct.unpack_from('<QII',raw,mem_rva+4+i*16)
    memory.append((start,size,file_rva))
images={}
def read_memory(address,size):
    for start,length,file_rva in memory:
        if start<=address and address+size<=start+length:
            return raw[file_rva+address-start:file_rva+address-start+size]
    m=module(address)
    if not m:return None
    path=m['name']
    if Path(path).name.lower()=='rpcs3.exe':path=r'F:\d1\<pc>-import\dev\rpcs3\bin\rpcs3.exe'
    if path not in images:
        if not Path(path).exists():return None
        pe=Path(path).read_bytes();offset,=struct.unpack_from('<I',pe,60)
        assert pe[offset:offset+4]==b'PE\0\0'
        sections,=struct.unpack_from('<H',pe,offset+6);optional,=struct.unpack_from('<H',pe,offset+20)
        section_at=offset+24+optional
        table=[]
        for i in range(sections):
            virtual_size,rva,file_size,file_at=struct.unpack_from('<IIII',pe,section_at+i*40+8)
            table.append((rva,file_size,file_at))
        images[path]=(pe,table)
    pe,table=images[path];rva=address-m['base']
    for start,length,file_at in table:
        if start<=rva and rva+size<=start+length:
            return pe[file_at+rva-start:file_at+rva-start+size]
    return None
class Address(c.Structure):
    _fields_=[('Offset',c.c_uint64),('Segment',w.WORD),('Mode',w.DWORD)]
class KDHelp(c.Structure):
    _fields_=[('Thread',c.c_uint64),('ThCallbackStack',w.DWORD),('ThCallbackBStore',w.DWORD),('NextCallback',w.DWORD),('FramePointer',w.DWORD),
              ('KiCallUserMode',c.c_uint64),('KeUserCallbackDispatcher',c.c_uint64),('SystemRangeStart',c.c_uint64),('KiUserExceptionDispatcher',c.c_uint64),
              ('StackBase',c.c_uint64),('StackLimit',c.c_uint64),('BuildVersion',w.DWORD),('RetpolineStubFunctionTableSize',w.DWORD),
              ('RetpolineStubFunctionTable',c.c_uint64),('RetpolineStubOffset',w.DWORD),('RetpolineStubSize',w.DWORD),('Reserved0',c.c_uint64*2)]
class StackFrame(c.Structure):
    _fields_=[('AddrPC',Address),('AddrReturn',Address),('AddrFrame',Address),('AddrStack',Address),('AddrBStore',Address),
              ('FuncTableEntry',c.c_void_p),('Params',c.c_uint64*4),('Far',w.BOOL),('Virtual',w.BOOL),('Reserved',c.c_uint64*3),('KdHelp',KDHelp)]
assert c.sizeof(StackFrame)==264
ReadCallback=c.WINFUNCTYPE(w.BOOL,w.HANDLE,c.c_uint64,c.c_void_p,w.DWORD,c.POINTER(w.DWORD))
FunctionCallback=c.WINFUNCTYPE(c.c_void_p,w.HANDLE,c.c_uint64)
ModuleCallback=c.WINFUNCTYPE(c.c_uint64,w.HANDLE,c.c_uint64)
@ReadCallback
def read_callback(process,address,out,size,read):
    data=read_memory(address,size)
    if data is None:read[0]=0;return False
    c.memmove(out,data,len(data));read[0]=len(data);return True
dbg.SymFunctionTableAccess64.argtypes=[w.HANDLE,c.c_uint64];dbg.SymFunctionTableAccess64.restype=c.c_void_p
function_tables={}
@FunctionCallback
def function_callback(process,address):
    m=module(address)
    if not m:return None
    key=m['name']
    if key not in function_tables:
        path=key
        if Path(path).name.lower()=='rpcs3.exe':path=r'F:\d1\<pc>-import\dev\rpcs3\bin\rpcs3.exe'
        if not Path(path).exists():return None
        pe=Path(path).read_bytes();nt,=struct.unpack_from('<I',pe,60)
        exception_rva,size=struct.unpack_from('<II',pe,nt+24+112+3*8)
        data=read_memory(m['base']+exception_rva,size) if size else None
        if not data:return None
        assert len(data)%12==0
        entries=list(struct.iter_unpack('<III',data))
        function_tables[key]=(entries,[x[0] for x in entries],c.create_string_buffer(data))
    entries,starts,buffer=function_tables[key]
    rva=address-m['base'];index=bisect.bisect_right(starts,rva)-1
    if index>=0 and entries[index][0]<=rva<entries[index][1]:
        return c.addressof(buffer)+index*12
    return None
@ModuleCallback
def module_callback(process,address):
    m=module(address);return m['base'] if m else 0
dbg.StackWalk64.argtypes=[w.DWORD,w.HANDLE,w.HANDLE,c.POINTER(StackFrame),c.c_void_p,ReadCallback,FunctionCallback,ModuleCallback,c.c_void_p]
dbg.StackWalk64.restype=w.BOOL
def unwind(tid,ctx):
    context=c.create_string_buffer(ctx)
    frame=StackFrame()
    frame.AddrPC.Offset=struct.unpack_from('<Q',ctx,248)[0]
    frame.AddrStack.Offset=struct.unpack_from('<Q',ctx,152)[0]
    frame.AddrFrame.Offset=struct.unpack_from('<Q',ctx,160)[0]
    frame.AddrPC.Mode=frame.AddrStack.Mode=frame.AddrFrame.Mode=3 # AddrModeFlat
    result=[];seen=set()
    for i in range(40):
        ok=dbg.StackWalk64(0x8664,h,w.HANDLE(tid),c.byref(frame),context,read_callback,function_callback,module_callback,None)
        key=(frame.AddrPC.Offset,frame.AddrStack.Offset)
        if not ok or not frame.AddrPC.Offset or key in seen or not module(frame.AddrPC.Offset):break
        seen.add(key);result.append(dict(stack_pointer=hex(frame.AddrStack.Offset),**symbol(frame.AddrPC.Offset)))
    return result
names={}
if 24 in streams:
    _,at=streams[24]
    count,=struct.unpack_from('<I',raw,at)
    for i in range(count):
        tid,name_rva=struct.unpack_from('<IQ',raw,at+4+i*12)
        names[tid]=string(name_rva)
if 17 in streams:
    _,at=streams[17]
    header,entry,count=struct.unpack_from('<III',raw,at)
    for i in range(count):
        values=struct.unpack_from('<IIIIQQQQQQ',raw,at+header+i*entry)
        infos[values[0]]={'kernel_time_100ns':values[6],'user_time_100ns':values[7],'start_address':hex(values[8])}
_,at=streams[3]
count,=struct.unpack_from('<I',raw,at)
threads=[]
for i in range(count):
    pos=at+4+i*48
    tid,suspend,priority_class,priority,teb,stack_address,stack_size,stack_rva,context_size,context_rva=struct.unpack_from('<IIIIQQIIII',raw,pos)
    ctx=raw[context_rva:context_rva+context_size]
    assert len(ctx)>=256
    rsp,=struct.unpack_from('<Q',ctx,152);rip,=struct.unpack_from('<Q',ctx,248)
    candidates=[]
    # These are raw stack pointer candidates, not a reconstructed call stack.
    for p in range(max(0,rsp-stack_address),stack_size-7,8):
        addr,=struct.unpack_from('<Q',raw,stack_rva+p)
        m=module(addr)
        if m and Path(m['name']).name.lower()=='rpcs3.exe':
            candidates.append(dict(stack_offset=hex(p),**symbol(addr)))
            if len(candidates)>=20:break
    call_stack=unwind(tid,ctx) if names.get(tid) in ('rsx::thread','Vulkan Driver Manager') else []
    threads.append({'id':tid,'name':names.get(tid),'instruction':symbol(rip),'rsp':hex(rsp),'info':infos.get(tid),'raw_stack_candidates':candidates,'unwound_stack':call_stack})
dbg.SymCleanup(h)
result={'dump':'JOIN172-host-stall.dmp','streams':list(streams),'symbol_modules':loaded,'threads':threads,
        'scope':'Exact-context instruction addresses, raw stack candidates and bounded DbgHelp StackWalk64 for RSX/driver using captured stack memory and local matching images/PDB. Raw candidates alone are not call stacks. Private; no network symbol downloads.'}
(rt/'RESEARCH/JOIN172-host-symbol-candidates.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
for thread in threads:
    names=[x.get('symbol',x['module_offset']) for x in thread['raw_stack_candidates'][:6]]
    if names: print(json.dumps({'id':thread['id'],'name':thread['name'],'instruction':thread['instruction'],'candidates':names}))
