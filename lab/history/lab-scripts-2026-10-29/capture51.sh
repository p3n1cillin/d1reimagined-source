#!/bin/bash
# Marketing capture: record from Dock 13 arrival through the post-game screens, tour the tabs, stop.
C=F:/d1/d1preservation-main/cfw-staging; S=lab/history/lab-scripts-2026-10-29
P=lab/history/2026-10-28-evening-claude; R=$1
until ls -d $C/contained-boot/*-$R >/dev/null 2>&1; do sleep 5; done; D=$(ls -d $C/contained-boot/*-$R); H=$D/activity-host.log
mkdir -p $C/$R/capture; STOP=$C/$R/capture/stop.flag; rm -f $STOP
until [ -f $H ] && sed 's/\x1b\[[0-9;]*m//g' $H | grep -q "now in slice set 104"; do sleep 3; done
echo "$(date +%H:%M:%S) Dock 13: recording"
(cd F:/d1/public-work/d1rs-claude && python -I tools/ps4_setup/d1-record.py --out $C/$R/capture/ending.mp4 --seconds 2400 --stop-file $STOP > $C/$R/capture/ending-record.log 2>&1 &)
until sed 's/\x1b\[[0-9;]*m//g' $H | grep -q "control: sensor clients F:/d1/d1preservation-main/cfw-staging/agr-postgame-phase7.draft.txt"; do sleep 3; done
echo "$(date +%H:%M:%S) phase 7 published"
sleep 25
python -c "
import importlib.util,ctypes,time
s=importlib.util.spec_from_file_location('gf','$S/gamefocus.py');gf=importlib.util.module_from_spec(s);s.loader.exec_module(gf)
u=ctypes.WinDLL('user32')
def tap(vk):
    gf.ensure(); u.keybd_event(vk,u.MapVirtualKeyW(vk,0),0,None); time.sleep(0.15); u.keybd_event(vk,u.MapVirtualKeyW(vk,0),2,None)
time.sleep(12); tap(0x55); time.sleep(14); tap(0x55); time.sleep(12); tap(0x55); time.sleep(10)" 2>&1 | tail -1
touch $STOP; echo "$(date +%H:%M:%S) recording stopped"; sleep 30; tail -3 $C/$R/capture/ending-record.log
