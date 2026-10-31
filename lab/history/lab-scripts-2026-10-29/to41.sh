#!/bin/bash
S=lab/history/lab-scripts-2026-10-29
H=$(ls -d F:/d1/d1preservation-main/cfw-staging/contained-boot/*-$1)/activity-host.log
done41() { sed 's/\x1b\[[0-9;]*m//g' $H | grep -q "player trigger 0x992bb471/0x1d/41"; }
for i in $(seq 1 8); do
  done41 && break
  echo "try $i $(date +%H:%M:%S)"
  timeout 60 python -u $S/run.py '126,-571.5;132.2,-567.4;135.1,-564;134.9,-554.3;133.6,-549.5;121.9,-549.2;117,-544.5' --max-s 40 --tol 1.5 --walk 2>&1 | grep -v shadPS4 | tail -1
  timeout 70 python -u $S/cqb.py --seconds 30 2>&1 | grep -c 'hip fire' | sed 's/^/  bursts: /'
  done41 && break
  timeout 60 python -u $S/run.py '115.6,-542.14;114.5,-540.5;111,-536.4;109,-534.4;105.67,-537.77;102.4,-539.3' --max-s 40 --tol 1.2 --walk 2>&1 | grep -v shadPS4 | tail -1
  done41 && break
  timeout 70 python -u $S/explore.py 89.8 -536 --max-s 45 2>&1 | grep -v shadPS4 | tail -1
done
done41 && echo "TRIGGER 41 $(date +%H:%M:%S)" || echo "NO 41"
