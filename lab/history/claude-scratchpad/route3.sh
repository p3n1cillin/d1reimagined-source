S=%USERPROFILE%/AppData/Local/Temp/claude/F--d1-d1xenia/eb49bdd6-2562-42a0-a73d-ed14111dbab8/scratchpad
R=/f/d1/public-work/d1reimagined-source
until grep -q 'in gameplay' $S/cap/boot-route3.log 2>/dev/null; do sleep 5; done; sleep 15
until powershell -NoProfile -File /f/d1/tools/d1-shot.ps1 -Out $S/r3.png >/dev/null && python -I -W ignore $S/lum.py $S/r2.png | awk '{exit !($3>40)}'; do sleep 5; done
cd $R
bash $S/breachwalk.sh r3 2>&1 | tail -4
T=$S/tracked-r3.json
# step off and back on the rifle so the claim fires even if the drop came late
python -I $S/tools/d1-step.py --tracked $T --trail $S/trail-r3.json --keys Hold:S:1200,Hold:W:1300 | tail -1
timeout 300 python -I -u tools/ps4_setup/d1-walk.py --tracked $T --route missions/routes/breach_corridor.json --max-seconds 240 --tolerance 1.5 > $S/walk-corr-r3.log 2>&1; grep -E "done|stopped|fell" $S/walk-corr-r2.log
timeout 1000 python -I -u $S/tools/d1-explore.py --tracked $T --goal 700,-815 --out $S/route-east-r3.json --max-seconds 900 > $S/explore-east-r3.log 2>&1; tail -2 $S/explore-east-r2.log
timeout 1300 python -I -u tools/ps4_setup/d1-walk.py --keepalive 1200
