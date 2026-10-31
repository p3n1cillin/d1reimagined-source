#!/bin/bash
# cycle.sh RUN_DIR MINUTES - repeat: retrack, explorer legs still ahead (by x), then the combat follower from drop2fall.
R=$1; END=$(( $(date +%s) + ${2:-25} * 60 ))
S=lab/history/lab-scripts-2026-10-29
T=/f/d1/public-work/d1reimagined-source/tools/ps4_setup
c=0
while [ $(date +%s) -lt $END ]; do
  c=$((c + 1))
  line=$(cd /f/d1/public-work/d1rs-claude/tools/ps4_setup && timeout 400 python -I -u d1-walk.py --tracked $R/tracked.json --retrack --respawns "319,-664,7;277,-625,6;189,-606,5;225,-603,4;197,-575,5" 2>&1 | grep "tracked; position" | tail -1)
  x=$(echo "$line" | sed -E 's/.*position \(([-0-9.]+),.*/\1/' | cut -d. -f1)
  echo "cycle $c: $line"
  for leg in fa1:262,-640 fa:243,-619 drop1left:225,-600 drop2:195,-573 drop2fall:170,-591; do
    n=${leg%%:*}; g=${leg#*:}; gx=${g%%,*}
    [ -n "$x" ] && [ "$x" -le $((gx + 3)) ] && continue
    (cd $T && timeout 330 python -I -u d1-explore.py --tracked $R/tracked.json --goal $g --reach 4 --out $R/route-$n-cyc$c.json --max-seconds 300 --stop-on-death --lethal $S/lethal.json > $R/explore-$n-cyc$c.log 2>&1)
    echo "  $n: $(tail -1 $R/explore-$n-cyc$c.log | cut -c1-90)"
    tail -1 $R/explore-$n-cyc$c.log | grep -q '^reached' || continue 2
  done
  mkdir -p $R/cycle$c
  python -I -u $S/follow.py --seconds 150 --shots $R/cycle$c --tracked $R/tracked.json 2>&1 | grep -v "^shadPS4" > $R/cycle$c.log
  echo "  fight: $(grep -c 'enemy plate' $R/cycle$c.log) engagements; last: $(tail -1 $R/cycle$c.log | cut -c1-100)"
done
