"""Count Dock 13 squads whose last host-logged live count is above 0. alive.py HOSTLOG [GROUP]"""
import re, sys
group = sys.argv[2] if len(sys.argv) > 2 else '0x062dd95d'
s = {}
for l in open(sys.argv[1], encoding='utf-8', errors='replace'):
    m = re.search(r'squad ' + group + r'/0x1/(\d+) live members -?\d+ -> (-?\d+)', re.sub(r'\x1b\[[0-9;]*m', '', l))
    if m: s[m[1]] = int(m[2])
print(sum(1 for v in s.values() if v > 0), ' '.join(f'{k}:{v}' for k, v in sorted(s.items(), key=lambda kv: int(kv[0]))))
