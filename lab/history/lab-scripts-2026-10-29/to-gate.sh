#!/bin/bash
# Walk the Divide crossing route from the nearest point to just outside the gate (native unit position).
S=lab/history/lab-scripts-2026-10-29
cd F:/d1/public-work/d1reimagined-sense
python -I -c "
import importlib.util,json,math
s=importlib.util.spec_from_file_location('w','tools/ps4_setup/d1-walk.py');w=importlib.util.module_from_spec(s);s.loader.exec_module(w)
c=w.Client(); c.native_unit=True; here=w.position(c,None)
r=json.load(open('missions/routes/divide_crossing.json'))
i=min(range(len(r)),key=lambda k:math.dist(r[k][:2],here[:2]))
json.dump(r[i:-${1:-2}],open(r'$S/route-gate.json','w')); print('from',here,'wp',i)
" 2>&1 | tail -1
timeout 200 python -u -I tools/ps4_setup/d1-walk.py --native-unit --memory-facing --route $S/route-gate.json --turn-sign 1 --turn-rate 2.26 --max-seconds 150 2>&1 | tail -2
