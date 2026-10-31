from pathlib import Path
from datetime import datetime, timezone
import json
import subprocess

rt=Path(r'F:\d1\<pc>-import\d1xenia\local')
out=rt/'RESEARCH/JOIN174-filter-reproduction-lf'
out.mkdir(exist_ok=False)
subprocess.run(['git','init','--quiet',str(out)],check=True)
for relative,name in [('rpcs3/Emu/RSX/d1diag.h','d1diag.h'),('rpcs3/Emu/Cell/PPUThread.cpp','PPUThread.cpp')]:
    path=out/relative
    path.parent.mkdir(parents=True,exist_ok=True)
    path.write_bytes((rt/'RESEARCH/JOIN172-source-before'/name).read_text(encoding='utf-8').encode('utf-8'))
original=Path(r'F:\d1\d1xenia\patches\rpcs3-ppu-watch-filters.patch')
patch=out/'patch-under-test.patch'
patch.write_bytes(original.read_text(encoding='utf-8').encode('utf-8'))
subprocess.run(['git','-c','core.autocrlf=false','apply','--check','--whitespace=error-all',str(patch)],cwd=out,check=True)
subprocess.run(['git','-c','core.autocrlf=false','apply','--whitespace=error-all',str(patch)],cwd=out,check=True)
root=Path(r'F:\d1\<pc>-import\dev\rpcs3')
checks={name:(out/name).read_text(encoding='utf-8') == (root/name).read_text(encoding='utf-8') for name in
        ('rpcs3/Emu/RSX/d1diag.h','rpcs3/Emu/Cell/PPUThread.cpp','rpcs3/Emu/RSX/Common/ppu_watch_filter.h')}
assert all(checks.values()),checks
receipt={'at':datetime.now(timezone.utc).isoformat(),'git_apply_check_whitespace':'PASS','normalized_files_reproduced':checks,
         'supersedes':'JOIN174-filter-reproduction: initial fixture wrote CRLF through Windows text IO; LF fixture matches Git patch context. Both failed fixture and original renderer files preserved.'}
(out/'receipt.json').write_bytes(json.dumps(receipt,indent=2).encode('utf-8'))
print(json.dumps(receipt,indent=2))
