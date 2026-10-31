p = '%USERPROFILE%/AppData/Local/Temp/claude/F--d1-d1xenia/eb49bdd6-2562-42a0-a73d-ed14111dbab8/scratchpad/edit_py.py'
raw = open(p, 'rb').read().decode('utf-8')
start = raw.index('def load(f):')
end = raw.index('def sub1(')
BS = chr(92)
hdr = ("def load(f):\n"
       "    t = (D / f).read_bytes().decode('utf-8')\n"
       f"    return t.replace('{BS}r{BS}n', '{BS}n'), '{BS}r{BS}n' in t\n\n\n"
       "def save(f, t, crlf):\n"
       f"    (D / f).write_bytes((t.replace('{BS}n', '{BS}r{BS}n') if crlf else t).encode('utf-8'))\n\n\n")
body = raw[:start] + hdr + raw[end:]
body = body.replace('\r\n', '\n')
open(p, 'wb').write(body.encode('utf-8'))
print(body[:700])
