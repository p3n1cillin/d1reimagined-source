S=%USERPROFILE%/AppData/Local/Temp/claude/F--d1-d1xenia/eb49bdd6-2562-42a0-a73d-ed14111dbab8/scratchpad
R=/f/d1/public-work/d1reimagined-source
cd $R; T=$S/tracked-r8.json
timeout 700 python -I -u tools/ps4_setup/d1-walk.py --tracked $T --route missions/routes/breach.json --avoid 330,-837,340,-812 --max-seconds 640 > $S/walk-r8a.log 2>&1; echo "breach: $(grep -E 'done|stopped' $S/walk-r8a.log | tail -1)"
timeout 300 python -I -u tools/ps4_setup/d1-walk.py --tracked $T --route missions/routes/breach_platform.json --max-seconds 200 > $S/walk-r8b.log 2>&1; echo "platform: $(grep -E 'done|stopped' $S/walk-r8b.log | tail -1)"
timeout 200 python -I -u tools/ps4_setup/d1-walk.py --tracked $T --route $S/rifle.json --max-seconds 150 --tolerance 1.5 > $S/walk-r8c.log 2>&1; echo "rifle: $(grep -E 'done|stopped' $S/walk-r8c.log | tail -1)"
cd $S
python -I tools/d1-step.py --tracked $T --trail trail-r8.json --keys Hold:S:1200,Hold:W:1300 | tail -1
python -I -u tools/d1-walk.py --tracked $T --retrack | tail -1
timeout 900 python -I -u tools/d1-walk.py --tracked $T --route to-chest.json --max-seconds 840 --tolerance 1.3 > walk-r8chest.log 2>&1; echo "chest: $(grep -E 'done|stopped' walk-r8chest.log | tail -1)"
