#!/bin/bash
# After the outro: push the completed profile, wait for the scoreboard draft, publish it, then @phase 7.
# Usage: postgame2.sh RUNNAME [SCOREBOARD_LIST]
C=F:/d1/d1preservation-main/cfw-staging; P=lab/history/2026-10-28-evening-claude
S=%USERPROFILE%/AppData/Local/Temp/claude/F--d1-d1xenia/eb49bdd6-2562-42a0-a73d-ed14111dbab8/scratchpad
SB=${2:-$C/agr-postgame-scoreboard.draft.txt}
grab() { python -c "
import importlib.util
s=importlib.util.spec_from_file_location('fight','$S/fight.py');f=importlib.util.module_from_spec(s);s.loader.exec_module(f)
try: f.grab('$C/$1/capture/$2.png')
except Exception: pass" 2>/dev/null; }
publish() { echo "clients 0x4786c0e0 $1" > $C/ah-control.txt; until [ ! -f $C/ah-control.txt ]; do sleep 1; done; sleep 2
  sed 's/\x1b\[[0-9;]*m//g' $H | grep "control:" | tail -1 | cut -c12-27,60-260; }
until ls -d $C/contained-boot/*-$1 >/dev/null 2>&1; do sleep 5; done
D=$(ls -d $C/contained-boot/*-$1); H=$D/activity-host.log; mkdir -p $C/$1/capture
until sed 's/\x1b\[[0-9;]*m//g' $H | grep -q "mission step 25"; do sleep 3; done
sed 's/\x1b\[[0-9;]*m//g' $H | grep -E "mission step 2[2-5]|incident (2681|891)" | tail -5 | cut -c12-27,60-160
sleep 5; cp $P/profile-complete164-prog.bin $C/profile-push.bin; echo "$(date +%H:%M:%S) completed profile staged"
sleep 25; grep -a "PROFILE PUSH" $D/responder.log | tail -1 | cut -c1-120
# XP after the completion: the collector seeds on completion (stage 2) and only records samples in stages 2-3,
# so a rise now is counted as XP gained when phase 7 finalizes it.
cp $P/profile-complete164-xp900.bin $C/profile-push.bin; until [ ! -f $C/profile-push.bin ]; do sleep 1; done; sleep 3
grep -a "PROFILE PUSH" $D/responder.log | tail -1 | cut -c1-120; echo "$(date +%H:%M:%S) XP-900 delta sent"
echo "$(date +%H:%M:%S) parked (phase 3); waiting for $SB"
until [ -f $SB ]; do sleep 10; done
echo "$(date +%H:%M:%S) scoreboard draft present"
(cd F:/d1/public-work/d1rs-claude && python -I tools/ps4_setup/d1-record.py --out $C/$1/capture/postgame.mp4 --seconds 300 >/dev/null 2>&1 &)
sleep 4; publish $SB; sleep 15
t0=$(date -u +%Y-%m-%dT%H:%M:%S); r0=$(wc -l < $D/responder.log)
publish F:/d1/d1preservation-main/cfw-staging/agr-postgame-phase7.draft.txt
for k in 1 2 3 4 5 6; do sleep 15; grab $1 postgame-$k; done
echo "== host since phase 7"; sed 's/\x1b\[[0-9;]*m//g' $H | awk -v t=$t0 '$1>t' | grep -vE "heartbeat|WantsToPush|user_defined_type: 15\b|sensor_auth_update send" | head -15 | cut -c12-27,60-200
echo "== responder since phase 7"; tail -n +$r0 $D/responder.log | grep -a -E "RECV type|INVESTMENT message id" | grep -v "0x00fa\|id 70[12]\|RECV type 0x000a" | head -12 | cut -c1-160
