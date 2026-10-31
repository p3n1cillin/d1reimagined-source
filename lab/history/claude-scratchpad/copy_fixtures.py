"""Copy the 56578 mission fixtures into the public repo (ps4/fixtures/56578) and rewrite the absolute
paths inside the mission lists to names in the same folder."""
import os
import re
import shutil

B = 'F:/d1/d1preservation-main/capture-rig/builds/later-56578'
C = 'F:/d1/d1preservation-main/cfw-staging'
OUT = 'F:/d1/public-work/wt-ci/ps4/fixtures/56578'
RENAME = {'world-table-sense58.txt': 'world-table-tower.txt'}

characters = ['profile-tutorial-t.bin', 'profile-geared-385-director-c.bin', 'activity-table.tsv',
              'inventory', 'inventory-tutorial', 'rosters-f0-geared', 'rosters-f0-tutorial', 'grants/khvostov-3598.bin',
              'grants/profile-tutorial-th.bin', 'grants/rosters-f0-tutorial-khv']
postgame = ['profile-active164-xp0.draft.bin', 'profile-active164-xp900.draft.bin',
            'arcadia-profile-xp900-owned.draft.bin', 'arcadia-3517-inventory.draft.bin',
            'agr-postgame-scoreboard-arcadia-ratio30.draft.txt']


def copy(src, dst):
    os.makedirs(os.path.dirname(dst), exist_ok=True)
    if os.path.isdir(src):
        shutil.copytree(src, dst, dirs_exist_ok=True)
    else:
        shutil.copy2(src, dst)


for name in characters:
    copy(f'{B}/{name}', f'{OUT}/characters/{name}')
for name in postgame:
    copy(f'{C}/{name}', f'{OUT}/postgame/{name}')

# The mission lists and every file they reach.
roots = [f'{C}/agr-steps-divide.txt', f'{C}/world-table-sense58.txt', f'{B}/world-table.txt',
         f'{C}/sense35/breach-prop3-only.txt']
seen, todo = set(), list(roots)
pat = re.compile(r'[A-Za-z]:[\\/][^\s|,]+')
while todo:
    src = todo.pop().replace('\\', '/')
    if src in seen:
        continue
    seen.add(src)
    text = open(src, encoding='utf-8').read()
    todo += pat.findall(text)
    text = pat.sub(lambda m: RENAME.get(os.path.basename(m.group().replace('\\', '/')),
                                        os.path.basename(m.group().replace('\\', '/'))), text)
    base = os.path.basename(src)
    dst = f'{OUT}/mission/{RENAME.get(base, base)}'
    os.makedirs(os.path.dirname(dst), exist_ok=True)
    with open(dst, 'w', encoding='utf-8', newline='\n') as f:
        f.write(text)
print(len(seen), 'mission files')
