"""Extract only owned package blocks containing entity definitions; outputs private."""
from pathlib import Path
import collections,json,os,re,struct,subprocess,time
ROOT=Path(__file__).parent;BASE=Path(r'F:\d1\d1preservation-main')
env=dict(os.environ,D1_BUILD='later-56578',D1_BUILDS=str(BASE/'capture-rig/builds.json'))
oracle=BASE/'d1-server/target/release/d1-oracle.exe'
entries=[e for e in json.loads((ROOT/'entries-025c-v2.json').read_text()) if e['class_tag']==0x80800861]
cache=ROOT/'owned-definition-blocks';cache.mkdir(exist_ok=True)
def block(n):
    dest=cache/f'{n}.bin'
    if not dest.exists():
        r=subprocess.run([str(oracle),'oodle','block','--pkg','ps4_cosmo_activities_activity_025c_5.pkg','--block',str(n),'--out',str(dest)],env=env,capture_output=True,text=True,timeout=120)
        if r.returncode:raise RuntimeError(r.stderr)
    return dest.read_bytes()
found=[]
for number,n in enumerate(sorted({e['loc']&0x3fff for e in entries}),1):
    plain=block(n)
    for e in entries:
        if e['loc']&0x3fff!=n:continue
        offset=((e['loc']>>14)&0x3fff)*16;size=(e['loc']>>28)&0x3fffffff
        data=plain[offset:offset+size]
        if len(data)<size:data+=block(n+1)[:size-len(data)]
        assert len(data)==size
        if struct.pack('<I',0x062dd95d) not in data:continue
        tag=0x80cb8000+e['index']
        strings=[s.decode() for s in re.findall(rb'[ -~]{6,}',data)]
        dest=ROOT/f'owned-definition-{tag:08x}.bin';dest.write_bytes(data)
        found.append({'tag':hex(tag),'bytes':size,'strings':strings,'file':dest.name})
    if number%15==0:print(f'{number} blocks; {len(found)} Dock references',flush=True)
dest=ROOT/'dock-definition-index.json';dest.write_text(json.dumps(found,indent=2),encoding='utf8')
print(json.dumps(found,indent=2))
