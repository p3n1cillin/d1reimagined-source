#!/bin/bash
# After the replay reports DIVIDE REACHED: gate fights until step 18, garage -> orange room -> Dock 13 corridor,
# then hangar route + fights until step 21 (trigger 41). Logs host milestones. Run: divide-to-bay.sh RUNNAME
S=lab/history/lab-scripts-2026-10-29
H=$(ls -d F:/d1/d1preservation-main/cfw-staging/contained-boot/*-$1)/activity-host.log
step() { sed 's/\x1b\[[0-9;]*m//g' $H | grep -q "mission step $1 at"; }
sed -i "s#^H=.*#H=$H#" $S/breach.sh
gate_clear() { python $S/alive.py $H 0x9243dffb | grep -qE "17:0.*18:0.*19:0"; }
for i in $(seq 1 12); do step 18 && break; gate_clear && break; bash $S/breach.sh 1 2>&1 | grep -E "^cycle" ; done
echo "$(date +%H:%M:%S) gate done (step 18: $(step 18 && echo yes || echo no))"
for i in $(seq 1 6); do
  step 19 && break
  timeout 40 python -u $S/run.py "146,-608;142.8,-606.1" --max-s 20 --tol 1.2 2>&1 | grep -v shadPS4 | tail -1
  (cd $S/pr57 && python -c "
import importlib.util,math
s=importlib.util.spec_from_file_location('w','d1-walk.py');w=importlib.util.module_from_spec(s);s.loader.exec_module(w)
s2=importlib.util.spec_from_file_location('fo',r'$S/follow.py');fo=importlib.util.module_from_spec(s2);s2.loader.exec_module(fo)
c=w.Client(); c.native_unit=True
for _ in range(3):
    d=w.wrap(2.70-w.facing(c,None))
    if abs(d)<0.05: break
    fo.fight.pad('Hold:%s:%d'%('L' if d<0 else 'J', max(12,int(abs(d)/2.76e-3))))
fo.fight.pad('Hold:W:2500')" 2>&1 | tail -1)
  timeout 90 python -u $S/run.py "124.2,-597.9;123.1,-591.3;113.8,-579.6;121.4,-575.6;126,-571.5;132.2,-567.4;135.1,-564" --max-s 80 --tol 1.5 --walk 2>&1 | grep -v shadPS4 | tail -1
  sleep 5
done
step 19 || { echo "NO STEP 19 (Dock 13)"; exit 1; }
echo "$(date +%H:%M:%S) step 19 reached (Dock 13)"
for i in $(seq 1 8); do
  step 21 && break
  timeout 60 python -u $S/run.py '135.1,-564;134.9,-554.3;133.6,-549.5;121.9,-549.2;117,-544.5' --max-s 40 --tol 1.5 2>&1 | grep -v shadPS4 | tail -1
  timeout 60 python -u $S/run.py '115.6,-542.14;114.5,-540.5;111,-536.4;109,-534.4;105.67,-537.77;102.4,-539.3' --max-s 35 --tol 1.2 --walk 2>&1 | grep -v shadPS4 | tail -1
  timeout 70 python -u $S/explore.py 89.8 -536 --max-s 45 2>&1 | grep -v shadPS4 | tail -1
  step 21 && break
  timeout 100 python -u $S/cqb.py --seconds 60 2>&1 | grep -c 'hip fire' | sed 's/^/hip-fire bursts: /'
  timeout 60 python -u $S/run.py '126,-571.5;132.2,-567.4;135.1,-564' --max-s 30 --tol 1.5 2>&1 | grep -v shadPS4 | tail -1
done
step 21 && echo "$(date +%H:%M:%S) step 21 reached (Clear Out the Fallen)" || echo "NO STEP 20"
