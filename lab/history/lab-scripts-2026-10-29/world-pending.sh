#!/bin/bash
# After CONTINUE lands in a world whose sensor list is a placeholder: read the client's own pending list,
# publish it under its lifetime row's group, then watch (no input, so a cinematic is not skipped).
# Usage: world-pending.sh RUNNAME [TAG]
C=F:/d1/d1preservation-main/cfw-staging; S=lab/history/lab-scripts-2026-10-29
W=F:/d1/public-work/d1reimagined-sense/tools/ps4_setup; R=$C/$1; K=$R/capture; T=${2:-world}
grab() { (cd $S && python -c "
import importlib.util
s=importlib.util.spec_from_file_location('fight','fight.py');f=importlib.util.module_from_spec(s);s.loader.exec_module(f); f.grab('$K/$1.png')" 2>&1 | grep -v shadPS4); }
until grep -q '^  world' $R/continue.log 2>/dev/null; do sleep 10; done
D=$(ls -d $C/contained-boot/*-$1); H=$D/activity-host.log    # the boot folder exists by now
echo "$(date +%H:%M:%S) departure summary present"
recorders() { powershell -NoProfile -Command "(Get-CimInstance Win32_Process -Filter \"Name='python.exe'\" | Where-Object { \$_.CommandLine -match 'd1-record' }).Count"; }
until [ "$(recorders | tr -d '\r')" = "0" ]; do sleep 5; done     # one recorder at a time (shared audio tap)
(cd F:/d1/public-work/d1rs-claude && python -I tools/ps4_setup/d1-record.py --out $K/$T.mp4 --seconds 200 > $K/$T-record.log 2>&1 &)
P=$R/$T-pending.txt
(cd $W && timeout 120 python d1-winguest.py sensor-clients --types --bundle --out $P 2>&1 | tail -1 | sed 's/^/pending read: /')
n=$(grep -vc '^#' $P)
echo "  groups: $(grep -v '^#' $P | awk '{print $1}' | sort | uniq -c | tr '\n' ' ')"
G=$(grep -v '^#' $P | awk '$2=="0x10" {print $1; exit}')
echo "  lifetime rows: $(grep -v '^#' $P | awk '$2=="0x10"' | cut -c1-40 | tr '\n' ';')"
if [ "$n" -gt 0 ] && [ -n "$G" ]; then
  L=$R/$T-live.txt; { echo "# $1 $T: the client's own pending list, read live $(date +%H:%M) (d1-winguest sensor-clients --bundle)"; grep -v '^#' $P | sed 's/  #.*//'; } > $L
  n0=$(wc -l < $H); echo "clients $G $L" > $C/ah-control.txt; until [ ! -f $C/ah-control.txt ]; do sleep 1; done
  echo "$(date +%H:%M:%S) published $n clients under group $G"; sleep 25
  tail -n +$n0 $H | sed 's/\x1b\[[0-9;]*m//g' | grep -E "control:|CAPTURED udt (6|18|3|14)\b|incident|world table|live members" | cut -c12-27,60-190 | head -15
  (cd $W && timeout 120 python d1-winguest.py sensor-clients 2>&1 >/dev/null | tail -1 | sed 's/^/after publish: /')
else
  echo "  nothing to publish (pending $n, lifetime group '$G')"
fi
for k in $(seq 1 12); do sleep 10; grab $T-$(printf %03d $((k*10))); done
echo "$(date +%H:%M:%S) host since departure:"; sed 's/\x1b\[[0-9;]*m//g' $H | grep -E "activity follow|world table: activity|incident [0-9]+ reported|CAPTURED udt (14|3)\b" | tail -12 | cut -c12-27,60-190
