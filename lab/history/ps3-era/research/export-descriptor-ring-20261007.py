from pathlib import Path
import hashlib
import json
import subprocess

source = Path(r'F:\d1\<pc>-import\dev\rpcs3')
research = Path(r'F:\d1\<pc>-import\d1xenia\local\RESEARCH')
baseline = research / 'descriptor-ring-20261007-baseline'
export = research / 'descriptor-ring-20261007-export'
verify = research / 'descriptor-ring-20261007-reproduction'
patch = Path(r'F:\d1\d1xenia\patches\rpcs3-descriptor-lifetime-ring.patch')
old = json.loads((baseline / 'baseline.json').read_text(encoding='utf-8-sig'))['files']
added = 'rpcs3/Emu/RSX/Common/descriptor_lifetime_ring.h'

def run(root, *args):
    return subprocess.check_output(['git','-C',str(root),*args],stderr=subprocess.STDOUT)

def normalized(path):
    return path.read_text(encoding='utf-8').replace('\r\n','\n').encode('utf-8')

for root in (export,verify):
    root.mkdir(exist_ok=False)
    run(root,'init','-q')
    run(root,'config','core.autocrlf','false')
    for relative in old:
        target = root / relative
        target.parent.mkdir(parents=True,exist_ok=True)
        target.write_bytes(normalized(baseline / relative))
    run(root,'add','--',*old)
    run(root,'-c','user.name=Fixture','-c','user.email=fixture@invalid','commit','-qm','Exact pre-ring private baseline')
for relative in [*old,added]:
    target = export / relative
    target.parent.mkdir(parents=True,exist_ok=True)
    target.write_bytes(normalized(source / relative))
run(export,'add','-N','--',added)
patch.write_bytes(run(export,'diff','--no-ext-diff','--',*old,added))
run(verify,'apply','--check',str(patch))
run(verify,'apply',str(patch))
matches = {relative: normalized(verify / relative) == normalized(source / relative) for relative in [*old,added]}
assert all(matches.values())
receipt = {'patch_sha256':hashlib.sha256(patch.read_bytes()).hexdigest(),
           'source_matches_applied_patch':matches,
           'actual_source_sha256':{relative:hashlib.sha256(normalized(source / relative)).hexdigest() for relative in [*old,added]},
           'scope':'Only the bounded ring increment against the exact private baseline; unrelated renderer work excluded.'}
(research / 'descriptor-ring-20261007-patch-reproduction.json').write_text(json.dumps(receipt,indent=2),encoding='utf-8')
print(json.dumps(receipt,indent=2))
