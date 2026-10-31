#!/bin/bash
# After CONTINUE lands in the Tower: record a slow camera circle (the authored squads), save a still per
# step, then keep the Guardian from the inactivity kick. Usage: tower-arrival.sh RUNNAME
C=F:/d1/d1preservation-main/cfw-staging; S=lab/history/lab-scripts-2026-10-29
R=$C/$1; K=$R/capture
recorders() { powershell -NoProfile -Command "(Get-CimInstance Win32_Process -Filter \"Name='python.exe'\" | Where-Object { \$_.CommandLine -match 'd1-record' }).Count"; }
pad() { powershell -NoProfile -File F:/d1/tools/d1-pad.ps1 -Keys "$1" > /dev/null; }
grab() { (cd $S && python -c "
import importlib.util
s=importlib.util.spec_from_file_location('fight','fight.py');f=importlib.util.module_from_spec(s);s.loader.exec_module(f); f.grab('$K/$1.png')" 2>&1 | grep -v shadPS4); }
until grep -q '^  world' $R/continue.log 2>/dev/null; do sleep 10; done
grep '^  world' $R/continue.log
until [ "$(recorders | tr -d '\r')" = "0" ]; do sleep 5; done
(cd F:/d1/public-work/d1rs-claude && python -I tools/ps4_setup/d1-record.py --out $K/tower-pan.mp4 --seconds 150 > $K/tower-pan-record.log 2>&1 &)
sleep 6
for i in $(seq 1 12); do pad Hold:L:232; sleep 5; grab pan-$(printf %02d $i); done     # 12 x 30 deg, 5 s each
date +%s > /f/d1/tools/d1-pad-last.txt
echo "$(date +%H:%M:%S) pan done; keep-alive on"
exec bash $S/social-keepalive.sh
