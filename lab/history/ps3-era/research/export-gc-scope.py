from pathlib import Path
import hashlib
import json
import subprocess

source = Path(r'F:\d1\<pc>-import\dev\rpcs3')
baseline = Path(r'F:\d1\<pc>-import\d1xenia\local\RESEARCH\gpu-gc-scope-baseline')
export = baseline.parent / 'gpu-gc-scope-export'
patch = Path(r'F:\d1\d1xenia\patches\rpcs3-gc-scope.patch')
files = ['rpcs3/Emu/RSX/VK/VKResourceManager.h']

def run(*args):
    return subprocess.check_output(['git', '-C', str(export), *args], stderr=subprocess.STDOUT)

def normalized(path):
    return path.read_text(encoding='utf-8').replace('\r\n', '\n').encode('utf-8')

export.mkdir(exist_ok=False)
run('init', '-q')
run('config', 'core.autocrlf', 'false')
for relative in files:
    target = export / relative
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(normalized(baseline / relative))
run('add', '--', *files)
run('-c', 'user.name=Fixture', '-c', 'user.email=fixture@invalid', 'commit', '-qm', 'Exact private pre-change GC baseline')
for relative in files:
    (export / relative).write_bytes(normalized(source / relative))
patch.write_bytes(run('diff', '--no-ext-diff', '--', *files))
print(run('diff', '--stat').decode())
run('reset', '--hard', '-q', 'HEAD')
run('apply', '--check', str(patch))
run('apply', str(patch))
matches = {relative: normalized(export / relative) == normalized(source / relative) for relative in files}
receipt = {'patch_sha256': hashlib.sha256(patch.read_bytes()).hexdigest(), 'source_matches_applied_patch': matches}
(export.parent / 'gpu-gc-scope-reproduction.json').write_text(json.dumps(receipt, indent=2))
assert all(matches.values()), receipt
print(json.dumps(receipt, indent=2))
