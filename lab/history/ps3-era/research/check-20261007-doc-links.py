from pathlib import Path
import json
import re
import subprocess

roots = [Path(r'F:\d1\d1xenia'), Path(r'F:\d1\d1xenia-data')]
checked = 0
missing = []
files = []
for root in roots:
    changed = subprocess.check_output(['git', '-C', str(root), 'diff', '--name-only'], text=True).splitlines()
    added = subprocess.check_output(['git', '-C', str(root), 'ls-files', '--others', '--exclude-standard'], text=True).splitlines()
    for relative in sorted(set(changed + added)):
        path = root / relative
        if path.suffix != '.md' or not path.exists():
            continue
        files.append(str(path))
        for target in re.findall(r'\[[^\]]*\]\(([^)]+)\)', path.read_text(encoding='utf-8')):
            if '://' in target or target.startswith('#'):
                continue
            target = target.split('#', 1)[0].strip('<>')
            checked += 1
            if not (path.parent / target).exists():
                missing.append({'document': relative, 'target': target})
receipt = {'files': files, 'relative_links_checked': checked, 'missing': missing}
Path(r'F:\d1\<pc>-import\d1xenia\local\RESEARCH\20261007-doc-links.json').write_text(json.dumps(receipt, indent=2), encoding='utf-8')
print(json.dumps(receipt, indent=2))
assert not missing
