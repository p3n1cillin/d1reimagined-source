#!/bin/bash
# Walk to the green objective marker (Armsday: Banshee-44): triangulate it, seek it, repeat; a shot after each leg.
S=lab/history/lab-scripts-2026-10-29
K=F:/d1/d1preservation-main/cfw-staging/sense66/banshee2; mkdir -p $K
export D1_ROOT=F:/d1 D1_LAB_DRIVER=1
shot() { (cd $S && python -c "
import importlib.util
s=importlib.util.spec_from_file_location('fight','fight.py');f=importlib.util.module_from_spec(s);s.loader.exec_module(f); f.grab('$K/$1.png')" 2>&1 | grep -v shadPS4); }
touch $S/codex.lock; cd $S; python release.py
for leg in 1 2 3 4 5; do
  T=$(timeout 120 python -u triangulate.py --waypoint --shots $K 2>$K/tri-$leg.err | grep -v shadPS4 | tail -1)
  echo "$(date +%H:%M:%S) leg $leg: target '$T'"
  if echo "$T" | grep -qE '^-?[0-9.]+,-?[0-9.]+$'; then
    echo "  seek: $(timeout 200 python -u seek.py $T --no-nav --max-s 150 --tol 3.0 --trail $K/trail.jsonl 2>&1 | grep -v shadPS4 | tail -1)"
  else
    python -u -c "import sys" ; powershell -NoProfile -File F:/d1/tools/d1-pad.ps1 -Keys "Hold:S:1500" >/dev/null
  fi
  python release.py; shot leg-$leg
done
rm -f $S/codex.lock; echo "$(date +%H:%M:%S) walk done"
