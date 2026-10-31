#!/bin/bash
# In the Tower as activity 165: follow the green objective waypoint while recording, and collect every
# trigger and incident the client reports to the activity host on the way. If 165 is the scripted first
# Tower visit, its first objective's volume should report like A Guardian Rises' triggers did.
# Usage: intro-probe.sh RUNNAME [STEPS]
S=lab/history/lab-scripts-2026-10-29
R=F:/d1/d1preservation-main/cfw-staging/$1; K=$R/intro; mkdir -p $K
H=$(ls -d F:/d1/d1preservation-main/cfw-staging/contained-boot/*-$1)/activity-host.log
export D1_ROOT=F:/d1 D1_LAB_DRIVER=1
touch $S/codex.lock; sleep 65; python $S/release.py   # the keep-alive finishes its minute, then pauses
start=$(date -u +%Y-%m-%dT%H:%M:%S)
(cd F:/d1/public-work/d1rs-claude && python -I tools/ps4_setup/d1-record.py --out $K/intro-walk.mp4 --seconds 330 > $K/record.log 2>&1 &)
sleep 5
echo "$(date +%H:%M:%S) following the waypoint"
timeout 320 python -u $S/waypoint.py --steps ${2:-45} --walk-ms 1600 --shots $K 2>&1 | tail -12
python $S/release.py
sleep 10
rm -f $S/codex.lock
echo "$(date +%H:%M:%S) host reports since $start:"
sed 's/\x1b\[[0-9;]*m//g' $H | awk -v s="$start" '$1 >= s' | grep -E "player trigger|incident|mission step|objective|peer_leave" | cut -c12-190 | head -40
