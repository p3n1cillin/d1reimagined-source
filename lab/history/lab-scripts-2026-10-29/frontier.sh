# frontier.sh GOALX,GOALY ATTEMPTS: replay to the hall, explore until death or timeout, repeat
S=lab/history/lab-scripts-2026-10-29
cd $S
for n in $(seq 1 $2); do
  python -I -u tools/d1-walk.py --tracked tracked-r4.json --retrack | tail -1
  timeout 500 python -I -u tools/d1-walk.py --tracked tracked-r4.json --route to-hall2.json --max-seconds 420 --tolerance 1.3 > walk-fr$n.log 2>&1
  echo "attempt $n: $(grep -E 'done|stopped' walk-fr$n.log | tail -1)"
  (timeout 400 python -I -u tools/d1-explore.py --tracked tracked-r4.json --goal $1 --out route-fr$n.json --max-seconds 300 --stop-on-death --lethal lethal.json > explore-fr$n.log 2>&1 &)
  sleep 5
  while tasklist | grep -q python; do m=$(tasklist | grep python | awk '{gsub(",","",$5); print $5}' | sort -n | tail -1); [ "${m:-0}" -gt 3000000 ] && { powershell -NoProfile -c "Get-Process python | Stop-Process -Force"; echo KILLED-MEM; }; sleep 15; done
  grep -E "lethal|retracked" explore-fr$n.log | head -3
  tail -1 explore-fr$n.log
done
