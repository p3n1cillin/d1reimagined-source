"""Export reviewed source paths into a fresh ps4/ directory, without private history."""
import json
from pathlib import Path
import shutil
import subprocess

src = Path(r'F:\d1\d1preservation-main')
target = Path(r'F:\d1\public-work\d1reimagined-source')
out = target / 'ps4'
if out.exists(): raise SystemExit('Refusing an existing export')
allowed = {'.rs','.toml','.lock','.md','.py','.sh','.proto','.h','.ps1','.java',
           '.json','.jsonl','.xml','.txt','.html'}
pruned = {'target','dumps','cfw-staging','external','historicaldocs','runs',
          'logs','captures','catalogue','manifest','.git','.claude','.codex','__pycache__'}
excluded_dirs = ('capture-rig/characters/','capture-rig/inventory/','capture-rig/profiles/',
                 'capture-rig/rosters','capture-rig/unlocks/','capture-rig/manifests/',
                 'capture-rig/schemas/','capture-rig/testdata/','d1-observe/fixtures/',
                 'ghidra/lists/','ghidra/structs/',
                 'd1-server/crates/d1-eboot/tests/fixtures/',
                 'd1-server/crates/d1-scope/tests/fixtures/')
excluded_files = {'CLAUDE.md','README.md','status/cfw-console.md',
                  'status/LOCAL-INPUTS-2026-10-11.md','capture-rig/launch_knowledge.json'}
copied = []; excluded = []
def walk(directory):
    for p in sorted(directory.iterdir()):
        rel = p.relative_to(src).as_posix()
        if p.is_symlink() or p.name in pruned:
            continue
        if p.is_dir():
            if not any((rel+'/').startswith(x) for x in excluded_dirs): walk(p)
            continue
        if rel in excluded_files or (p.suffix not in allowed and p.name not in {'LICENSE','.gitignore'}):
            excluded.append(rel); continue
        dest = out / rel
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(p, dest); copied.append(rel)
walk(src)
# Record only source provenance and exclusion categories, not private directory inventories.
(target/'docs').mkdir(exist_ok=True)
receipt = {'date':'2026-10-12','private_project_commit':subprocess.check_output(
    ['git','-C',r'F:\d1\d1xenia','rev-parse','HEAD']).decode().strip(),
    'ps4_source':'User-supplied d1preservation-main source snapshot plus documented local fixes',
    'files':len(copied),'history_imported':False,'game_inputs_included':False,
    'exclusions':['game/firmware binaries','native derived fixtures and catalogues',
                  'captures and raw logs','private machine status and operator instructions',
                  'build output and third-party tool installations']}
(target/'docs'/'SOURCE-PROVENANCE.json').write_text(json.dumps(receipt,indent=2)+'\n')
print(json.dumps({'copied':len(copied),'excluded_files':len(excluded)}))
