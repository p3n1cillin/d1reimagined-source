from pathlib import Path
import sys,struct,json,datetime,hashlib
sys.path.insert(0,'F:/d1/d1xenia/scripts')
from rpcs3_owned_memory import OwnedMemory
from rpcs3_final124_performance import Resources,u32
root=Path(__file__).parent
manifest=root.parent/'SIGNON_LOCAL/logs/FINAL124_JOIN195-processes.json'
target=0x0b30b1ec
with OwnedMemory(manifest) as mem:
    count=u32(mem.read,0x1a26188+4)
    if count>64:raise ValueError(f'unit library count {count} exceeds measured probe bound')
    records=mem.read(0x1a26188,8+count*20)
    resources=Resources(mem.read)
    rows=[]
    for index in range(count):
        record=records[index*20:(index+1)*20]
        handle=struct.unpack_from('>I',record,8)[0]
        try:
            address=resources.resolve(handle)
            header=mem.read(address,32)
            length,n,relative=struct.unpack_from('>III',header)
            if n>65536 or not relative or not relative+0x18+n*8<=length<=0x200000:
                raise ValueError('unsupported native library array')
            entries=mem.read(address+relative+0x18,n*8)
            matches=[{'index':i,'resource':hex(struct.unpack_from('>I',entries,i*8+4)[0])}
                     for i in range(n) if struct.unpack_from('>I',entries,i*8)[0]==target]
            names=[struct.unpack_from('>I',entries,i*8)[0] for i in range(n)]
            inversions=[i for i in range(1,n) if names[i]<names[i-1]]
            first,remaining,search=0,n,[]
            found=None
            while remaining:
                middle=first+(remaining>>1)
                name=names[middle]
                search.append({'index':middle,'name':hex(name)})
                if name==target:
                    found={'index':middle,'resource':hex(struct.unpack_from('>I',entries,middle*8+4)[0])}
                    break
                if target>name:
                    first=middle+1
                    remaining-=1
                remaining>>=1
            rows.append({'index':index,'handle':hex(handle),'address':hex(address),'count':n,'matches':matches,
                         'ascending_unsigned':not inversions,'inversions':inversions[:20],
                         'native_binary_search':search,'binary_result':found,
                         'entries_sha256':hashlib.sha256(entries).hexdigest(),
                         'header_unchanged':mem.read(address,32)==header})
        except (ValueError,OSError) as error:rows.append({'index':index,'handle':hex(handle),'error':str(error)})
    unchanged=mem.read(0x1a26188,8+count*20)==records and resources.unchanged()
out={'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'atomic':False,'count':count,'libraries':rows,'rechecks_pass':unchanged}
path=root/('JOIN195-unit-libraries-'+datetime.datetime.now(datetime.timezone.utc).strftime('%H%M%S')+'.json')
with path.open('x',encoding='utf-8') as stream:json.dump(out,stream,indent=2)
print(json.dumps(out))
