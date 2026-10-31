#!/bin/bash
# Repeat: sprint from wherever the Guardian is into the garage room, then close-quarters fight; log host progress.
S=lab/history/lab-scripts-2026-10-29
H=F:/d1/d1preservation-main/cfw-staging/contained-boot/20261028-155923-sense42/activity-host.log
for cyc in $(seq 1 ${1:-6}); do
  sleep 3
  cd $S/pr57
  P=$(python -c "
import json,math,importlib.util
s=importlib.util.spec_from_file_location('w','d1-walk.py');w=importlib.util.module_from_spec(s);s.loader.exec_module(w)
c=w.Client(); c.native_unit=True; here=w.position(c,None)[:2]
r=json.load(open('divide_crossing.json'))
i=min(range(len(r)),key=lambda k:math.dist(r[k][:2],here))
sub=(r[i+1::2]+[r[-1]]) if math.dist(here,r[-1][:2])>6 else [r[-1]]
print(';'.join(f'{p[0]},{p[1]}' for p in sub))" 2>/dev/null | grep -v shadPS4 | tail -1)
  echo "cycle $cyc $(date +%H:%M:%S) run: $(timeout 90 python -u $S/run.py "$P" 2>&1 | grep -v shadPS4 | tail -1)"
  mkdir -p $S/br40/$cyc
  timeout 200 python -u $S/cqb.py --seconds 150 --shots $S/br40/$cyc 2>&1 | grep -v "^shadPS4" | grep -c "hip fire" | sed 's/^/  hip-fire bursts: /'
  sed 's/\x1b\[[0-9;]*m//g' $H | grep -E "squad 0x.*-> 0|now in slice set 104|mission step 1[7-9]" | grep -v "T09:14:50" | cut -c12-27,60-200 | tail -3 | sed "s/^/  host: /"
  if sed 's/\x1b\[[0-9;]*m//g' $H | grep -q "now in slice set 104"; then echo ARRIVED-104; break; fi
done
