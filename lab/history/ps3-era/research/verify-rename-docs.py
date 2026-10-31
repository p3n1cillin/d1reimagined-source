from pathlib import Path
import json
import re
import subprocess
from datetime import datetime, timezone

roots = [Path(r'F:\d1\d1xenia'), Path(r'F:\d1\d1xenia-data')]
files = []
broken = []
for root in roots:
    tracked = subprocess.check_output(['git', '-C', str(root), 'ls-files'], text=True).splitlines()
    for relative in tracked:
        path = root / relative
        if path.name.lower() != 'readme.md':
            continue
        files.append(str(path))
        text = path.read_text(encoding='utf-8')
        assert 'D1 Reimagined' in text, path
        assert '2026-10-06' in text, path
        assert 'https://github.com/p3n1cillin/d1xenia' not in text, path
        for target in re.findall(r'\[[^\]]*\]\(([^)]+)\)', text):
            if '://' in target or target.startswith('#'):
                continue
            target = target.split('#', 1)[0]
            if not (path.parent / target).exists():
                broken.append({'file': relative, 'target': target})
assert len(files) == 6, files
assert not broken, broken
receipt = {'at': datetime.now(timezone.utc).isoformat(), 'readmes_reviewed': files,
           'readme_count': len(files), 'broken_local_links': broken,
           'brand_date_and_current_repository_links': 'PASS'}
Path(r'F:\d1\<pc>-import\d1xenia\local\RESEARCH\rename-readme-verification.json').write_text(json.dumps(receipt, indent=2))
print('Six README branding/date/current URL and relative-link checks PASS.')
