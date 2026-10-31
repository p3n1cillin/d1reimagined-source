"""List every file reachable from the mission lists through absolute paths written in them."""
import os
import re
import sys

roots = sys.argv[1:]
seen = {}
todo = list(roots)
while todo:
    f = todo.pop().replace('\\', '/')
    if f in seen:
        continue
    if not os.path.exists(f):
        seen[f] = 'MISSING'
        continue
    text = open(f, encoding='utf-8', errors='replace').read()
    seen[f] = len(text)
    for m in re.findall(r'[A-Za-z]:[\\/][^\s|,]+', text):
        todo.append(m)
for k, v in sorted(seen.items()):
    print(v, k)
