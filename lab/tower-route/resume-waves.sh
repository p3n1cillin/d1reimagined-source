#!/bin/bash
# Supervised resume of an -Waves run after the rifle, then the Divide chain if it gets there.
#
#   bash lab/tower-route/resume-waves.sh RUNNAME [SCHEDULE_DIR]
#
# Stops this run's replay and walker first (exact PIDs, listed before they are stopped) and releases
# the pad, then runs d1-agr-replay.py --resume-after-rifle --waves, which starts at the leg nearest the
# Guardian. SCHEDULE_DIR is the folder of the served wave lists (default: this repository's fixtures).
# Logs: <staging>/RUNNAME/replay-console-resumeN.log and chain.log.
set -u
S=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd); REPO=$(cd "$S/../.." && pwd)
C=$(python "$REPO/tools/ps4_setup/d1paths.py" staging); F=$(python "$REPO/tools/ps4_setup/d1paths.py" fixtures)
R=$1; DIR=${2:-$F/mission}
D=$(ls -d $C/contained-boot/*-$R 2>/dev/null) || { echo "no contained run $R"; exit 2; }
for pid in $(powershell -NoProfile -Command "Get-CimInstance Win32_Process -Filter \"Name='python.exe'\" | Where-Object { \$_.CommandLine -match 'd1-agr-replay.*$R|d1-walk' } | ForEach-Object { \$_.ProcessId }"); do
    pid=$(echo $pid | tr -d '\r'); echo "stopping $pid"; powershell -NoProfile -Command "Stop-Process -Id $pid -ErrorAction SilentlyContinue"
done
python -I $S/release.py >/dev/null 2>&1; rm -f $C/$R/replay.lock
n=1; while [ -e $C/$R/replay-console-resume$n.log ]; do n=$((n + 1)); done
OUT=$C/$R/replay-console-resume$n.log; ERR=$C/$R/replay-error-resume$n.log
echo "resume $n -> $OUT"
cd $REPO && python -u -I tools/ps4_setup/d1-agr-replay.py --run-dir "$(cd $C/$R && pwd -W)" \
    --host-log "$(cd $D && pwd -W)/activity-host.log" --garage-clients "$(cd $C/$R && pwd -W)/breach-prop3-only.txt" \
    --resume-after-rifle --waves --schedule-dir "$(cd $DIR && pwd -W)" > $OUT 2> $ERR
tail -2 $OUT; tail -2 $ERR
grep -q 'DIVIDE REACHED' $OUT && bash $S/chain2.sh $R > $C/$R/chain.log 2>&1 && tail -4 $C/$R/chain.log
