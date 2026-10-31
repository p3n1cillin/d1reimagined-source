S=%USERPROFILE%/AppData/Local/Temp/claude/F--d1-d1xenia/eb49bdd6-2562-42a0-a73d-ed14111dbab8/scratchpad
R=/f/d1/public-work/d1reimagined-source
cd $S; T=$S/tracked-r9.json
timeout 300 python -I -u tools/d1-walk.py --tracked $T --route breach-tail.json --max-seconds 240 > walk-r10a.log 2>&1; echo "breach: $(grep -E 'done|stopped' walk-r10a.log | tail -1)"
timeout 300 python -I -u tools/d1-walk.py --tracked $T --route $R/missions/routes/breach_platform.json --max-seconds 200 > walk-r10b.log 2>&1; echo "platform: $(grep -E 'done|stopped' walk-r10b.log | tail -1)"
timeout 200 python -I -u tools/d1-walk.py --tracked $T --route rifle.json --max-seconds 150 --tolerance 1.5 > walk-r10c.log 2>&1; echo "rifle: $(grep -E 'done|stopped' walk-r10c.log | tail -1)"
python -I tools/d1-step.py --tracked $T --trail trail-r10.json --keys Hold:S:1200,Hold:W:1300 | tail -1
python -I -u tools/d1-walk.py --tracked $T --retrack | tail -1
timeout 900 python -I -u tools/d1-walk.py --tracked $T --route to-chest.json --max-seconds 840 --tolerance 1.3 > walk-r10chest.log 2>&1; echo "chest: $(grep -E 'done|stopped' walk-r10chest.log | tail -1)"
timeout 120 python -I -u tools/d1-walk.py --tracked $T --route mine-a.json --max-seconds 90 --tolerance 1.2 > walk-r10-ma.log 2>&1
powershell -NoProfile -File /f/d1/tools/d1-pad.ps1 -Keys NumPad6 > /dev/null
timeout 300 python -I -u tools/d1-walk.py --tracked $T --route mine-b.json --max-seconds 240 --tolerance 1.5 > walk-r10-mb.log 2>&1
powershell -NoProfile -File /f/d1/tools/d1-pad.ps1 -Keys NumPad6 > /dev/null
timeout 200 python -I -u tools/d1-walk.py --tracked $T --route mine-c.json --max-seconds 150 --tolerance 1.2 > walk-r10-mc.log 2>&1
echo "junction: $(grep -E 'done|stopped' walk-r10-mc.log | tail -1)"
timeout 400 python -I -u tools/d1-walk.py --tracked $T --route jw-route.json --max-seconds 330 --tolerance 1.3 > walk-r10-jw.log 2>&1
echo "loop: $(grep -E 'done|stopped' walk-r10-jw.log | tail -1)"
