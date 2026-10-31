S=%USERPROFILE%/AppData/Local/Temp/claude/F--d1-d1xenia/eb49bdd6-2562-42a0-a73d-ed14111dbab8/scratchpad
cd $S; T=tracked-r8.json; k=0
replay() {
  python -I -u tools/d1-walk.py --tracked $T --retrack | tail -1
  timeout 900 python -I -u tools/d1-walk.py --tracked $T --route to-chest.json --max-seconds 840 --tolerance 1.3 > rw-chest.log 2>&1
  timeout 120 python -I -u tools/d1-walk.py --tracked $T --route mine-a.json --max-seconds 90 --tolerance 1.2 > rw-a.log 2>&1
  powershell -NoProfile -File /f/d1/tools/d1-pad.ps1 -Keys NumPad6 > /dev/null
  timeout 300 python -I -u tools/d1-walk.py --tracked $T --route mine-b.json --max-seconds 240 --tolerance 1.5 > rw-b.log 2>&1
  powershell -NoProfile -File /f/d1/tools/d1-pad.ps1 -Keys NumPad6 > /dev/null
  timeout 200 python -I -u tools/d1-walk.py --tracked $T --route mine-c.json --max-seconds 150 --tolerance 1.2 > rw-c.log 2>&1
  timeout 400 python -I -u tools/d1-walk.py --tracked $T --route jw-route.json --max-seconds 330 --tolerance 1.3 > rw-jw.log 2>&1
  echo "replay: $(grep -E 'done|stopped' rw-jw.log | tail -1)"
}
for goal in 395,-600 300,-705 300,-640 395,-800 395,-600 300,-705; do
  k=$((k+1))
  echo "== attempt $k goal $goal"
  timeout 500 python -I -u tools/d1-explore.py --tracked $T --goal $goal --out route-r2-$k.json --max-seconds 360 --stop-on-death --lethal lethal.json > explore-r2-$k.log 2>&1
  grep -E "lethal|retracked" explore-r2-$k.log | head -2; tail -1 explore-r2-$k.log
  if grep -q "marked lethal" explore-r2-$k.log; then replay; fi
done
