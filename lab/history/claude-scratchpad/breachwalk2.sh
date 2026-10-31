cd /f/d1/public-work/d1reimagined-source
S=%USERPROFILE%/AppData/Local/Temp/claude/F--d1-d1xenia/eb49bdd6-2562-42a0-a73d-ed14111dbab8/scratchpad
T=$S/tracked-$1.json
timeout 700 python -I -u tools/ps4_setup/d1-walk.py --tracked $T --route missions/routes/breach.json --max-seconds 640 > $S/walk-$1.log 2>&1; grep -E "done|stopped|fell" $S/walk-$1.log
timeout 300 python -I -u tools/ps4_setup/d1-walk.py --tracked $T --route missions/routes/breach_platform.json --max-seconds 200 > $S/walk2-$1.log 2>&1; grep -E "done|stopped|fell" $S/walk2-$1.log
sleep 30
timeout 200 python -I -u tools/ps4_setup/d1-walk.py --tracked $T --route $S/rifle.json --max-seconds 150 --tolerance 1.5 > $S/walk3-$1.log 2>&1; grep -E "done|stopped|fell" $S/walk3-$1.log
