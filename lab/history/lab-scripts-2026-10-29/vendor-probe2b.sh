#!/bin/bash
# vendor-probe.sh with world-position steering: triangulate the vendor marker, seek.py to that point
# (it follows walls, so it finds the stairs up to a raised vendor), then hold square under the four
# execute breakpoints. Usage: vendor-probe2.sh RUNNAME
S=lab/history/lab-scripts-2026-10-29
R=F:/d1/d1preservation-main/cfw-staging/$1; K=$R/vendor-w2; mkdir -p $K
export D1_ROOT=F:/d1 D1_LAB_DRIVER=1
pad() { powershell -NoProfile -File F:/d1/tools/d1-pad.ps1 -Keys "$1" > /dev/null; }
shot() { (cd $S && python -c "
import importlib.util
s=importlib.util.spec_from_file_location('fight','fight.py');f=importlib.util.module_from_spec(s);s.loader.exec_module(f); f.grab('$K/$1.png')" 2>&1 | grep -v shadPS4); }
touch $S/codex.lock; python $S/release.py   # the keep-alive finishes its minute, then pauses
cd $S
for attempt in 1 2 3; do
  T=$(timeout 120 python -u triangulate.py --shots $K 2>$K/tri-$attempt.err | grep -v shadPS4 | tail -1)
  echo "$(date +%H:%M:%S) attempt $attempt: target '$T' $(grep -v shadPS4 $K/tri-$attempt.err | tail -1)"
  echo "$T" | grep -qE '^-?[0-9.]+,-?[0-9.]+$' || continue
  echo "  seek: $(timeout 200 python -u seek.py $T --no-nav --max-s 150 --tol 2.0 --trail $K/trail.jsonl 2>&1 | grep -v shadPS4 | tail -1)"
  python release.py
done
shot before
(cd F:/d1/public-work/d1rs-claude/tools/ps4_setup && timeout 150 python -u d1-watchwrite.py --seconds 90 --max-hits 40 \
   --exec 5c5a00 --exec 5c5a41 --exec a96110 --exec 552ef0 --out $K/watch.jsonl > $K/watch.log 2>&1 &)
sleep 8
for i in 1 2; do
  echo "$(date +%H:%M:%S) hold square $i"; pad Hold:NumPad4:2200; sleep 3; shot talk-$i
done
sleep 85
python release.py; rm -f $S/codex.lock
echo "$(date +%H:%M:%S) done; execute hits by address:"
grep '"event": "exec"' $K/watch.jsonl | grep -o '"rip_img": "[^"]*"' | sort | uniq -c
grep -E '"event": "(start|detached)"' $K/watch.jsonl | cut -c1-160
