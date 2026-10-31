from pathlib import Path
import hashlib
import re

root = Path(r'F:\d1\d1xenia-data')
relative = 'ps3_updates/EXPORT_VALIDATION.md'
contents = (root / relative).read_bytes()
manifest = root / 'SHA256SUMS.txt'
text = manifest.read_text(encoding='utf-8')
replacement = f'{hashlib.sha256(contents).hexdigest()}  {len(contents)}  {relative}'
text, count = re.subn(r'^[a-f0-9]{64}\s+\d+\s+ps3_updates/EXPORT_VALIDATION\.md$',
                      replacement, text, flags=re.MULTILINE)
assert count == 1, count
manifest.write_text(text, encoding='utf-8', newline='\n')
print(replacement)
