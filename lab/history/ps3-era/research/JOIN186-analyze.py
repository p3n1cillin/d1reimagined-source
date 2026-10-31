import collections, datetime, json, struct, sys
from pathlib import Path
root = Path(__file__).parent
all_rows = [json.loads(line) for line in (root/'JOIN186-performance-timeline.jsonl').read_text(encoding='utf-8').splitlines()]
rows = [row for row in all_rows if row.get('initialized')]
summary = {'at':datetime.datetime.now(datetime.timezone.utc).isoformat(), 'samples':len(rows),
           'first':rows[0]['utc'], 'last':rows[-1]['utc'],
           'unchanged_failures':sum(not r['unchanged_during_read'] for r in rows),
           'errors':collections.Counter(p.get('error') for r in rows for p in r['performances'] if p.get('error')),
           'performances':{}, 'actors':[]}
for tag in (0x80cb8d65,0x80cb8d5b):
    selected = [(r,p) for r in rows for p in r['performances'] if p.get('asset') == tag]
    states = collections.Counter((p.get('phase_raw'),p.get('scene_index')) for r,p in selected)
    active = [(r,p) for r,p in selected if p.get('scene_index',-1) >= 0]
    transitions = []; previous = None
    for r,p in selected:
        state = p.get('phase_raw'),p.get('scene_index')
        if state != previous:
            transitions.append({'utc':r['utc'],'phase':state[0],'scene':state[1]})
            previous = state
    item = {'name':selected[0][1]['name'], 'states':{str(k):v for k,v in states.items()},'transitions':transitions}
    if active:
        sample,p = active[len(active)//2]
        item['sample_utc']=sample['utc']; item['scene_definitions']=[{k:s.get(k) for k in ('index','handle','offset','definition','definition_bytes')} for s in p['scenes']]
        item['authored_words']=[f'{v:08x}' for v in struct.unpack('>48I',bytes.fromhex(p['authored_bytes']))]
    summary['performances'][f'{tag:08x}']=item
for index in range(6):
    values = collections.Counter(r['actor_slots'][index] for r in rows)
    summary['actors'].append({'index':index,'variants':len(values),'most_common':[{'samples':n,'bytes':v} for v,n in values.most_common(3)]})
(root/'JOIN186-performance-analysis.json').write_text(json.dumps(summary,indent=2)+'\n',encoding='utf-8')
print(json.dumps({k:v for k,v in summary.items() if k not in ('performances','actors')}))
for tag,item in summary['performances'].items():print(tag,item['name'],item['states'],item['transitions'])
print('actor variants',[(a['index'],a['variants']) for a in summary['actors']])
sys.path.insert(0,'F:/d1/d1xenia/scripts')
from d1_eboot import Eboot
eb=Eboot(root/'ps3_updates/01.24/USRDIR/EBOOT.elf')
out=root/'JOIN186-static';out.mkdir(exist_ok=True)
for pc,count in ((0x42a60c,120),(0x42e890,180),(0x42e678,16),(0x42fb70,200),(0x42f3a0,360)):
    lines=[]
    for address in range(pc,pc+4*count,4):lines.extend(list(eb.disasm(address,1)) or [f'{address:08x}: .long {eb.u32(address):08x}'])
    (out/f'{pc:x}.txt').write_text('\n'.join(lines)+'\n',encoding='utf-8')

