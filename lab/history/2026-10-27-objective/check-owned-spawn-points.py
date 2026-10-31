import json,os,struct,subprocess
from pathlib import Path
root=Path(__file__).parent
env=dict(os.environ,D1_BUILDS=r'F:\d1\d1preservation-main\capture-rig\builds.json',D1_BUILD='later-56578')
oracle=r'F:\d1\public-work\d1reimagined-source\ps4\d1-server\target\release\d1-oracle.exe'
rows=[]
for tag in (0x80cc380d,0x80cc3810,0x80cc3811,0x80cc3812,0x80cc3813):
    out=root/f'owned-spawn-{tag:08x}.bin'
    r=subprocess.run([oracle,'oodle','entry','--tag',hex(tag),'--out',str(out),'--hex','0'],env=env,capture_output=True,text=True,timeout=90)
    (root/f'owned-spawn-{tag:08x}.log').write_text(r.stdout+r.stderr)
    assert r.returncode==0 and 'MATCHES' in r.stdout, r.stderr
    owned=out.read_bytes(); native=(root/f'native-squad-definition-{tag:08x}.bin').read_bytes()
    positions=[]
    for off in range(0,len(owned)-12,4):
        x,y,z=struct.unpack_from('<fff',owned,off)
        if 50<x<500 and -850<y<-420 and -40<z<100:
            assert owned[off:off+12]==native[off:off+12]
            positions.append((hex(off),[round(v,2) for v in (x,y,z)]))
    rows.append({'tag':hex(tag),'size':len(owned),'positions_match_native':positions})
    print(json.dumps(rows[-1]),flush=True)
(root/'owned-spawn-summary.json').write_text(json.dumps(rows,indent=2))
