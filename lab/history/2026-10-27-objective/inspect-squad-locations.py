exec(compile(open(__file__.replace('inspect-squad-locations.py','inspect-namespace.py')).read().split('groups=[]')[0], 'inspect-namespace.py','exec'))
for r in json.loads(Path(__file__).with_name('namespace-01.json').read_text())['rows']:
    obj=int(r['object'],16); tag=p.u(obj,'I'); off=p.u(obj+8,'Q'); definition=resolve(tag)+off
    child=p.u(definition+0x68,'I'); childoff=p.u(definition+0x70,'Q')
    print(hex(tag),'def',hex(definition),'child',hex(child),hex(childoff))
    raw=p.read(definition,0x100)
    print('definition',raw.hex())
    childaddr=resolve(child)
    if childaddr:
        raw=p.read(childaddr+childoff,0x180)
        print('child',hex(childaddr+childoff),raw.hex())
