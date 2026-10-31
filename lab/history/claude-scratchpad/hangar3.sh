#!/bin/bash
# nav -> bay, combat2 until death or 150 s; repeat. Logs Dock 13 squad counts.
S=%USERPROFILE%/AppData/Local/Temp/claude/F--d1-d1xenia/eb49bdd6-2562-42a0-a73d-ed14111dbab8/scratchpad
H=$(ls -d F:/d1/d1preservation-main/cfw-staging/contained-boot/*-$1)/activity-host.log
for i in $(seq 1 ${2:-12}); do
  echo "round $i $(date +%H:%M:%S) $(timeout 220 python -u $S/nav.py bay --max-s 180 2>&1 | grep -v shadPS4 | tail -1)"
  timeout 200 python -u $S/combat2.py --seconds 150 --log $S/c3.jsonl 2>&1 | grep -v shadPS4 | grep -E "lock|bursts|stopped" | tail -2 | sed 's/^/  /'
  a=$(python $S/alive.py $H); echo "  squads: $a"
  [ "${a%% *}" = "0" ] && { echo CLEARED; break; }
done
