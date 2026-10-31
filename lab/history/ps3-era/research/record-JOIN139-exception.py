import json, sys
from pathlib import Path
sys.path.insert(0, r'F:\d1\d1xenia\tools\content_capture')
from capture import Capture
from monitor import Detector, native_exception_observation
root=Path(r'F:\d1\<pc>-import\d1xenia\local')
manifest=json.loads((root/'SIGNON_LOCAL/logs/FINAL124_JOIN139-processes.json').read_text(encoding='utf-8-sig'))
owned=next(p for p in manifest['processes'] if p['tag']=='rpcs3')
records=json.loads((root/'SIGNON_LOCAL/logs/JOIN139-windows-crash-structured.json').read_text(encoding='utf-8-sig'))
if isinstance(records,dict): records=[records]
matched=[]
for record in records:
    data=record['data']
    if (int(data['ProcessId'],16)==owned['id'] and
        int(data['ProcessCreationTime'],16)+504911232000000000==int(owned['startTimeUtcTicks']) and
        data['AppPath'].casefold()==owned['executable'].casefold()): matched.append(record)
if len(matched)!=1: raise RuntimeError('Windows exception did not match exact owned identity')
record=matched[0]
observation=native_exception_observation(int(record['data']['ExceptionCode'],16))
if observation is None: raise RuntimeError('Unrecognized exception status')
feature,status,evidence,maturity=observation
evidence.update(source='identity-matched Windows Application Error', faulting_module='VkLayer_khronos_validation.dll',
    summary='The exact owned test process exited with access violation 0xc0000005 in VkLayer_khronos_validation.dll. This is a host validation-layer crash, not proof of Vulkan device loss or a game graphics repair.')
cap=Capture(root/'content-capture/config.json')
event=Detector(cap).observe(feature,status,evidence,maturity,run=manifest['name'],observed_at=record['at'])
(root/'SIGNON_LOCAL/logs/JOIN139-exception-capture-observation.json').write_text(json.dumps(event,indent=2))
print(json.dumps({'identity_match':True,'run':manifest['name'],'event_id':event['event_id'] if event else None,'at':record['at']}))