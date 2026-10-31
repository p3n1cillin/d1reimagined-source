#!/bin/bash
# look.sh NAME [d1-step args]: step (optional turn/walk), then a frame dump view at S/NAME.png
S=%USERPROFILE%/AppData/Local/Temp/claude/F--d1-d1xenia/eb49bdd6-2562-42a0-a73d-ed14111dbab8/scratchpad
D=/f/d1/tools/shadPS4/dump-live
n=$1; shift
cd $S
python -I tools/d1-step.py --tracked ${LT:-tracked-r10.json} --trail trail-look9.json "$@" 2>&1 | tail -1
find /f/d1/tools/shadPS4/dump-live -maxdepth 1 -name '*.raw' -delete
touch $D.flag; sleep 4; rm -f $D.flag; sleep 1
python -I $S/scene.py $D $S/$n.png | tail -1
