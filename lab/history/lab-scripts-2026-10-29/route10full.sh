S=lab/history/lab-scripts-2026-10-29
until grep -q "in gameplay" $S/cap/boot-route10.log 2>/dev/null; do sleep 5; done; sleep 20
R=/f/d1/public-work/d1reimagined-source
cd $R; T=$S/tracked-r10.json; timeout 600 python -I -u tools/ps4_setup/d1-walk.py --scan 383,-962,12.5 --radius 25 --out $T 2>&1 | grep tracked | tail -1
cd $R; T=$S/tracked-r10.json
timeout 700 python -I -u tools/ps4_setup/d1-walk.py --tracked $T --route missions/routes/breach.json --avoid 330,-837,340,-812 --max-seconds 640 > $S/walk-r10a.log 2>&1; echo "breach: $(grep -E 'done|stopped' $S/walk-r10a.log | tail -1)"
timeout 300 python -I -u tools/ps4_setup/d1-walk.py --tracked $T --route missions/routes/breach_platform.json --max-seconds 200 > $S/walk-r10b.log 2>&1; echo "platform: $(grep -E 'done|stopped' $S/walk-r10b.log | tail -1)"
timeout 200 python -I -u tools/ps4_setup/d1-walk.py --tracked $T --route $S/rifle.json --max-seconds 150 --tolerance 1.5 > $S/walk-r10c.log 2>&1; echo "rifle: $(grep -E 'done|stopped' $S/walk-r10c.log | tail -1)"
cd $S
python -I tools/d1-step.py --tracked $T --trail trail-r10.json --keys Hold:S:1200,Hold:W:1300 | tail -1
python -I -u tools/d1-walk.py --tracked $T --retrack | tail -1
timeout 900 python -I -u tools/d1-walk.py --tracked $T --route to-chest.json --max-seconds 840 --tolerance 1.3 > walk-r10chest.log 2>&1; echo "chest: $(grep -E 'done|stopped' walk-r10chest.log | tail -1)"
T=$S/tracked-r10.json
timeout 120 python -I -u tools/d1-walk.py --tracked $T --route mine-a.json --max-seconds 90 --tolerance 1.2 > walk-r10-ma.log 2>&1
powershell -NoProfile -File /f/d1/tools/d1-pad.ps1 -Keys NumPad6 > /dev/null
timeout 300 python -I -u tools/d1-walk.py --tracked $T --route mine-b.json --max-seconds 240 --tolerance 1.5 > walk-r10-mb.log 2>&1
powershell -NoProfile -File /f/d1/tools/d1-pad.ps1 -Keys NumPad6 > /dev/null
timeout 200 python -I -u tools/d1-walk.py --tracked $T --route mine-c.json --max-seconds 150 --tolerance 1.2 > walk-r10-mc.log 2>&1
echo "junction: $(grep -E 'done|stopped' walk-r10-mc.log | tail -1)"
timeout 400 python -I -u tools/d1-walk.py --tracked $T --route jw-route.json --max-seconds 330 --tolerance 1.3 > walk-r10-jw.log 2>&1
echo "loop: $(grep -E 'done|stopped' walk-r10-jw.log | tail -1)"
