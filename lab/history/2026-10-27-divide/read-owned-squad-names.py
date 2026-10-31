"""Recover authored labels from the owned adjacent namespace entries. Never a public fixture."""
import os, re, subprocess
from pathlib import Path
p = Path(__file__).resolve().parent
root = Path(r'F:\d1\d1preservation-main')
env = dict(os.environ,D1_BUILDS=str(root/'capture-rig/builds.json'),D1_BUILD='later-56578')
for tag in range(0x80CB8454,0x80CB8460):
    out=p/f'owned-entry-{tag:08x}.bin'
    if tag==0x80CB8454:
        data=(p/'squad4-owned-entry.bin').read_bytes()
    else:
        args=[str(root/'d1-server/target/release/d1-oracle.exe'),'oodle','entry',
              '--tag',hex(tag),'--out',str(out),'--hex','0']
        result=subprocess.run(args,capture_output=True,text=True,env=env,timeout=100)
        (p/f'owned-entry-{tag:08x}.log').write_text(result.stdout+result.stderr)
        if result.returncode or 'self-length header:' not in result.stdout or 'MATCHES' not in result.stdout:
            raise RuntimeError(f'Owned content extraction/control failed for {tag:#x}; do not cite')
        data=out.read_bytes()
    names=[s.decode() for s in re.findall(rb'sq_[a-zA-Z0-9_]+',data)]
    print(hex(tag),names,flush=True)
