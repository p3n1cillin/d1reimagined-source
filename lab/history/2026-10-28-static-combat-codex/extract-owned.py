from pathlib import Path
import json,os,struct,subprocess,sys
ROOT=Path(__file__).parent
BASE=Path(r'F:\d1\d1preservation-main')
ORACLE=BASE/'d1-server/target/release/d1-oracle.exe'
ENV=dict(os.environ,D1_BUILD='later-56578',D1_BUILDS=str(BASE/'capture-rig/builds.json'))
for arg in sys.argv[1:]:
    tag=int(arg,0);dest=ROOT/f'owned-{tag:08x}.bin'
    if not dest.exists():
        r=subprocess.run([str(ORACLE),'oodle','entry','--tag',hex(tag),'--out',str(dest)],env=ENV,capture_output=True,text=True,timeout=120)
        if r.returncode:raise RuntimeError(r.stderr)
    b=dest.read_bytes();print(hex(tag),len(b),b[:0x40].hex(' '),flush=True)
