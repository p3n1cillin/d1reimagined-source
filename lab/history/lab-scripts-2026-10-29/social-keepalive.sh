#!/bin/bash
# Try to keep a parked Guardian from the social-space kick (peer_leave reason 0x4ca36d9a =
# error.inactivity_timed_out). Walking forward then back each minute (net zero displacement) did NOT
# prevent it (sense60, 22:26). This version alternates direction minute to minute, so the position really
# changes between checks, and adds a jump (cross) and a camera turn. Pauses while codex.lock exists.
# Usage: social-keepalive.sh [STOPFILE]   (runs until shadPS4 exits or STOPFILE appears)
S=lab/history/lab-scripts-2026-10-29; STOP=${1:-$S/keepalive.stop}
pad() { powershell -NoProfile -File F:/d1/tools/d1-pad.ps1 -Keys "$1" > /dev/null; }
k=0
while tasklist | grep -q shadPS4 && [ ! -f "$STOP" ]; do
  [ -f $S/codex.lock ] && { sleep 5; continue; }
  if [ $((k % 2)) -eq 0 ]; then pad Hold:W:800; pad Hold:L:300; else pad Hold:S:800; pad Hold:J:300; fi
  pad NumPad2                                   # jump
  date +%s > /f/d1/tools/d1-pad-last.txt
  echo "$(date +%H:%M:%S) keep-alive $k"
  k=$((k + 1)); sleep 60
done
