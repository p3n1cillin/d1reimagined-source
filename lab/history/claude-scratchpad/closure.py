"""Transitive set of files the Tower route (ending-run.ps1) reaches: lab scripts named by file name,
Python imports of sibling modules, and absolute paths (with $C/$S/$Q-style folder variables)."""
import os
import re
import sys

S = 'lab/history/lab-scripts-2026-10-29'
VARS = {
    '$C': 'F:/d1/d1preservation-main/cfw-staging',
    '$S': S,
    '$Q': 'lab/history/2026-10-29-postgame-codex',
}
names = set(os.listdir(S))
seen, todo = {}, [f'{S}/ending-run.ps1']
abs_pat = re.compile(r'(?:[A-Za-z]:|/[a-z])[\\/][^\s"\'`|,;)]+')
var_pat = re.compile(r'(\$[CSQ])/([A-Za-z0-9_.\-/]+)')
while todo:
    f = todo.pop().replace('\\', '/')
    if re.match(r'^/[a-z]/', f):
        f = f[1].upper() + ':' + f[2:]
    if f in seen:
        continue
    if not os.path.isfile(f):
        seen[f] = 'MISSING/DIR' if not os.path.isdir(f) else 'DIR'
        continue
    seen[f] = os.path.getsize(f)
    if not f.endswith(('.ps1', '.sh', '.py', '.txt')) or os.path.getsize(f) > 200000:
        continue
    text = open(f, encoding='utf-8', errors='replace').read()
    for m in abs_pat.findall(text):
        todo.append(m.rstrip('.'))
    for var, rest in var_pat.findall(text):
        todo.append(f'{VARS[var]}/{rest}')
    for n in names:
        if re.search(r'(?<![\w.-])' + re.escape(n) + r'(?![\w.-])', text):
            todo.append(f'{S}/{n}')
    for mod in re.findall(r"(?:^|\n)\s*(?:import|from)\s+(\w+)", text):
        if f'{mod}.py' in names:
            todo.append(f'{S}/{mod}.py')
    for mod in re.findall(r"spec_from_file_location\('\w+',\s*[^)]*?'(\w+\.py)'", text):
        todo.append(f'{os.path.dirname(f)}/{mod}')
for k, v in sorted(seen.items()):
    print(v, k)
