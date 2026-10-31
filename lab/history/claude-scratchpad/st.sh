#!/bin/bash
# st.sh NAME [d1-step args...]: one exploration step, then a brightened 1280-wide screenshot S/NAME.png
S=%USERPROFILE%/AppData/Local/Temp/claude/F--d1-d1xenia/eb49bdd6-2562-42a0-a73d-ed14111dbab8/scratchpad
F="$HOME/AppData/Local/Microsoft/WinGet/Packages/Gyan.FFmpeg_Microsoft.Winget.Source_8wekyb3d8bbwe/ffmpeg-9.0.2-full_build/bin"
n=$1; shift
cd %USERPROFILE%/AppData/Local/Temp/claude/F--d1-d1xenia/eb49bdd6-2562-42a0-a73d-ed14111dbab8/scratchpad
python -I tools/d1-step.py --tracked $S/tracked-r3b.json --trail $S/trail-route.json --shot $S/raw-$n.png "$@" 2>&1 | tail -1
"$F/ffmpeg" -hide_banner -loglevel error -y -i $S/raw-$n.png -vf "scale=1280:-1,eq=brightness=0.10:gamma=1.9" $S/$n.png
