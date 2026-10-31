S=%USERPROFILE%/AppData/Local/Temp/claude/F--d1-d1xenia/eb49bdd6-2562-42a0-a73d-ed14111dbab8/scratchpad
cd $S
k=0
for goal in 545,-650 650,-700 700,-830 600,-950 520,-950 430,-900 545,-650 650,-700 700,-830 600,-950 520,-950 430,-900; do
  k=$((k+1))
  python -I -u tools/d1-walk.py --tracked tracked-r4.json --retrack | tail -1
  timeout 500 python -I -u tools/d1-walk.py --tracked tracked-r4.json --route to-hall.json --max-seconds 420 --tolerance 1.3 > walk-rot$k.log 2>&1
  echo "attempt $k goal $goal: $(grep -E 'done|stopped' walk-rot$k.log | tail -1)"
  timeout 400 python -I -u tools/d1-explore.py --tracked tracked-r4.json --goal $goal --out route-rot$k.json --max-seconds 300 --stop-on-death --lethal lethal.json > explore-rot$k.log 2>&1
  grep -E "lethal|retracked" explore-rot$k.log | head -2
  tail -1 explore-rot$k.log
done
