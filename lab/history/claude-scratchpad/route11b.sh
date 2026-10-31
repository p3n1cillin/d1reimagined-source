S=%USERPROFILE%/AppData/Local/Temp/claude/F--d1-d1xenia/eb49bdd6-2562-42a0-a73d-ed14111dbab8/scratchpad
R=/f/d1/public-work/d1reimagined-source
cd $R; T=$S/tracked-r11.json
python -I -u tools/ps4_setup/d1-walk.py --tracked $T --retrack | tail -1
timeout 700 python -I -u tools/ps4_setup/d1-walk.py --tracked $T --route $S/breach-from14.json --avoid 330,-837,340,-812 --max-seconds 640 > $S/walk-r11a.log 2>&1; echo "breach: $(grep -E 'done|stopped' $S/walk-r11a.log | tail -1)"
timeout 300 python -I -u tools/ps4_setup/d1-walk.py --tracked $T --route missions/routes/breach_platform.json --max-seconds 200 > $S/walk-r11b.log 2>&1; echo "platform: $(grep -E 'done|stopped' $S/walk-r11b.log | tail -1)"
timeout 200 python -I -u tools/ps4_setup/d1-walk.py --tracked $T --route $S/rifle.json --max-seconds 150 --tolerance 1.5 > $S/walk-r11c.log 2>&1; echo "rifle: $(grep -E 'done|stopped' $S/walk-r11c.log | tail -1)"
cd $S
python -I tools/d1-step.py --tracked $T --trail trail-r11.json --keys Hold:S:1200,Hold:W:1300 | tail -1
python -I -u tools/d1-walk.py --tracked $T --retrack | tail -1
timeout 900 python -I -u tools/d1-walk.py --tracked $T --route to-chest.json --max-seconds 840 --tolerance 1.3 > walk-r11chest.log 2>&1; echo "chest: $(grep -E 'done|stopped' walk-r11chest.log | tail -1)"
T=$S/tracked-r11.json
timeout 120 python -I -u tools/d1-walk.py --tracked $T --route mine-a.json --max-seconds 90 --tolerance 1.2 > walk-r11-ma.log 2>&1
powershell -NoProfile -File /f/d1/tools/d1-pad.ps1 -Keys NumPad6 > /dev/null
timeout 300 python -I -u tools/d1-walk.py --tracked $T --route mine-b.json --max-seconds 240 --tolerance 1.5 > walk-r11-mb.log 2>&1
powershell -NoProfile -File /f/d1/tools/d1-pad.ps1 -Keys NumPad6 > /dev/null
timeout 200 python -I -u tools/d1-walk.py --tracked $T --route mine-c.json --max-seconds 150 --tolerance 1.2 > walk-r11-mc.log 2>&1
echo "junction: $(grep -E 'done|stopped' walk-r11-mc.log | tail -1)"
timeout 400 python -I -u tools/d1-walk.py --tracked $T --route jw-route.json --max-seconds 330 --tolerance 1.3 > walk-r11-jw.log 2>&1
echo "loop: $(grep -E 'done|stopped' walk-r11-jw.log | tail -1)"
echo 'clients 0x4786c0e0 F:/d1/d1preservation-main/cfw-staging/agr-b-props-378.txt' > /f/d1/d1preservation-main/cfw-staging/ah-control.txt; sleep 12
R=/f/d1/public-work/d1reimagined-source/missions/routes
timeout 200 python -I -u tools/d1-walk.py --tracked $T --route $R/wall_garage_door.json --max-seconds 150 --tolerance 1.0 > walk-r11-door.log 2>&1; echo "door: $(grep -E 'done|stopped' walk-r11-door.log | tail -1)"
timeout 500 python -I -u tools/d1-walk.py --tracked $T --route $R/wall_fan_tunnel_to_divide.json --max-seconds 420 --tolerance 1.3 > walk-r11-fan.log 2>&1; echo "divide: $(grep -E 'done|stopped' walk-r11-fan.log | tail -1)"
