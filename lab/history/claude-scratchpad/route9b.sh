S=%USERPROFILE%/AppData/Local/Temp/claude/F--d1-d1xenia/eb49bdd6-2562-42a0-a73d-ed14111dbab8/scratchpad
cd $S; T=tracked-r9.json
python -I -u tools/d1-walk.py --tracked $T --retrack | tail -1
timeout 900 python -I -u tools/d1-walk.py --tracked $T --route to-chest.json --max-seconds 840 --tolerance 1.3 > walk-r9chest2.log 2>&1; echo "chest: $(grep -E 'done|stopped' walk-r9chest2.log | tail -1)"
timeout 120 python -I -u tools/d1-walk.py --tracked $T --route mine-a.json --max-seconds 90 --tolerance 1.2 > walk-r9-ma.log 2>&1
powershell -NoProfile -File /f/d1/tools/d1-pad.ps1 -Keys NumPad6 > /dev/null
timeout 300 python -I -u tools/d1-walk.py --tracked $T --route mine-b.json --max-seconds 240 --tolerance 1.5 > walk-r9-mb.log 2>&1
powershell -NoProfile -File /f/d1/tools/d1-pad.ps1 -Keys NumPad6 > /dev/null
timeout 200 python -I -u tools/d1-walk.py --tracked $T --route mine-c.json --max-seconds 150 --tolerance 1.2 > walk-r9-mc.log 2>&1
echo "junction: $(grep -E 'done|stopped' walk-r9-mc.log | tail -1)"
timeout 400 python -I -u tools/d1-walk.py --tracked $T --route jw-route.json --max-seconds 330 --tolerance 1.3 > walk-r9-jw.log 2>&1
echo "loop: $(grep -E 'done|stopped' walk-r9-jw.log | tail -1)"
