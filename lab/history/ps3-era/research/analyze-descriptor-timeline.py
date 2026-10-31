from pathlib import Path
import collections,json,re,sys
path=Path(sys.argv[1])
counts=collections.Counter();tags={};submitted={};completed_by_queue={};callbacks={};retire_by_pool={};resets={};failures=[];cap=False;last_trace=None
h=lambda v:f'{int(v,16):016x}'
with path.open(encoding='utf-8',errors='replace') as f:
    for number,line in enumerate(f,1):
        if 'D1DESC ' in line:
            payload=line.split('D1DESC ',1)[1].strip();kind=payload.split(' ',1)[0];counts[kind]+=1;last_trace={'line':number,'message':payload}
            values=dict(re.findall(r'([a-z_]+)=(-?[a-zA-Z0-9]+)',payload))
            if kind.startswith('cap='):cap=True;continue
            if kind=='tag':
                tags[values['cmd']]={'eid':int(values['eid']),'cmd':values['cmd'],'fence':values['fence'],'tag_line':number,'completed':False}
            elif kind=='submit':
                cmd=values['cmd'];entry=dict(tags.get(cmd,{}));entry.update(order=int(values['order']),queue=values['queue'],result=int(values['result']),submit_line=number,fence=values['fence'])
                submitted[cmd]=entry
            elif kind=='completed':
                cmd=values['cmd'];entry=submitted.get(cmd,{})
                if entry.get('eid')==int(values['eid']):
                    entry['completed']=True;entry['completion_line']=number
                    queue=entry.get('queue')
                    if queue and entry.get('result')==0 and entry.get('order',0)>completed_by_queue.get(queue,{}).get('order',0):completed_by_queue[queue]=dict(entry)
            elif kind=='retire':
                entry=dict(values,line=number);callbacks[values['object']]=entry;retire_by_pool[values['pool']]=entry
            elif kind=='dispose' and values['object'] in callbacks:
                callbacks[values['object']].update(scope=int(values['scope']),dispose_line=number,assigned_current=int(values['current']))
            elif kind=='reset':
                pool=values['pool'];retirement=dict(retire_by_pool.get(pool,{}));resets[pool]={'line':number,'reset':values,'retirement':retirement}
        match=re.search(r'vkResetDescriptorPool\(\).*VkDescriptorPool 0x([0-9a-fA-F]+).*VkCommandBuffer 0x([0-9a-fA-F]+)',line)
        if match:
            pool,cmd=map(h,match.groups());context=dict(resets.get(pool,{}));entry=dict(submitted.get(cmd,{}));context.update(validation_line=number,validation=line.strip(),pool=pool,pending_command=cmd,last_tag=tags.get(cmd),last_submission=entry,highest_completed_same_queue=completed_by_queue.get(entry.get('queue')))
            failures.append(context)
result={'source':str(path),'trace_counts':dict(counts),'cap_reached':cap,'last_trace':last_trace,'descriptor_failures':failures,'scope':'Private handle correlation; hypotheses require native validation. No visual acceptance.'}
output=path.with_name(path.stem+'-descriptor-analysis.json');output.write_text(json.dumps(result,indent=2),encoding='utf-8')
print(json.dumps(result,indent=2))