#!/bin/bash
# Respawn -> NUX door -> trigger-41 room -> fight; repeat. Logs Dock 13 squad counts.
S=lab/history/lab-scripts-2026-10-29
H=$(ls -d F:/d1/d1preservation-main/cfw-staging/contained-boot/*-sense40)/activity-host.log
mkdir -p $S/hg2
for cyc in $(seq 1 ${1:-8}); do
  sleep 3
  echo "cycle $cyc $(date +%H:%M:%S) run1: $(timeout 60 python -u $S/run.py '126,-571.5;132.2,-567.4;135.1,-564;134.9,-554.3;133.6,-549.5;121.9,-549.2;117,-544.5' --max-s 40 --tol 1.5 2>&1 | grep -v shadPS4 | tail -1)"
  echo "  run2: $(timeout 60 python -u $S/run.py '115.6,-542.14;114.5,-540.5;111,-536.4;109,-534.4;105.67,-537.77;102.8,-540.7;99.7,-544' --max-s 40 --tol 1.2 --walk 2>&1 | grep -v shadPS4 | tail -1)"
  mkdir -p $S/hg2/$cyc
  echo "  hip-fire bursts: $(timeout 130 python -u $S/cqb.py --seconds 90 --shots $S/hg2/$cyc 2>&1 | grep -c 'hip fire')"
  a=$(python $S/alive.py $H); echo "  squads: $a"
  [ "${a%% *}" = "0" ] && { echo CLEARED; break; }
done
