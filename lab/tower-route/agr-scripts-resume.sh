#!/bin/bash
# SPDX-License-Identifier: GPL-2.0-or-later
# A Guardian Rises under the script bridge, after agr-scripts-drive.sh reached phase 3 (the rifle in
# hand): scan the Guardian's position at the rifle, then run d1-agr-replay.py --resume-after-rifle
# --waves through the Wall (it fights the squads the scripts spawn; with no --schedule-dir it
# publishes nothing) to the Divide, then chain2.sh (the gate, Dock 13, the hangar).
#
#   bash lab/tower-route/agr-scripts-resume.sh NAME
#
# The replay and chain2 read the host log by a run name: this makes junctions to the full-game run's
# boot folder (STAGING/full-game/<utc>-NAME/hostlog-NAME-replay and STAGING/contained-boot/<utc>-NAME)
# and writes their logs to STAGING/NAME-replay and STAGING/NAME. Run agr24 (7 Nov): the Divide with no
# death, then Dock 13 and the hangar (phase 5).
NAME=${1:?run name}
HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
ST=${D1_STAGING:-/f/d1/d1preservation-main/cfw-staging}; FG=$ST/full-game
R=$(ls -d $FG/*-$NAME | tail -1); H=$R/boot/activity-host.log
until grep -q "mission phase 3 " $H; do sleep 3; done; sleep 5
junction() { [ -e "$1" ] || pwsh -NoProfile -Command "New-Item -ItemType Junction -Path '$(cygpath -w $1)' -Target '$(cygpath -w $2)' | Out-Null"; }
RD=$ST/$NAME-replay; mkdir -p $RD $ST/$NAME; cp $HERE/ps4/fixtures/56578/mission/breach-prop3-only.txt $RD/
J=$R/hostlog-$NAME-replay; junction $J $R/boot; junction $ST/contained-boot/$(basename $R) $R/boot
[ -f $J/activity-host.log ] || { echo "no junction"; exit 1; }
cd "$HERE"
python -u -I tools/ps4_setup/d1-walk.py --scan 410.6,-794.2,19.0 --radius 25 --out $RD/tracked.json > $RD/scan.log 2>&1
echo "scan: $(tail -1 $RD/scan.log)"
python -u -I tools/ps4_setup/d1-agr-replay.py --run-dir $RD --host-log $J/activity-host.log \
  --garage-clients $RD/breach-prop3-only.txt --resume-after-rifle --waves > $RD/replay-console.log 2> $RD/replay-error.log
echo "replay exit $?: $(tail -1 $RD/replay-console.log)"
grep -q "DIVIDE REACHED" $RD/replay-console.log || exit 1
bash lab/tower-route/chain2.sh $NAME > $ST/$NAME/chain.log 2>&1; echo "chain2: $(tail -1 $ST/$NAME/chain.log)"
