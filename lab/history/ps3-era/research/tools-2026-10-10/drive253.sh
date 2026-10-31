L=/f/d1/<pc>-import/d1xenia/local
S="%USERPROFILE%/AppData/Local/Temp/claude/F--d1-d1xenia/b998a02a-df96-4125-8e75-a935101b3d81/scratchpad"
E="$S/explore253.py"
C=$L/SIGNON_LOCAL/FINAL124_JOIN253-control.jsonl
AHL=$L/SIGNON_LOCAL/logs/FINAL124_JOIN253-ahl.log
loot() { (cd /f/d1/d1xenia && python scripts/rpcs3_final124_loot.py --process-manifest "F:\d1\<pc>-import\d1xenia\local\SIGNON_LOCAL\logs\FINAL124_JOIN253-processes.json" --self-character $1 --character-id 0x0a3d4a2c8fbea6ba --out "F:\d1\<pc>-import\d1xenia\local\RESEARCH\JOIN253-loot-$2.json" >/dev/null && python -c "
import json,sys; d=json.load(open(sys.argv[1]))
print('loot', sys.argv[2], 'cache', d['cache_count'], [(c['nonce_id'], c['world_id']) for c in d['cache']], [(r['item_index'], r['unit_reference'], r['position_matches_current_world']) for r in d['records']])" "F:\d1\<pc>-import\d1xenia\local\RESEARCH\JOIN253-loot-$2.json" $2) || echo "loot $2 read failed"; }
until [ -s $L/RESEARCH/JOIN253-session-timeline.jsonl ]; do sleep 2; [ -s $L/RESEARCH/JOIN253-client-exit.json ] && { echo CLIENT-EXIT; exit 1; }; done
echo "ready $(date -u +%T)"
sleep 20; STARTED=1; pwsh -NoProfile -File "$S/rec.ps1" -Run JOIN253 2>&1 | tail -1
sleep 20
pwsh -NoProfile -File "$S/key253.ps1" -Keys X -HoldMs 800 | tail -1
n=0; until grep -aq "zone set 112 reached" $AHL; do sleep 2; n=$((n+1)); [ -s $L/RESEARCH/JOIN253-client-exit.json ] && { echo CLIENT-EXIT; exit 1; }; if [ $((n % 30)) = 0 ] && [ $n -le 90 ]; then echo "no 112 yet, X $(date -u +%T)"; pwsh -NoProfile -File "$S/key253.ps1" -Keys X -HoldMs 800 | tail -1; fi; done
echo "112 $(date -u +%T)"
sleep 10
pwsh -NoProfile -File "$S/key253.ps1" -Keys X -HoldMs 800 | tail -1
sleep 5
cd /f/d1/d1xenia && python scripts/d1_guardian_route.py --manifest $L/SIGNON_LOCAL/logs/FINAL124_JOIN253-processes.json --out-dir $L/RESEARCH/JOIN253-guardian-route --allow-breach-preload --continue-breach --wait 60 --budget 300 --final-radius 0.25 > $L/RESEARCH/JOIN253-route.out 2>&1
echo "route exit $? $(date -u +%T)"
sleep 25
python "$E" 150 1.0 365,-796 378,-793 383,-797 387,-799 390,-795 394,-794.5 395,-791.5 2>&1 | tail -1 | cut -c1-120
until grep -aq "object 0xfc0e45a3:0x1d:120 -> {0: True" $AHL; do sleep 1; done
sleep 2
python "$E" 30 0.8 392.5,-790 2>&1 | tail -1 | cut -c1-120
echo "chain $(date -u +%T)"
sleep 32
python "$E" 90 0.8 394.5,-787 402,-786 406,-788 409,-791 face:410.8,-797 2>&1 | tail -1 | cut -c1-200
echo "at wall $(date -u +%T)"
grep -a "loot drop" $AHL | tail -2 | cut -c1-200
sleep 8
echo "pickup $(date -u +%T)"
sleep 2
python "$E" 25 0.35 410.644,-794.385 2>&1 | tail -1 | cut -c1-300
echo "rifle walk $(date -u +%T)"
sleep 2; pwsh -NoProfile -File "$S/key253.ps1" -Keys X -HoldMs 800 | tail -1; sleep 3; pwsh -NoProfile -File "$S/key253.ps1" -Keys X -HoldMs 800 | tail -1; sleep 2
python "$E" 40 0.6 410.6,-794.4 409.8,-797.5 2>&1 | tail -1 | cut -c1-160
sleep 3
grep -aE "loot claim|after-601|service 601" $L/SIGNON_LOCAL/logs/FINAL124_JOIN253-bap.log | tail -6 | cut -c1-220
grep -aE "0x992bb471:0x1d:35|0x992bb471:0x20:14" $AHL | tail -3 | cut -c1-170
sleep 4
pwsh -NoProfile -File "$S/key253.ps1" -Keys X -HoldMs 800 | tail -1
sleep 3
pwsh -NoProfile -File "$S/key253.ps1" -Keys X -HoldMs 800 | tail -1
sleep 3
python "$E" 10 0.6 face:403,-786 2>&1 | tail -1 | cut -c1-80
sleep 3
echo "fire $(date -u +%T)"
pwsh -NoProfile -File "$S/key253.ps1" -Keys E -HoldMs 300 | tail -1
sleep 3
pwsh -NoProfile -File "$S/key253.ps1" -Keys E -HoldMs 1200 | tail -1
sleep 6
pwsh -NoProfile -File "$S/snap.ps1" "F:\d1\<pc>-import\d1xenia\local\RPCS3_FINAL124_SMOKE001\log\RPCS3.log" "F:\d1\<pc>-import\d1xenia\local\RESEARCH\JOIN253-after-grant-native.log"
[ -n "$STARTED" ] && pwsh -NoProfile -File "$S/rec.ps1" -Run JOIN253 2>&1 | tail -1
echo "done $(date -u +%T)"
touch /f/d1/<pc>-import/d1xenia/local/SIGNON_LOCAL/logs/JOIN253-finish.on; echo finish
