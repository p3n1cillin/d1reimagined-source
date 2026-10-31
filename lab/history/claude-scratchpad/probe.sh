S=%USERPROFILE%/AppData/Local/Temp/claude/F--d1-d1xenia/eb49bdd6-2562-42a0-a73d-ed14111dbab8/scratchpad
cd $S; T=tracked-r8.json; k=0
python -I -u tools/d1-walk.py --tracked $T --retrack | tail -1
for goal in 560,-690 420,-600 420,-800 300,-700; do
  k=$((k+1)); echo "== probe $k goal $goal"
  timeout 400 python -I -u tools/d1-explore.py --tracked $T --goal $goal --out route-pb$k.json --max-seconds 300 --stop-on-death --lethal lethal.json > explore-pb$k.log 2>&1
  grep -E "lethal|retracked" explore-pb$k.log | head -2; tail -1 explore-pb$k.log
done
