#!/bin/bash
# SPDX-License-Identifier: GPL-2.0-or-later
# A Guardian Rises under the script bridge (full-game profile), by pad input only: wait for the
# Guardian's spawn, walk into the Wall, then (once the scripts' phase 2 begins) the Breach, the
# platform (the Ghost's light-up, trigger 120) and the rifle (trigger 35, then phase 3).
#
#   bash lab/tower-route/agr-scripts-drive.sh NAME
#
# NAME is the full-game run's name (tools/ps4_setup/d1-full-game.ps1 -Name NAME); the script finds
# its newest run folder under STAGING/full-game and writes walk-*.log there. It also accepts the
# step schedule's markers (mission step 5), so a full-game run with the bridge off can be driven
# the same way. Walks use d1-walk.py --native-unit (missions/routes). A "unit unavailable" error on
# the first walk is a start-up race: run the script again (it resumes from the spawn).
# Runs agr4-agr20 and stepfg1-2 (6 Nov, notes/ps4-56578-full-game-test-2026-11-06.md).
NAME=${1:?run name}
HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
FG=${D1_STAGING:-/f/d1/d1preservation-main/cfw-staging}/full-game
until R=$(ls -d $FG/*-$NAME 2>/dev/null | tail -1) && [ -n "$R" ] && [ -f $R/boot/activity-host.log ]; do sleep 3; done
H=$R/boot/activity-host.log; RT=missions/routes; cd "$HERE"; echo "run $R"
walk() { local name=$1 route=$2 secs=$3 tol=$4; shift 4
  python -u -I tools/ps4_setup/d1-walk.py --native-unit --memory-facing --route $route --max-seconds $secs \
    --tolerance $tol --turn-sign 1 --turn-rate 2.26 "$@" > $R/walk-$name.log 2>&1
  echo "$(date -u +%T) $name exit $? $(tail -1 $R/walk-$name.log)"; }
waitlog() { local pat=$1 secs=$2; local end=$(( $(date +%s) + secs )); until grep -q "$pat" $H; do [ $(date +%s) -gt $end ] && return 1; sleep 2; done; }
waitlog "incident 2014 reported" 1500 || { echo "no spawn"; exit 1; }
echo "spawned $(date -u +%T)"; sleep 25
walk enter-wall $RT/enter_the_wall.json 650 2.5
# Runs agr29 and agr32 (7 Nov): the first walk right after spawn can fail before the Guardian's unit
# is readable ("Current Guardian unit is unavailable"); one more try after a pause.
if ! waitlog "mission phase 2 \|mission step 5 at" 20 && grep -q "unit is unavailable" $R/walk-enter-wall.log; then
  sleep 10; walk enter-wall-2 $RT/enter_the_wall.json 650 2.5; fi
waitlog "mission phase 2 \|mission step 5 at" 60 && echo "phase 2 $(date -u +%T)" || { echo "NO phase 2"; exit 1; }
walk breach $RT/breach.json 640 1.3 --avoid 330,-837,340,-812
walk platform $RT/breach_platform.json 200 1.3
if waitlog "0xfc0e45a3/0x1d/120 reported" 15; then echo "trigger 120 (light-up) reported"; else
  python -c "import json,sys; json.dump(list(reversed(json.load(open(sys.argv[1])))), open(sys.argv[2],'w'))" $RT/breach_platform.json $R/platform-back.json
  walk platform-back $R/platform-back.json 200 1.3; walk platform-again $RT/breach_platform.json 200 1.3
  waitlog "0xfc0e45a3/0x1d/120 reported" 15 && echo "trigger 120 reported (second pass)" || echo "NO trigger 120"; fi
sleep 30   # the light-up: the Ghost's performance, the control (15 s), the gating lights (23 s)
for a in 0 1; do walk rifle-$a $RT/breach_rifle.json 150 1.0; waitlog "0x992bb471/0x1d/35 reported\|mission phase 3 " 20 && { echo "rifle reached"; break; }; done
waitlog "mission phase 3 " 60 && echo "phase 3 $(date -u +%T)" || echo "no phase 3 yet"
echo "end $(date -u +%T)"
