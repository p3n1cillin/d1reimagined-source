exec(compile(open(__file__.replace('squad-position-summary.py','inspect-namespace.py')).read().split('groups=[]')[0], 'inspect-namespace.py','exec'))
for r in json.loads(Path(__file__).with_name('namespace-01.json').read_text())['rows']:
    obj=int(r['object'],16); tag=p.u(obj,'I'); off=p.u(obj+8,'Q'); definition=resolve(tag)+off
    child=p.u(definition+0x68,'I'); childaddr=resolve(child)+p.u(definition+0x70,'Q')
    raw=p.read(childaddr,p.u(childaddr,'I'))
    out=[]
    for o in range(0,len(raw)-12,4):
        x,y,z=struct.unpack_from('<fff',raw,o)
        if 50<x<500 and -850<y<-420 and -40<z<100: out.append((hex(o),tuple(round(v,2) for v in (x,y,z))))
    print(hex(tag),hex(child),'size',len(raw),'positions',out[:12])
    Path(__file__).with_name(f'native-squad-definition-{child:08x}.bin').write_bytes(raw)
