exec(compile(open(__file__.replace('read-objective-definitions.py','inspect-namespace.py')).read().split('groups=[]')[0], 'inspect-namespace.py','exec'))
root=Path(__file__).parent
for tag in (0x80cb9104,0x80cb9107,0x80cb911c,0x80cb911f,0x80802993,0x80802590):
    a=resolve(tag)
    raw=p.read(a,0x600)
    (root/f'native-definition-{tag:08x}.bin').write_bytes(raw)
    print(hex(tag),hex(a))
    for o in range(0,0x180,16): print(f'  {o:03x}',raw[o:o+16].hex(' '))
for r in json.loads((root/'namespace-20.json').read_text())['rows']:
    if r['dx'] not in (14,17): continue
    raw=bytes.fromhex(r['data'])
    print('objective',r['dx'],r['object'])
    for o in range(0x40,0x1b0,16): print(f'  {o:03x}',raw[o:o+16].hex(' '))
