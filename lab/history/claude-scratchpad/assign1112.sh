#!/bin/bash
# After trigger 41: publish Codex's squad 11/12 assignment draft, then sample their positions. Usage: assign1112.sh RUNNAME
C=F:/d1/d1preservation-main/cfw-staging; S=%USERPROFILE%/AppData/Local/Temp/claude/F--d1-d1xenia/eb49bdd6-2562-42a0-a73d-ed14111dbab8/scratchpad
until ls -d $C/contained-boot/*-$1 >/dev/null 2>&1; do sleep 5; done; D=$(ls -d $C/contained-boot/*-$1); H=$D/activity-host.log
until [ -f $H ] && sed 's/\x1b\[[0-9;]*m//g' $H | grep -q "mission step 21 at"; do sleep 3; done
sample() { python -c "
import importlib.util
s=importlib.util.spec_from_file_location('c','$S/combat2.py');c=importlib.util.module_from_spec(s);s.loader.exec_module(c)
W=c.World(); st=W.me(); m=[x for x in W.members(st[0],500) if x[1][2] in (11,12)]
print(' '.join('sq%d=(%.1f,%.1f,%.1f)'%(x[1][2],*x[2]) for x in m) or 'none')" 2>&1 | grep -v shadPS4 | tail -1; }
echo "$(date +%H:%M:%S) before: $(sample)"
sleep 8; echo "clients 0x4786c0e0 F:/d1/d1preservation-main/cfw-staging/agr-dock13-assign-1112.draft.txt" > $C/ah-control.txt
until [ ! -f $C/ah-control.txt ]; do sleep 1; done; sleep 2; sed 's/\x1b\[[0-9;]*m//g' $H | grep "assign-1112" | tail -1 | cut -c12-27,40-260
for k in 1 2 3 4 5 6 7 8 9 10 11 12; do sleep 10; echo "$(date +%H:%M:%S) $(sample)"; done
