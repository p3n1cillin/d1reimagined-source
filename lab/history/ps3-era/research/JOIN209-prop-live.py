"""Exactly owned, read-only sample of units observed in this run's creator trace."""
import json, re, struct, sys
from pathlib import Path
sys.path.insert(0, 'F:/d1/d1xenia/scripts')
from rpcs3_owned_memory import OwnedMemory
from rpcs3_final124_performance import Resources, u32
from rpcs3_final124_attachment import unit_membership, UNIT_POOL
from d1_guardian_route import open_shared_read
root=Path(__file__).parent
manifest=root.parent/'SIGNON_LOCAL/logs/FINAL124_JOIN209-processes.json'
log=root.parent/'RPCS3_FINAL124_SMOKE001/log/RPCS3.log'
pending={}; observed=[]; lines=[]
with open_shared_read(log) as stream:
    trace=stream.read(100*1024*1024).decode('utf-8',errors='replace').splitlines(True)
for line in trace:
    if not ('pc=0x13f068 ' in line or 'pc=0x2145d0 ' in line): continue
    lines.append(line)
    stack=re.search(r'r1=(0x[0-9a-f]+)',line)
    if not stack: continue
    if 'pc=0x13f068 ' in line:
        m=re.search(r'm4=([0-9a-f]+)',line)
        if m: pending[stack[1]]={'unit_asset':hex(int(m[1][:8],16)),'creation_bytes':m[1]}
    else:
        m=re.search(r'm1\+70=([0-9a-f]{8})',line)
        if m and stack[1] in pending:
            observed.append({**pending.pop(stack[1]),'datum':int(m[1],16)})
(root/'JOIN209-prop-creator-lines.txt').write_text(''.join(lines),encoding='utf-8')
with OwnedMemory(manifest) as mem:
    read=mem.read
    resources=Resources(read)
    pool=read(UNIT_POOL,0x40)
    base,stride=struct.unpack_from('>II',pool,8)
    if stride!=0xc0: raise ValueError('unexpected unit stride')
    for result in observed:
        try:
            datum=result['datum']; result['datum']=hex(datum)
            membership=unit_membership(read,datum); result.update(membership)
            if not all(membership[k] for k in ('unit_bank_matches','unit_index_in_range','unit_mask_bit')): continue
            address=base+(datum&0x1fff)*stride
            row=read(address,stride)
            entity_datum=struct.unpack_from('>I',row,0x78)[0]
            result['row']=row.hex();result['entity_datum']=hex(entity_datum)
            if entity_datum!=0xffffffff:
                entity=resources.resolve(entity_datum)
                result.update(entity=hex(entity),entity_bytes=read(entity,0x60).hex(),xyz_candidate=struct.unpack('>3f',read(entity+0x40,12)))
            result['row_unchanged']=read(address,stride)==row
            result['membership_unchanged']=unit_membership(read,datum)==membership
        except (ValueError,OSError) as exc: result['error']=str(exc)
    result={'scope':'read-only non-atomic creator-handle sample; pose offset requires native-layout confirmation','registry_unchanged':resources.unchanged(),'unit_pool_unchanged':read(UNIT_POOL,0x40)==pool,'units':observed}
    (root/'JOIN209-prop-live.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result,indent=2))
