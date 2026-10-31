#!/bin/bash
# Respawn (orange room) -> fight room -> NUX door -> NW gap -> hangar bay -> fight. Repeat.
S=lab/history/lab-scripts-2026-10-29
H=$(ls -d F:/d1/d1preservation-main/cfw-staging/contained-boot/*-sense42)/activity-host.log
mkdir -p $S/bay
for cyc in $(seq 1 ${1:-8}); do
  sleep 3
  echo "cycle $cyc $(date +%H:%M:%S) leg1: $(timeout 60 python -u $S/run.py '126,-571.5;132.2,-567.4;135.1,-564;134.9,-554.3;133.6,-549.5;121.9,-549.2;117,-544.5' --max-s 40 --tol 1.5 2>&1 | grep -v shadPS4 | tail -1)"
  echo "  leg2: $(timeout 60 python -u $S/run.py '115.6,-542.14;114.5,-540.5;111,-536.4;109,-534.4;105.67,-537.77;102.4,-539.3' --max-s 35 --tol 1.2 --walk 2>&1 | grep -v shadPS4 | tail -1)"
  echo "  leg3: $(timeout 70 python -u $S/explore.py 89.8 -536 --max-s 45 2>&1 | grep -v shadPS4 | tail -1)"
  mkdir -p $S/bay/$cyc
  echo "  hip-fire bursts: $(timeout 150 python -u $S/cqb.py --seconds 110 --shots $S/bay/$cyc 2>&1 | grep -c 'hip fire')"
  a=$(python $S/alive.py $H); echo "  squads: $a"
  [ "${a%% *}" = "0" ] && { echo CLEARED; break; }
done
