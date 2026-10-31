S=lab/history/lab-scripts-2026-10-29
cd $S; T=tracked-r8.json; k=0
for goal in 560,-690 420,-600 300,-700; do
  k=$((k+1)); echo "== crouched attempt $k goal $goal"
  timeout 500 python -I -u tools/d1-explore.py --tracked $T --goal $goal --out route-cr$k.json --max-seconds 360 --crouch --stop-on-death --lethal lethal.json > explore-cr$k.log 2>&1
  grep -E "lethal|retracked" explore-cr$k.log | head -2; tail -1 explore-cr$k.log
  if grep -q "marked lethal" explore-cr$k.log; then echo "(died: standing after respawn)"; else powershell -NoProfile -File /f/d1/tools/d1-pad.ps1 -Keys NumPad6 > /dev/null; fi
done
