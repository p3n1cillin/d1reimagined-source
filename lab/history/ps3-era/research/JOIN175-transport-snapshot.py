import sys
import json
import struct
from pathlib import Path
from datetime import datetime, timezone
sys.path.insert(0, r'F:\d1\d1xenia\scripts')
from rpcs3_owned_memory import OwnedMemory

manifest, object_text, output_text = sys.argv[1:]
obj = int(object_text, 0)
with OwnedMemory(manifest) as memory:
    def u32(address):
        return struct.unpack('>I', memory.read(address, 4))[0]
    inner = u32(obj+0x41c4)
    result = {'utc': datetime.now(timezone.utc).isoformat(), 'atomic': False, 'queue_object': hex(obj),
              'queue_fields114': memory.read(obj+0x114, 20).hex(), 'queue_consumed2124': u32(obj+0x2124),
              'inner': hex(inner), 'inner_state10f8': u32(inner+0x10f8),
              'inner_queue_objects': [hex(u32(inner+off)) for off in (0x50,0x54)],
              'send_state_1628_1643': memory.read(inner+0x1628, 28).hex(),
              'counter_1624': u32(inner+0x1624), 'forward_queues': []}
    for channel in (0,1):
        base = inner+channel*0x110
        raw = memory.read(base+0x1830, 12)
        cap, head, used = struct.unpack('>III',raw)
        q = {'channel': channel, 'capacity': cap, 'head': head, 'used': used, 'records': []}
        if cap <= 128 and used <= cap and (head < cap or used == 0):
            for offset in range(used):
                slot=base+0x183c+((head+offset)%cap)*4
                pointer=u32(slot)
                data=memory.read(pointer,0x128)
                length, bodyptr=struct.unpack_from('>II', data,0x114)
                q['records'].append({'pointer': hex(pointer), 'state': struct.unpack_from('>I',data)[0],
                                     'sequence': struct.unpack_from('>I',data,8)[0],
                                     'header':data[0x90:0x9e].hex(), 'body_length':length,
                                     'body_pointer':hex(bodyptr), 'pointer_unchanged':u32(slot)==pointer})
        q['queue_header_unchanged']=memory.read(base+0x1830,12)==raw
        result['forward_queues'].append(q)
    result['inner_pointer_unchanged']=u32(obj+0x41c4)==inner
with Path(output_text).open('x',encoding='utf-8') as out:
    json.dump(result,out,indent=2)
print(json.dumps(result,indent=2))
