#!/bin/bash
# XP + ship run (Codex's collector timing): while the profile activity is still ffff, push active164/XP0 (collector
# target 2); at Dock 13 push active164/XP900 (sample recorded in stage 2); after the outro push the completed profile
# with XP900 and the owned Arcadia plus its inventory item (PR #81 responder); then scoreboard, @phase 7, tour.
# Usage: postgame4.sh RUNNAME [SCOREBOARD_LIST]
C=F:/d1/d1preservation-main/cfw-staging; Q=lab/history/2026-10-29-postgame-codex
S=lab/history/lab-scripts-2026-10-29
SB=${2:-$C/agr-postgame-scoreboard-arcadia-ratio30.draft.txt}
grab() { python -c "
import importlib.util
s=importlib.util.spec_from_file_location('fight','$S/fight.py');f=importlib.util.module_from_spec(s);s.loader.exec_module(f)
try: f.grab('$C/$1/capture/$2.png')
except Exception: pass" 2>/dev/null; }
publish() { echo "clients 0x4786c0e0 $1" > $C/ah-control.txt; until [ ! -f $C/ah-control.txt ]; do sleep 1; done; sleep 2
  sed 's/\x1b\[[0-9;]*m//g' $H | grep "control:" | tail -1 | cut -c12-27,60-260; }
collector() { (cd $Q && timeout 60 python probe-postgame.py 2>/dev/null | tail -n +2 | python -c "
import sys,json
try:
  d=json.load(sys.stdin); c=d['collector']; print('    collector', {k:c[k] for k in ('state','target','activity','level_xp')}, 'selection', d['selection']['activity'])
except Exception as e: print('    collector read failed', e)"); }
push() { cp $1 $C/profile-push.bin; until [ ! -f $C/profile-push.bin ]; do sleep 1; done; sleep 3
  echo "$(date +%H:%M:%S) pushed $(basename $1): $(grep -a 'PROFILE PUSH' $D/responder.log | tail -1 | cut -c1-110)"; collector; }
until ls -d $C/contained-boot/*-$1 >/dev/null 2>&1; do sleep 5; done
D=$(ls -d $C/contained-boot/*-$1); H=$D/activity-host.log; mkdir -p $C/$1/capture
until grep -q 'DIVIDE REACHED' $C/$1/replay-console.log 2>/dev/null; do sleep 5; done
collector; push $C/profile-active164-xp0.draft.bin
sleep 20; collector
until sed 's/\x1b\[[0-9;]*m//g' $H | grep -q "now in slice set 104"; do sleep 5; done
push $C/profile-active164-xp900.draft.bin
sleep 20; collector
until sed 's/\x1b\[[0-9;]*m//g' $H | grep -q "mission step 25"; do sleep 3; done
sed 's/\x1b\[[0-9;]*m//g' $H | grep -E "mission step 2[2-5]|incident (2681|891)" | tail -5 | cut -c12-27,60-160
sleep 5; cp $C/arcadia-3517-inventory.draft.bin $C/profile-push.item; echo d1000000000003f1 > $C/profile-push.soid
push $C/arcadia-profile-xp900-owned.draft.bin
grep -a -i "item" $D/responder.log | tail -3 | cut -c1-140
sleep 20; collector
echo "$(date +%H:%M:%S) parked (phase 3); waiting for $SB"
until [ -f $SB ]; do sleep 10; done
(cd F:/d1/public-work/d1rs-claude && python -I tools/ps4_setup/d1-record.py --out $C/$1/capture/postgame.mp4 --seconds 300 >/dev/null 2>&1 &)
sleep 4; publish $SB; sleep 15
t0=$(date -u +%Y-%m-%dT%H:%M:%S); r0=$(wc -l < $D/responder.log)
publish F:/d1/d1preservation-main/cfw-staging/agr-postgame-phase7.draft.txt
for k in 1 2 3 4 5 6; do sleep 15; grab $1 postgame-$k; done
collector
echo "== host since phase 7"; sed 's/\x1b\[[0-9;]*m//g' $H | awk -v t=$t0 '$1>t' | grep -vE "heartbeat|WantsToPush|user_defined_type: 15\b|sensor_auth_update send" | head -15 | cut -c12-27,60-200
echo "== responder since phase 7"; tail -n +$r0 $D/responder.log | grep -a -E "RECV type|INVESTMENT message id" | grep -v "0x00fa\|id 70[12]\|RECV type 0x000a" | head -12 | cut -c1-160
