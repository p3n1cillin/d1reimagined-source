#!/bin/bash
# Divide entry -> gate -> garage -> Dock 13 -> hangar bay, fighting with combat2 between legs.
# Progress is read from host events, not step numbers. Usage: chain2.sh RUNNAME
S=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
C=$(python "$S/../../tools/ps4_setup/d1paths.py" staging)
H=$(ls -d $C/contained-boot/*-$1)/activity-host.log
ev() { sed 's/\x1b\[[0-9;]*m//g' $H | grep -q "$1"; }
rel() { python $S/release.py; }                # every tool ends with no key held (a kill mid-press leaves it down)
fight() { timeout $(( $1 + 60 )) python -u $S/combat2.py --seconds $1 ${2:+--squads $2} --log $C/chain2-combat.jsonl 2>&1 | grep -E "lock|bursts|stopped" | tail -1 | sed 's/^/    fight: /'; rel; }
go() { echo "  nav $1: $(timeout 260 python -u $S/nav.py $1 --max-s 200 2>&1 | grep -v shadPS4 | tail -1)"; rel; }
stage() { echo "$(date +%H:%M:%S) $1"; }
stage "start: squads $(python $S/alive.py $H 0x9243dffb)"
for i in $(seq 1 10); do                      # gate fight until the gate squads (17-19) are down
  python $S/alive.py $H 0x9243dffb | grep -qE "17:0.*18:0.*19:0" && break
  # Run sense91: the host still counted one of squad 18 for ten rounds with none left in the world.
  [ $i -gt 2 ] && [ -z "$(python $S/target.py 17,18,19 2>/dev/null | grep -v shadPS4 | tail -1)" ] && { echo "  gate squads counted alive, none in the world"; break; }
  go pre_gate; fight 60; go gate; fight 60
done
stage "gate: $(python $S/alive.py $H 0x9243dffb)"
for i in $(seq 1 6); do ev "now in slice set 104" && break; go corridor; fight 30; done
stage "dock13: $(ev 'player trigger 0x992bb471/0x1d/39' && echo trig39) $(ev 'now in slice set 104' && echo slice104)"
for i in $(seq 1 8); do ev "player trigger 0x992bb471/0x1d/41" && break; go nux; fight 30; go bay; fight 30; done   # clear the NUX doorway first: a Dreg waits there
stage "bay: $(ev 'player trigger 0x992bb471/0x1d/41' && echo trig41)"
Q=5,6,7,8,9,10,11,12                           # the full hangar cohort; 11/12 released by the assignment list at trigger 41
for i in $(seq 1 14); do
  ev "is exactly zero" && break
  a=$(python $S/alive.py $H); echo "  squads: $a"
  t=$(python $S/target.py $Q 2>/dev/null | grep -v shadPS4 | tail -1)
  if [ -n "$t" ]; then echo "  seek $t: $(timeout 150 python -u $S/seek.py $t --max-s 90 --tol 5 --trail $C/seek-trails.jsonl 2>&1 | grep -v shadPS4 | tail -1)"; rel
  else go bay; fi
  fight 60 $Q
done
stage "end: $(python $S/alive.py $H)"
sed 's/\x1b\[[0-9;]*m//g' $H | grep -E "mission step 2[0-9]|HOLD" | tail -3 | cut -c12-27,60-200
