#!/bin/bash
# After the outro: step the activity lifetime phase via the host's phase file, recording what the client does.
# Usage: phase-step.sh RUNNAME "4 5 7 8 9 10" [SECONDS]
S=%USERPROFILE%/AppData/Local/Temp/claude/F--d1-d1xenia/eb49bdd6-2562-42a0-a73d-ed14111dbab8/scratchpad
C=F:/d1/d1preservation-main/cfw-staging; D=$(ls -d $C/contained-boot/*-$1); H=$D/activity-host.log; R=$D/responder.log
mkdir -p $C/$1/phases
for p in $2; do
  t0=$(date -u +%Y-%m-%dT%H:%M:%S); r0=$(grep -a -c "INVESTMENT message id" $R)
  echo $p > $C/ah-phase.txt; echo "$(date +%H:%M:%S) phase $p"
  sleep ${3:-30}
  python -c "
import importlib.util
s=importlib.util.spec_from_file_location('fight','$S/fight.py');f=importlib.util.module_from_spec(s);s.loader.exec_module(f); f.grab('$C/$1/phases/phase-$p.png')" 2>&1 | grep -v shadPS4
  sed 's/\x1b\[[0-9;]*m//g' $H | awk -v t=$t0 '$1>t' | grep -vE "heartbeat|WantsToPush|user_defined_type: 15\b|sensor_auth_update send" | grep -E "incident|slice|teleport|udt|Notification|phase|error|ERROR|evict|disconnect" | sort -u -k2 | head -8 | cut -c12-27,60-200 | sed 's/^/   /'
  echo "   new investment ids: $(grep -a -o 'INVESTMENT message id [0-9]*' $R | tail -n +$((r0+1)) | awk '{print $4}' | sort | uniq -c | tr '\n' ' ')"
done
echo x > $C/ah-phase.txt; echo "$(date +%H:%M:%S) phase file back to x"
