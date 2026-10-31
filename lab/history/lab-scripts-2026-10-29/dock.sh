#!/bin/bash
# Dock 13 attrition: sprint from the orange-room respawn to the fight room, close-quarters fight; log progress.
S=lab/history/lab-scripts-2026-10-29
H=$(ls -d F:/d1/d1preservation-main/cfw-staging/contained-boot/*-sense40)/activity-host.log
mkdir -p $S/dk40
for cyc in $(seq 1 ${1:-8}); do
  sleep 3
  echo "cycle $cyc $(date +%H:%M:%S) run: $(timeout 60 python -u $S/run.py '126,-571.5;132.2,-567.4;135.1,-564;134.9,-554.3;133.6,-549.5;121.9,-549.2;117,-543' --max-s 40 --tol 1.5 2>&1 | grep -v shadPS4 | tail -1)"
  mkdir -p $S/dk40/$cyc
  echo "  hip-fire bursts: $(timeout 130 python -u $S/cqb.py --seconds 90 --shots $S/dk40/$cyc 2>&1 | grep -c 'hip fire')"
  echo "  squads: $(python $S/alive.py $H)"
  sed 's/\x1b\[[0-9;]*m//g' $H | grep -E "mission step 2[0-9]|0x1d/41" | cut -c12-27,60-140 | tail -2 | sed 's/^/  host: /'
  sed 's/\x1b\[[0-9;]*m//g' $H | grep -q "mission step 20" && { echo STEP20; break; }
done
