from pathlib import Path
import hashlib
import json
import subprocess

root = Path(r'F:\d1\<pc>-import\d1xenia\local\RESEARCH')
fixture = root / 'rsx-replay-context-patch-fixture2'
fixture.mkdir(exist_ok=False)
subprocess.run(['git', 'init', '-q', str(fixture)], check=True)
subprocess.run(['git', '-C', str(fixture), 'config', 'core.autocrlf', 'false'], check=True)
target = fixture / 'rpcs3/Emu/System.cpp'
target.parent.mkdir(parents=True)
baseline = root / 'System.before-replay-null-page-20261007.cpp'
target.write_bytes(baseline.read_bytes().replace(b'\r\n', b'\n'))
patch = Path(r'F:\d1\d1xenia\patches\rpcs3-native-rsx-replay-context.patch')
for args in (['--check'], []):
    subprocess.run(['git', 'apply', *args, str(patch)], cwd=fixture, check=True)
actual = Path(r'F:\d1\<pc>-import\dev\rpcs3\rpcs3\Emu\System.cpp')
assert target.read_bytes() == actual.read_bytes().replace(b'\r\n', b'\n')
receipt = {'apply_check': 'PASS', 'apply': 'PASS', 'matches_actual_source_normalized': True,
           'patch_sha256': hashlib.sha256(patch.read_bytes()).hexdigest(),
           'scope': 'Isolated replay initialization change only; native replay pending.'}
(fixture / 'receipt.json').write_text(json.dumps(receipt, indent=2))
print(json.dumps(receipt))
