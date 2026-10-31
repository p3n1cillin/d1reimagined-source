"""Sweep objective 17's marker index; record where Nav Mode draws the waypoint diamond for each."""
import importlib.util, json, os, subprocess, sys, time
import numpy as np
from PIL import Image
HERE = r'F:\d1\public-work\d1reimagined-source\tools\ps4_setup'
spec = importlib.util.spec_from_file_location('nf', os.path.join(HERE, 'd1-navfollow.py'))
nf = importlib.util.module_from_spec(spec); spec.loader.exec_module(nf)
RUN = sys.argv[3]
CTRL = r'F:\d1\d1preservation-main\cfw-staging\ah-control.txt'
S = os.path.dirname(os.path.abspath(__file__))
tpl = np.asarray(Image.open(os.path.join(S, 'diamond17.png')).convert('L'), dtype=np.float32)
def pad(k): subprocess.run(['powershell', '-NoProfile', '-File', r'F:\d1\tools\d1-pad.ps1', '-Keys', k], capture_output=True)
out = open(os.path.join(S, 'sweep17.jsonl'), 'a')
for n in range(int(sys.argv[1]), int(sys.argv[2])):
    subprocess.run([sys.executable, '-I', os.path.join(RUN, 'make-obj17.py'), str(n)], capture_output=True)
    with open(CTRL, 'x') as f:
        f.write(f'clients 0x4786c0e0 {RUN}/obj17-{n}.txt'.replace(chr(92), '/') + chr(10))
    t = time.time()
    while os.path.exists(CTRL) and time.time() - t < 30: time.sleep(.5)
    time.sleep(3)
    pad('Hold:I:80'); pad('Hold:K:80')
    pad('Space'); time.sleep(1.8)
    img = nf.grab(os.path.join(S, 'sweep-shot.png'))
    pad('Space'); time.sleep(.6)
    os.makedirs(os.path.join(S, 'sweep17'), exist_ok=True)
    Image.open(os.path.join(S, 'sweep-shot.png')).convert('RGB').resize((640, 370)).save(os.path.join(S, 'sweep17', f'{n:03d}.png'))
    hit = nf.find(img, tpl, 0.6)
    rec = {'n': n, 'hit': [round(v, 2) for v in hit] if hit else None}
    out.write(json.dumps(rec) + '\n'); out.flush(); print(rec, flush=True)
