#!/bin/bash
# After the outro stop step: push the completed profile and record what the client does. Usage: complete-after-outro.sh RUNNAME
C=F:/d1/d1preservation-main/cfw-staging; P=lab/history/2026-10-28-evening-claude
S=%USERPROFILE%/AppData/Local/Temp/claude/F--d1-d1xenia/eb49bdd6-2562-42a0-a73d-ed14111dbab8/scratchpad
until ls -d $C/contained-boot/*-$1 >/dev/null 2>&1; do sleep 5; done
D=$(ls -d $C/contained-boot/*-$1); H=$D/activity-host.log
until sed 's/\x1b\[[0-9;]*m//g' $H | grep -q "mission step 25"; do sleep 3; done
sed 's/\x1b\[[0-9;]*m//g' $H | grep -E "mission step 2[2-5]|incident (2681|891)" | cut -c12-27,60-160
mkdir -p $C/$1/capture
(cd F:/d1/public-work/d1rs-claude && python -I tools/ps4_setup/d1-record.py --out $C/$1/capture/completion.mp4 --seconds 180 >/dev/null 2>&1 &)
sleep 5; t0=$(date -u +%Y-%m-%dT%H:%M:%S)
cp $P/${2:-profile-complete164.bin} $C/profile-push.bin; echo "$(date +%H:%M:%S) completed profile staged"
for k in 1 2 3 4 5 6; do
  sleep 20
  python -c "
import importlib.util
s=importlib.util.spec_from_file_location('fight','$S/fight.py');f=importlib.util.module_from_spec(s);s.loader.exec_module(f); f.grab('$C/$1/capture/complete-$k.png')" 2>&1 | grep -v shadPS4
done
echo "== responder"; grep -a -E "PROFILE PUSH|INVESTMENT message id|REFUSED|refus" $D/responder.log | grep -v "id 70[12]" | tail -8
echo "== host"; sed 's/\x1b\[[0-9;]*m//g' $H | awk -v t=$t0 '$1>t' | grep -vE "heartbeat|WantsToPush|user_defined_type: 15\b|sensor_auth_update send" | grep -E "incident|slice|teleport|udt|Notification|error|evict|disconnect|closed" | head -12 | cut -c12-27,60-200
grep -a -i "activity-chain\|completion\|investment:activity" $D/emulator.log | tail -5
