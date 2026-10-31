#!/bin/bash
# chain.sh RUN_DIR [LAST_GOAL_NAME]  - the run-20 firing-area chain across the Divide, explorer leg by leg.
R=$1; STOP=${2:-gate}
S=%USERPROFILE%/AppData/Local/Temp/claude/F--d1-d1xenia/eb49bdd6-2562-42a0-a73d-ed14111dbab8/scratchpad
cd /f/d1/public-work/d1reimagined-source/tools/ps4_setup
for leg in fa1:262,-640 fa:243,-619 drop1left:225,-600 drop2:195,-573 drop2fall:170,-591 exit:153,-606 gate:143,-609; do
  n=${leg%%:*}; g=${leg#*:}
  timeout 330 python -I -u d1-explore.py --tracked $R/tracked.json --goal $g --reach 4 --out $R/route-$n.json --max-seconds 300 --stop-on-death --lethal $S/lethal.json > $R/explore-$n.log 2>&1
  echo "$n: $(tail -1 $R/explore-$n.log | cut -c1-110)"
  tail -1 $R/explore-$n.log | grep -q '^reached' || exit 1
  [ "$n" = "$STOP" ] && exit 0
done
