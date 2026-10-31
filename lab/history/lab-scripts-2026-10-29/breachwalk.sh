cd /f/d1/public-work/d1reimagined-source
S=lab/history/lab-scripts-2026-10-29
T=$S/tracked-$1.json
for i in 1 2 3; do out=$(timeout 600 python -I -u tools/ps4_setup/d1-walk.py --scan 383,-962,12.5 --radius 25 --out $T 2>&1 | grep tracked | tail -1); echo "scan: $out"; n=$(echo "$out" | awk '{print $1}'); [ "${n:-0}" -ge 100 ] && break; done
timeout 700 python -I -u tools/ps4_setup/d1-walk.py --tracked $T --route missions/routes/breach.json --avoid 330,-837,340,-812 --max-seconds 640 > $S/walk-$1.log 2>&1; grep -E "done|stopped|fell|flipped|calibrated" $S/walk-$1.log
timeout 300 python -I -u tools/ps4_setup/d1-walk.py --tracked $T --route missions/routes/breach_platform.json --max-seconds 200 > $S/walk2-$1.log 2>&1; grep -E "done|stopped|fell|flipped" $S/walk2-$1.log
timeout 60 python -I -u tools/ps4_setup/d1-walk.py --keepalive 30
timeout 200 python -I -u tools/ps4_setup/d1-walk.py --tracked $T --route $S/rifle.json --max-seconds 150 --tolerance 1.5 > $S/walk3-$1.log 2>&1; grep -E "done|stopped|fell" $S/walk3-$1.log
