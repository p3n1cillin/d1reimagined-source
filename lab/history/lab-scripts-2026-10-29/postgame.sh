#!/bin/bash
# After the outro: push the completed profile, then publish the @phase 7 list; record what follows. Usage: postgame.sh RUNNAME
C=F:/d1/d1preservation-main/cfw-staging; P=lab/history/2026-10-28-evening-claude
S=lab/history/lab-scripts-2026-10-29
grab() { python -c "
import importlib.util
s=importlib.util.spec_from_file_location('fight','$S/fight.py');f=importlib.util.module_from_spec(s);s.loader.exec_module(f)
try: f.grab('$C/$1/capture/$2.png')
except Exception: pass" 2>/dev/null; }
until ls -d $C/contained-boot/*-$1 >/dev/null 2>&1; do sleep 5; done
D=$(ls -d $C/contained-boot/*-$1); H=$D/activity-host.log
until sed 's/\x1b\[[0-9;]*m//g' $H | grep -q "mission step 25"; do sleep 3; done
sed 's/\x1b\[[0-9;]*m//g' $H | grep -E "mission step 2[2-5]|incident (2681|891)" | tail -6 | cut -c12-27,60-160
mkdir -p $C/$1/capture
(cd F:/d1/public-work/d1rs-claude && python -I tools/ps4_setup/d1-record.py --out $C/$1/capture/postgame.mp4 --seconds 300 >/dev/null 2>&1 &)
sleep 5; cp $P/profile-complete164-prog.bin $C/profile-push.bin; echo "$(date +%H:%M:%S) completed profile staged"
sleep 25; grep -a "PROFILE PUSH" $D/responder.log | tail -1 | cut -c1-140
echo "pending before phase 7:"; (cd F:/d1/public-work/d1rs-claude/tools/ps4_setup && timeout 60 python -I d1-winguest.py sensor-clients 2>&1 | grep -v "^shadPS4" | head -4)
t0=$(date -u +%Y-%m-%dT%H:%M:%S); r0=$(grep -a -c "INVESTMENT message id\|RECV type" $D/responder.log)
echo "clients 0x4786c0e0 F:/d1/d1preservation-main/cfw-staging/agr-postgame-phase7.draft.txt" > $C/ah-control.txt
until [ ! -f $C/ah-control.txt ]; do sleep 1; done; echo "$(date +%H:%M:%S) phase 7 list published"
sed 's/\x1b\[[0-9;]*m//g' $H | grep "control:" | tail -2 | cut -c12-27,60-200
for k in 1 2 3 4 5 6 7 8; do sleep 15; grab $1 postgame-$k; done
echo "== host since phase 7"; sed 's/\x1b\[[0-9;]*m//g' $H | awk -v t=$t0 '$1>t' | grep -vE "heartbeat|WantsToPush|user_defined_type: 15\b|sensor_auth_update send" | grep -E "incident|slice|teleport|udt|Notification|phase|error|evict|disconnect|closed|join" | sort -u -k3 | head -15 | cut -c12-27,60-200
echo "== responder since phase 7"; tail -n +$((r0)) $D/responder.log | grep -a -E "RECV type|INVESTMENT message id|REFUSED" | grep -v "0x00fa\|id 70[12]\|0x000a" | head -15 | cut -c1-160
grep -a -c "INVESTMENT message id" $D/responder.log
