exec(compile(open(__file__.replace('inspect-objective-text.py','inspect-namespace.py')).read().split('groups=[]')[0], 'inspect-namespace.py','exec'))
for tag in (0x80cb9104,0x80cb911c):
    a=resolve(tag); n=p.u(a,'I'); raw=p.read(a,n)
    print(hex(tag),hex(a),'length',n)
    for o in range(0x538,min(n,0x690),16): print(f'+{o:03x}',raw[o:o+16].hex(' '))
    print('strings',__import__('re').findall(rb'[\x20-\x7e]{8,}',raw))
for sub,obj in ((14,0x3802610f0),(17,0x38025ca30)):
    print('LIVE',sub)
    for o in range(0x1b0,0x220,16): print(hex(o),p.read(obj+o,16).hex(' '))
