#!/bin/bash
# Repeat: sprint from the respawn to the firing spot, fight at the truck until death; log squad drops and slice arrivals.
S=lab/history/lab-scripts-2026-10-29
H=F:/d1/d1preservation-main/cfw-staging/contained-boot/20261028-083554-sense36/activity-host.log
SPOT=${SPOT:-178.5,-584.5}
for cyc in $(seq 1 ${1:-8}); do
  sleep 3
  cd $S/pr57
  P=$(python -c "
import json,math,importlib.util
s=importlib.util.spec_from_file_location('w','d1-walk.py');w=importlib.util.module_from_spec(s);s.loader.exec_module(w)
c=w.Client(); c.native_unit=True; here=w.position(c,None)[:2]
r=json.load(open('divide_crossing.json'))
i=min(range(len(r)),key=lambda k:math.dist(r[k][:2],here))
sx=float('$SPOT'.split(',')[0])
sub=[p for p in r[i+1::2] if p[0]>sx]+[[float(v) for v in '$SPOT'.split(',')]]
print(';'.join(f'{p[0]},{p[1]}' for p in sub))" 2>/dev/null | grep -v shadPS4 | tail -1)
  echo "cycle $cyc $(date +%H:%M:%S) dash: $(timeout 120 python -u $S/dash.py "$P" 2>&1 | tail -1)"
  mkdir -p $S/att/$cyc
  timeout 200 python -u $S/pointfight.py 142 -610 --seconds 150 --shots $S/att/$cyc 2>&1 | grep -v "^shadPS4" | tail -3 | sed "s/^/  /"
  sed 's/\x1b\[[0-9;]*m//g' $H | grep -E "squad 0x.*-> 0|now in slice set 104|mission step 1[7-9]" | cut -c12-27,60-200 | tail -3 | sed "s/^/  host: /"
  if sed 's/\x1b\[[0-9;]*m//g' $H | grep -q "now in slice set 104"; then echo ARRIVED-104; break; fi
done
