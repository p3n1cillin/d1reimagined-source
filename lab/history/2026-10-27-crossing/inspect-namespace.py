"""Read-only 56578 namespace probe; current process and base are verified by d1-winguest."""
import ctypes, importlib.util, json, struct, sys
from pathlib import Path
src = Path(r'F:\d1\public-work\d1reimagined-source\tools\ps4_setup\d1-winguest.py')
s = importlib.util.spec_from_file_location('winguest', src)
g = importlib.util.module_from_spec(s); s.loader.exec_module(g)
p, base = g.open_client()
T = base + g.LT_TABLE
index = p.u(base + g.LT_COUNT, 'H')
content = p.u(T + p.u(T, 'q') + index * p.u(T+0x10, 'Q') + 0x30, 'Q')
print(f'content {content:#x} index {index}; header {p.read(content, 16).hex(" ")}')
reg = p.u(p.u(base+g.REGISTRY, 'Q'), 'Q')
def resolve(tag):
    if tag == 0xffffffff: return None
    esi = struct.unpack('<i', struct.pack('<I',tag))[0] >> 13
    slot = (((esi & 0xffffffff)>>18)|0x3ff) & esi & 0xffffffff
    rec = reg + slot*0x40
    data, stride, mask = p.u(rec+0x10,'Q'), p.u(rec+0x30,'i'), p.u(rec+0x34,'i')
    if not data or not 0 < stride <= 0x100000: return None
    e = data + (tag & 0x1fff)*stride
    return e - ctypes.c_int64(mask & p.u(e+8,'Q')).value
groups=[]
for i in range(p.u(content+8,'i')):
    raw=p.read(content+0x10+i*0x14,0x14)
    row=struct.unpack('<5I',raw); groups.append(row)
    print('group',i, ' '.join(f'{x:08x}' for x in row))
kind = int(sys.argv[1],0) if len(sys.argv)>1 else 0x2c
start,count=struct.unpack('<hh',p.read(content+0x2bd1c+4*kind,4))
print('kind',hex(kind),'start',start,'count',count)
out=[]
for i in range(max(0,count)):
    dx=p.u(content+0x2a51c+2*(start+i),'H')
    row=p.read(content+0x510+dx*0x38,0x38)
    h,k,sub=struct.unpack_from('<IBxh',row)
    tag,_,offset,extra=struct.unpack_from('<IIQI',row,0x10)
    obj=resolve(tag)
    record={'i':i,'dx':dx,'hash':hex(h),'kind':hex(k),'sub':sub,'tag':hex(tag),'offset':hex(offset),'extra':hex(extra),'object':hex(obj) if obj else None,'row':row.hex()}
    if obj:
        try:
            data=p.read(obj+offset,0x580 if kind==0x20 else 0x400)
            record['data']=data.hex(); record['head']=data[:0x30].hex(' ')
        except OSError as e: record['read_error']=str(e)
    source_tag,source_offset=struct.unpack_from('<QQ',row,0x28)
    record['source_tag']=hex(source_tag); record['source_offset']=hex(source_offset)
    source=resolve(source_tag & 0xffffffff)
    if source:
        record['source_address']=hex(source+source_offset)
        record['source_data']=p.read(source+source_offset,0x60).hex()
    if obj and kind==0x20:
        record['body']=[(hex(o),data[o:o+16].hex(' ')) for o in range(0x1a0,0x538,16) if any(data[o:o+16])]
    out.append(record)
    print(json.dumps({k:v for k,v in record.items() if k not in ('row','data','source_data','body')}))
Path(__file__).with_name(f'namespace-{kind:02x}.json').write_text(json.dumps({'base':hex(base),'content':hex(content),'groups':groups,'rows':out},indent=2))
