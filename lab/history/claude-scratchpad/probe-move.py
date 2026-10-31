import importlib.util, subprocess, sys
L='lab/history/lab-scripts-2026-10-29'
s=importlib.util.spec_from_file_location('c',L+'/combat2.py');c=importlib.util.module_from_spec(s);s.loader.exec_module(c)
W=c.World()
def me():
    for _ in range(5):
        st=W.me()
        if st: return st
def pad(k): subprocess.run(['powershell','-NoProfile','-File',r'F:\d1\tools\d1-pad.ps1','-Keys',k],capture_output=True)
for k in sys.argv[1:]:
    a=me(); pad(k); b=me()
    print(k, tuple(round(v,1) for v in a[0]), round(a[1],2), '->', tuple(round(v,1) for v in b[0]), round(b[1],2))
