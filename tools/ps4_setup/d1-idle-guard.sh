#!/bin/bash
# d1-idle-guard.sh: while shadPS4 runs, press the keep-alive (square + a stick nudge) whenever no
# pad input has been sent for 5 minutes. The client kicks an idle player after 15.
F=/f/d1/tools/d1-pad-last.txt
while tasklist | grep -q shadPS4; do
  last=$(cat $F 2>/dev/null | tr -d '\r\n'); now=$(date +%s)
  if [ -z "$last" ] || [ $((now - last)) -gt 300 ]; then
    powershell -NoProfile -File /f/d1/tools/d1-pad.ps1 -Keys NumPad4,Hold:W:150,Hold:S:150 > /dev/null
  fi
  sleep 60
done
