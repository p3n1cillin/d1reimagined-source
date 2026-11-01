#!/bin/bash
# Post-game CONTINUE on a parked run: pre-departure component read -> recall gate check -> monitor + recording ->
# hold triangle ~4.2 s (hold-to-confirm, 3 s) -> screenshots through the load. Usage: continue-hold.sh RUNNAME [--no-record]
# --no-record (ending-run.ps1 -NoRecord): there is no capture tour to wait for, so it waits for the post-game
# pushes to finish (postgame5.sh's last section), and the departure is not recorded.
S=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd); REPO=$(cd "$S/../.." && pwd)
C=$(python "$REPO/tools/ps4_setup/d1paths.py" staging); Q=$C/probes   # probe output (routepaths.probes())
R=$C/$1; K=$R/capture; mkdir -p $K
NOREC=0; [ "$2" = "--no-record" ] && NOREC=1
recorders() { powershell -NoProfile -Command "(Get-CimInstance Win32_Process -Filter \"Name='python.exe'\" | Where-Object { \$_.CommandLine -match 'd1-record' }).Count"; }
if [ $NOREC = 0 ]; then
  until grep -q 'recording stopped' $R/capture.log 2>/dev/null; do sleep 10; done
  until [ "$(recorders | tr -d '\r')" = "0" ]; do sleep 10; done
else
  until grep -q '== responder since phase 7' $R/postgame.log 2>/dev/null; do sleep 10; done
fi
echo "$(date +%H:%M:%S) tour done, no recorder running"
# The tour ends on ACTIVITY REWARDS: R1 twice selects ADVANCEMENT. Codex's read-only probe captures the live UI
# tree there (screen/body tags, visibility, post_game_data) before CONTINUE destroys it.
powershell -NoProfile -File "$REPO/tools/ps4_setup/d1-pad.ps1" -Keys U > /dev/null; sleep 1.5
powershell -NoProfile -File "$REPO/tools/ps4_setup/d1-pad.ps1" -Keys U > /dev/null; sleep 2
(cd $S && python -c "
import importlib.util
s=importlib.util.spec_from_file_location('fight','fight.py');f=importlib.util.module_from_spec(s);s.loader.exec_module(f); f.grab('$K/advancement-selected.png')" 2>&1 | grep -v shadPS4)
(cd $S && timeout 240 python probe-advancement-ui-readonly.py 2>&1 | tail -4 | sed 's/^/advancement probe: /')
g=$(cd $S && timeout 90 python probe-recall-gate-readonly.py 2>/dev/null | grep -m1 '\.json$')
python -c "
import json,sys; d=json.load(open(sys.argv[1])); print('gate', {k:d.get(k) for k in ('profile_completion673','plane_state','leader_predicate','selection_activity','ordinary_branch_predicted')})
sys.exit(0 if d.get('ordinary_branch_predicted') is True and d.get('selection_activity')==165 else 3)" "$g" || { echo "gate not ordinary/165: no input sent"; exit 3; }
[ $NOREC = 0 ] && (cd $REPO && python -I tools/ps4_setup/d1-record.py --out $K/continue.mp4 --seconds 240 > $K/continue-record.log 2>&1 &)
(cd $S && timeout 220 python monitor-recall-departure-readonly.py --run $1 --seconds 180 > $K/continue-monitor.out 2>&1 &)
(cd $S && timeout 220 python probe-hangar-cues-readonly.py --run $1 --seconds 180 --interval 0.25 > $K/continue-cues.out 2>&1 &)   # Codex: read-only world/cue catalogue through the load
until grep -q READY $K/continue-monitor.out 2>/dev/null; do sleep 0.5; done; sleep 3
cd $S && python -c "
import importlib.util,ctypes,time
s=importlib.util.spec_from_file_location('gf','gamefocus.py');gf=importlib.util.module_from_spec(s);s.loader.exec_module(gf)
s2=importlib.util.spec_from_file_location('fight','fight.py');f=importlib.util.module_from_spec(s2);s2.loader.exec_module(f)
u=ctypes.WinDLL('user32'); gf.ensure(); time.sleep(1)
print(time.strftime('%H:%M:%S'), 'hold')
u.keybd_event(0x43,u.MapVirtualKeyW(0x43,0),0,None); time.sleep(1.5); f.grab('$K/hold-ring.png'); time.sleep(2.7); u.keybd_event(0x43,u.MapVirtualKeyW(0x43,0),2,None)
print(time.strftime('%H:%M:%S'), 'release')
for k in range(1,13): time.sleep(10); f.grab('$K/after-%03d.png'%(k*10))
" 2>&1 | grep -v shadPS4
until grep -q '"stop_reason"' $Q/recall-departure-$1-*.summary.json 2>/dev/null; do sleep 10; done
python -c "
import json,sys,glob,os
f=max(glob.glob(r'$Q/recall-departure-$1-*.summary.json'),key=os.path.getmtime); d=json.load(open(f)); print(f)
for k,v in d['milestones_after_start'].items(): print('  ',k,v['utc'][11:23])
w=d['last_observed'].get('native_slow',{}).get('world'); print('  world',w)"
