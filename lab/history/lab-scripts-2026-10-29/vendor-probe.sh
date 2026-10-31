#!/bin/bash
# In the Tower: steer to the vendor marker, then hold square (talk) under four execute breakpoints that
# show how far the vendor path gets:
#   5c5a00  vendor dialogue action's execute (vtable 0x19a9d20)  - the interaction reached the dialogue
#   5c5a41  ... past its entity flag gate (+0x40 bit 0)           - it will call ui_launch_vendor_screen
#   a96110  the screen launch body (after the deferred-queue hop)
#   552ef0  the 904 vendor_interact sender                         - the screen asked the server
# Pauses social-keepalive.sh (codex.lock) while it drives. Usage: vendor-probe.sh RUNNAME
S=lab/history/lab-scripts-2026-10-29
R=F:/d1/d1preservation-main/cfw-staging/$1; K=$R/vendor; mkdir -p $K
pad() { powershell -NoProfile -File F:/d1/tools/d1-pad.ps1 -Keys "$1" > /dev/null; }
shot() { (cd $S && python -c "
import importlib.util
s=importlib.util.spec_from_file_location('fight','fight.py');f=importlib.util.module_from_spec(s);s.loader.exec_module(f); f.grab('$K/$1.png')" 2>&1 | grep -v shadPS4); }
touch $S/codex.lock; sleep 65                 # let the keep-alive finish its current minute
python $S/release.py
echo "$(date +%H:%M:%S) steering to the vendor marker"
timeout 420 python -u $S/bluepoint.py --steps 30 --shots $K 2>&1 | tail -3; python $S/release.py
shot before
(cd F:/d1/public-work/d1rs-claude/tools/ps4_setup && timeout 150 python -u d1-watchwrite.py --seconds 90 --max-hits 40 \
   --exec 5c5a00 --exec 5c5a41 --exec a96110 --exec 552ef0 --out $K/watch.jsonl > $K/watch.log 2>&1 &)
sleep 8
for i in 1 2; do
  echo "$(date +%H:%M:%S) hold square $i"; pad Hold:NumPad4:2200; sleep 3; shot talk-$i
  pad Hold:W:300; sleep 1
done
sleep 85
python $S/release.py; rm -f $S/codex.lock
echo "$(date +%H:%M:%S) done; execute hits by address:"
grep '"event": "exec"' $K/watch.jsonl | grep -o '"rip_img": "[^"]*"' | sort | uniq -c
grep -E '"event": "(start|detached)"' $K/watch.jsonl | cut -c1-200
