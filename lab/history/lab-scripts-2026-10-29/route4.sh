S=lab/history/lab-scripts-2026-10-29
R=/f/d1/public-work/d1reimagined-source
until grep -q 'in gameplay' $S/cap/boot-route4.log 2>/dev/null; do sleep 5; done; sleep 15
until powershell -NoProfile -File /f/d1/tools/d1-shot.ps1 -Out $S/r4.png >/dev/null && python -I -W ignore $S/lum.py $S/r4.png | awk '{exit !($3>40)}'; do sleep 5; done
cd $R
bash $S/breachwalk.sh r4 2>&1 | tail -4
T=$S/tracked-r4.json
python -I $S/tools/d1-step.py --tracked $T --trail $S/trail-r4.json --keys Hold:S:1200,Hold:W:1300 | tail -1
python -I -u $S/tools/d1-walk.py --tracked $T --retrack | tail -1
timeout 400 python -I -u $S/tools/d1-walk.py --tracked $T --route $S/to-exitsign.json --max-seconds 360 --tolerance 1.5 > $S/walk-ftw-r4.log 2>&1; grep -E "done|stopped|fell" $S/walk-ftw-r4.log
timeout 800 python -I -u $S/tools/d1-explore.py --tracked $T --goal 485,-900 --out $S/route-s-r4.json --max-seconds 720 > $S/explore-s-r4.log 2>&1; tail -2 $S/explore-s-r4.log
timeout 2000 python -I -u $S/tools/d1-walk.py --keepalive 1800
