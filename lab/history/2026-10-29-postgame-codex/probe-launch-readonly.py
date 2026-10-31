"""Bounded owned activity table reads; READ + QUERY process rights only, no input."""
import ctypes, ctypes.wintypes as wt, subprocess, re, struct, json, time
from pathlib import Path
P = Path(__file__).parent
listing = subprocess.run(['tasklist','/FI','IMAGENAME eq shadPS4.exe','/FO','CSV','/NH'], capture_output=True, text=True).stdout
ids = [int(line.split('\",\"')[1]) for line in listing.splitlines() if line.startswith('\"shadPS4')]
assert len(ids) == 1
log = Path(r'F:\d1\tools\shadPS4\0.19.0\user\log\shad_log.txt').read_bytes()[:2000000].decode('utf8','replace')
bases = re.findall(r'Loading module eboot\.bin to (0x[0-9a-f]+)', log)
assert len(bases) == 1
base = int(bases[0],16)
k = ctypes.WinDLL('kernel32',use_last_error=True)
k.OpenProcess.restype = wt.HANDLE
k.ReadProcessMemory.argtypes = [wt.HANDLE,ctypes.c_void_p,ctypes.c_void_p,ctypes.c_size_t,ctypes.POINTER(ctypes.c_size_t)]
handle = k.OpenProcess(0x410,False,ids[0])
assert handle
def rd(address, size):
    buf = ctypes.create_string_buffer(size)
    got = ctypes.c_size_t()
    assert k.ReadProcessMemory(handle,address,buf,size,ctypes.byref(got)) and got.value == size, (hex(address),size)
    return buf.raw
def u(address, fmt='Q'):
    return struct.unpack('<'+fmt,rd(address,struct.calcsize(fmt)))[0]
assert base <= u(base+0x19474b0) < base+0x18d8000
ca = u(base+0x26b95b0)+8
n = u(ca,'I')
assert 165 < n <= 1024
cells = ca+8+u(ca+8,'q')+16
out = {'time':time.time(),'base':hex(base),'count':n,'nonempty_videos':[],'selected_local_records':[]}
for idx in range(n):
    cell = cells+idx*8
    rel = u(cell,'q')
    if not rel:
        continue
    definition = cell+rel
    video = u(definition+0x18,'I')
    world = u(definition+0x38,'I')
    row = {'index':idx,'address':hex(definition),'video':hex(video),'world_tag':hex(world),'head':rd(definition,72).hex()}
    if video != 0x811c9dc5:
        out['nonempty_videos'].append(row)
    if idx in (0,164,165,166,168):
        row['name'] = rd(definition+u(definition,'q'),100).split(b'\0')[0].decode('ascii','replace')
        out['selected_local_records'].append(row)
dest = P/'local-activity-video-control-readonly.json'
dest.write_text(json.dumps(out,indent=2))
print(dest)
print('count',n,'nonempty video controls',[(r['index'],r['video'],r['world_tag']) for r in out['nonempty_videos']])
print('selected',[(r['index'],r['video'],r['world_tag']) for r in out['selected_local_records']])
print('names',[(r['index'],r['name']) for r in out['selected_local_records']])
