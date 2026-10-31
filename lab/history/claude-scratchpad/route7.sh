S=%USERPROFILE%/AppData/Local/Temp/claude/F--d1-d1xenia/eb49bdd6-2562-42a0-a73d-ed14111dbab8/scratchpad
R=/f/d1/public-work/d1reimagined-source
until grep -q 'in gameplay' $S/cap/boot-route7.log 2>/dev/null; do sleep 5; done; sleep 15
until powershell -NoProfile -File /f/d1/tools/d1-shot.ps1 -Out $S/r7.png >/dev/null && python -I -W ignore $S/lum.py $S/r7.png | awk '{exit !($3>40)}'; do sleep 5; done
cd $R
bash $S/breachwalk.sh r7 2>&1 | tail -3
cd $S
python -I tools/d1-step.py --tracked tracked-r7.json --trail trail-r7.json --keys Hold:S:1200,Hold:W:1300 | tail -1
python -I -u tools/d1-walk.py --tracked tracked-r7.json --retrack | tail -1
timeout 900 python -I -u tools/d1-walk.py --tracked tracked-r7.json --route to-chest.json --max-seconds 840 --tolerance 1.3 > walk-r7chest.log 2>&1; grep -E "done|stopped" walk-r7chest.log
