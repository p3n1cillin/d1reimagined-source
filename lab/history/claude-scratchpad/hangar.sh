#!/bin/bash
# Alternate close-quarters fighting and marker following in Dock 13; log squad drops.
S=%USERPROFILE%/AppData/Local/Temp/claude/F--d1-d1xenia/eb49bdd6-2562-42a0-a73d-ed14111dbab8/scratchpad
H=F:/d1/d1preservation-main/cfw-staging/contained-boot/20261028-105842-sense37/activity-host.log
for cyc in $(seq 1 ${1:-10}); do
  mkdir -p $S/hg/$cyc
  echo "cycle $cyc $(date +%H:%M:%S) cqb hip-fire bursts: $(timeout 100 python -u $S/cqb.py --seconds 60 --shots $S/hg/$cyc 2>&1 | grep -c 'hip fire')"
  echo "  follow: $(timeout 60 python -u $S/follow.py --seconds 20 2>&1 | tail -1)"
  sed 's/\x1b\[[0-9;]*m//g' $H | grep -E "squad 0x062.*-> 0" | cut -c12-27,60-200 | sed 's/^/  host: /'
  alive=$(python $S/alive.py $H | cut -d" " -f1); echo "  counts: $(python $S/alive.py $H)"
  echo "  dock13 squads still alive: $alive"
  [ "$alive" = "0" ] && { echo CLEARED; break; }
done
