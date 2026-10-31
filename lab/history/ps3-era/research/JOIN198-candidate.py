from pathlib import Path
import sys, json, datetime, struct
sys.path.insert(0,'F:/d1/d1xenia/server')
import bap_server as bap
import investment
root=Path(__file__).parent
schema=investment.load_schema(Path('F:/d1/d1xenia-data/ps3_updates/types_01.24.json'))
layout=bap.character_layout(schema)
slot=11*layout['slot_size']
overlay={name:{str(layout[name+'_progression']):{str(layout['progression_items']):
         {str(slot):{'0xa':42,'0x18':{'0':{'0':15,'2':1}}}}}} for name in ('peer','select')}
path=root/'JOIN198-ghost-appearance-overlay.json'
if path.exists():
    assert json.loads(path.read_text(encoding='utf-8'))==overlay
else:
    with path.open('x',encoding='utf-8') as stream:json.dump(overlay,stream,indent=2);stream.write('\n')
extras=bap.character_extras(bap.gear_spec('starter:01.24:titan'),bap.overlay_spec('@'+str(path)),layout)
checks=[]
for name,handle in (('peer',0x808017fd),('select',0x80801954)):
    wire=investment.encode(schema,handle,extras[name])
    decoded=investment.decode(schema,handle,wire)
    item=decoded[layout[name+'_progression']][layout['progression_items']][slot]
    assert item[0]==0x10b and item[8]==862 and item[10]==42 and 12 not in item and item[24][0]=={0:15,2:1},item
    checks.append({'name':name,'wire_bytes':len(wire),'ghost_fields':{str(k):item.get(k) for k in (0,8,10,12,24)}})
(root/'JOIN198-overlay-verification.json').write_text(json.dumps({'at_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'checks':checks,'scope':'Private appearance plus Ghost channel 15/default cosmetic index 1 candidate; semantics and visibility require native confirmation.'},indent=2)+'\n',encoding='utf-8')
print(json.dumps(checks))
