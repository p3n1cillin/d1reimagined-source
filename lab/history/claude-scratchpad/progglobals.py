"""Read-only: find the investment globals object behind 0x25a550()->vtable+0x68 and its progression ids."""
import importlib.util, os, struct, sys
HERE = os.path.dirname(os.path.abspath(__file__))
s = importlib.util.spec_from_file_location('g', os.path.join(HERE, 'pr57', 'd1-winguest.py')); g = importlib.util.module_from_spec(s); s.loader.exec_module(g)
sys.path.insert(0, r'lab/history/2026-10-28-static-combat-codex')
import static56578 as st
p, base = g.open_client()
obj = p.u(base + 0x26527e0, 'Q'); vt = p.u(obj, 'Q'); f = p.u(vt + 0x68, 'Q')
print(f'base {base:#x} obj {obj:#x} vtable {vt:#x} (static {vt-base:#x}) fn+0x68 {f:#x} (static {f-base:#x})')
print(st.dis(f - base, 0x60))
