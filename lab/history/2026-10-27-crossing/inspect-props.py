exec(compile(open(__file__.replace('inspect-props.py','inspect-namespace.py')).read().split('groups=[]')[0], 'inspect-namespace.py','exec'))
root=Path(__file__).parent
for r in json.loads((root/'namespace-04.json').read_text())['rows']:
    if r['dx']<209 or not r['object']: continue
    obj=int(r['object'],16); tag=p.u(obj,'I'); definition=resolve(tag)+p.u(obj+8,'Q')
    raw=p.read(definition,0x1a0)
    print(hex(tag),'component',hex(obj),'entity+390',hex(p.u(obj+0x390,'I')),'def',hex(definition))
    print('state+1ac',p.read(obj+0x1ac,0x100).hex())
    print('def header',raw[:0xb0].hex())
    for o in range(0x40,0xa0,4):
        v=struct.unpack_from('<I',raw,o)[0]
        if v&0xff000000==0x80000000:
            print('tagfield',hex(o),hex(v))
    (root/f'prop-{tag:08x}-component.bin').write_bytes(p.read(obj,0x3d0))
    (root/f'prop-{tag:08x}-def.bin').write_bytes(raw)
