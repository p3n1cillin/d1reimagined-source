from pathlib import Path
import hashlib
import json
import subprocess

source = Path(r'F:\d1\<pc>-import\dev\rpcs3')
research = Path(r'F:\d1\<pc>-import\d1xenia\local\RESEARCH')
baseline = research / 'descriptor-allocation-20261007-baseline'
export = research / 'descriptor-allocation-20261007-export'
patch = Path(r'F:\d1\d1xenia\patches\rpcs3-descriptor-allocation-cache.patch')
relative = 'rpcs3/Emu/RSX/VK/vkutils/descriptors.cpp'

def run(*args):
    return subprocess.check_output(['git', '-C', str(export), *args], stderr=subprocess.STDOUT)

def normalized(path):
    return path.read_text(encoding='utf-8').replace('\r\n', '\n').encode('utf-8')

export.mkdir(exist_ok=False)
run('init', '-q')
run('config', 'core.autocrlf', 'false')
target = export / relative
target.parent.mkdir(parents=True, exist_ok=True)
target.write_bytes(normalized(baseline / 'descriptors.cpp'))
run('add', '--', relative)
run('-c', 'user.name=Fixture', '-c', 'user.email=fixture@invalid', 'commit', '-qm', 'Exact pre-change descriptor allocation baseline')
target.write_bytes(normalized(source / relative))
patch.write_bytes(run('diff', '--no-ext-diff', '--', relative))
# Only this newly created private fixture is reset. The renderer is never reset.
run('reset', '--hard', '-q', 'HEAD')
run('apply', '--check', str(patch))
run('apply', str(patch))
matches = normalized(target) == normalized(source / relative)
receipt = {'patch_sha256': hashlib.sha256(patch.read_bytes()).hexdigest(),
           'source_matches_applied_patch': matches,
           'actual_source_sha256': hashlib.sha256(normalized(source / relative)).hexdigest(),
           'baseline_source_sha256': hashlib.sha256(normalized(baseline / 'descriptors.cpp')).hexdigest()}
(research / 'descriptor-allocation-20261007-reproduction.json').write_text(json.dumps(receipt, indent=2), encoding='utf-8')
assert matches, receipt
print(json.dumps(receipt, indent=2))
