"""Inventory only the retired Xbox/Xenia portion of this project's local tree."""
import json
import os
from pathlib import Path

runtime = Path('F:/d1/<pc>-import/d1xenia/local')
targets = [(p, 'retired emulator build/test') for p in runtime.iterdir() if p.name.upper().startswith('XENIA')]
for rel, reason in (
    ('00_ORIGINAL_DUMP/HDD_CAPTURE_2026-09-23', 'Xbox HDD capture'),
    ('00_ORIGINAL_DUMP/USB_CAPTURE_2026-09-22', 'Xbox USB capture'),
    ('01_WORKING/test001', 'Xbox installed-game working copy'),
    ('01_WORKING/test002_packages', 'Xbox content working copy'),
    ('02_EXTRACTED/Content1_WORKAROUND', 'Xbox STFS extraction'),
    ('TOOLS/Extract-STFS', 'Xbox-only extractor'),
    ('TOOLS/XBLA-Extract', 'Xbox-only extractor'),
    ('TOOLS/xenia-source-74c4e4acb', 'retired emulator source'),
    ('TOOLS/xenia_canary_windows_74c4e4a.7z', 'retired emulator download'),
):
    p = runtime / rel
    if p.exists(): targets.append((p, reason))
for p in (runtime / 'LOGS').iterdir():
    if p.is_file() and (p.name.startswith(('Gfx-', 'Test0', 'Content', 'Xenia-', 'usb_copy_', 'working_copy_test001'))
                        or 'STFS' in p.name):
        targets.append((p, 'Xbox experiment output'))
for p in (runtime / 'LOGS').iterdir():
    if p.is_dir() and p.name == 'console-network-capture':
        targets.append((p, 'Xbox reference network capture'))
for p in (runtime / 'HASHES').iterdir():
    if p.is_file() and p.name.lower().startswith(('usb_content_', 'hdd_destiny_')):
        targets.append((p, 'Xbox capture hash manifest'))
records = []
for p, reason in sorted(targets):
    if not p.resolve().is_relative_to(runtime.resolve()) or p.is_symlink() or p.is_junction():
        raise ValueError(f'Unexpected deletion target: {p}')
    files = []
    if p.is_dir():
        for parent, dirs, names in os.walk(p, followlinks=False):
            for entry in dirs + names:
                child = Path(parent) / entry
                if child.is_symlink() or child.is_junction():
                    raise ValueError(f'Reparse point in retired tree: {child}')
            files.extend(Path(parent) / name for name in names)
    else:
        files = [p]
    records.append({'path': str(p), 'reason': reason, 'files': len(files),
                    'bytes': sum(f.stat().st_size for f in files), 'reparse_points': 0})
report = {'root': str(runtime), 'targets': records, 'files': sum(r['files'] for r in records),
          'bytes': sum(r['bytes'] for r in records)}
(runtime / 'PRIVATE_ENGINEERING/2026-10-06-xenia-cleanup-inventory.json').write_text(json.dumps(report, indent=2))
print(json.dumps({'targets': len(records), 'files': report['files'], 'bytes': report['bytes']}))
