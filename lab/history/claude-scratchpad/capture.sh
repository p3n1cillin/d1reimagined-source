S=%USERPROFILE%/AppData/Local/Temp/claude/F--d1-d1xenia/eb49bdd6-2562-42a0-a73d-ed14111dbab8/scratchpad
R=/f/d1/public-work/d1reimagined-source/tools/ps4_setup
TL=$S/cap/timeline.txt; : > $TL
mark() { echo "$(date +%s.%N | cut -c1-14) $*" >> $TL; }
pad() { powershell -NoProfile -File /f/d1/tools/d1-pad.ps1 -Keys "$@" >/dev/null; }
until grep -q 'in gameplay' $S/cap/boot.log 2>/dev/null; do sleep 5; done; sleep 15
until powershell -NoProfile -File /f/d1/tools/d1-shot.ps1 -Out $S/cap/w.png >/dev/null && python -I -W ignore $S/lum.py $S/cap/w.png | awk '{exit !($3>40)}'; do sleep 5; done
rm -f $S/cap/stop
python -I -u $R/d1-record.py --out $S/cap/source.mp4 --seconds 1500 --stop-file $S/cap/stop > $S/cap/record.log 2>&1 &
RP=$!
mark record-start
sleep 4
mark walk-start
bash $S/breachwalk.sh c1 2>&1 | sed "s/^/  /" >> $TL
mark at-rifle
for i in 1 2 3 4 5; do pad Hold:O:700; sleep 1.5; done
mark fired
pad Hold:J:900; for i in 1 2 3 4 5 6 7 8; do pad Hold:O:600 Hold:L:300; sleep 3; done
mark fallen-window-1
for i in $(seq 1 20); do pad Hold:O:500 Hold:J:250; sleep 3; done
mark fallen-window-end
touch $S/cap/stop; wait $RP
mark record-done
cd $R/../..; timeout 1300 python -I -u tools/ps4_setup/d1-walk.py --keepalive 1200
