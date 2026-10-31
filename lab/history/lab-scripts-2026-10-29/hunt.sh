#!/bin/bash
# Hunt the live members of a Dock 13 cohort until the host gate reports it cleared. Usage: hunt.sh RUNNAME [ROUNDS]
S=lab/history/lab-scripts-2026-10-29
H=$(ls -d F:/d1/d1preservation-main/cfw-staging/contained-boot/*-$1)/activity-host.log
C=5,6,7,8,9,10,11,12
rel() { python $S/release.py; }                # every tool ends with no key held (a kill mid-press leaves it down)
for i in $(seq 1 ${2:-12}); do
  sed 's/\x1b\[[0-9;]*m//g' $H | grep -q "is exactly zero" && break
  echo "$(date +%H:%M:%S) squads: $(python $S/alive.py $H)"
  t=$(python $S/target.py $C 2>/dev/null | grep -v shadPS4 | tail -1)
  [ -z "$t" ] && { echo "  no live member visible"; break; }
  echo "  seek $t: $(timeout 200 python -u $S/seek.py $t --max-s 90 --tol 6 --trail $S/seek-trails.jsonl 2>&1 | grep -v shadPS4 | tail -1)"; rel
  timeout 120 python -u $S/combat2.py --seconds 50 --squads $C --log $S/hunt-combat.jsonl 2>&1 | grep -E "vit|bursts|stopped" | tail -2 | sed 's/^/    /'; rel
done
echo "$(date +%H:%M:%S) end: $(python $S/alive.py $H)"
sed 's/\x1b\[[0-9;]*m//g' $H | grep -E "is exactly zero|mission step 2[0-9]|slice set 10[45]" | tail -4 | cut -c12-27,60-220
