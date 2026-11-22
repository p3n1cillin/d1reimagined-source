#!/usr/bin/env bash
# BOOTSTRAP.md 3a/3b/4: use existing generators and controls, never substitute a build.
set -euo pipefail
repo=${D1_REPO_LINUX:?set D1_REPO_LINUX to the WSL path of the workspace folder}
export D1_REPO_LINUX="$repo"
cd "$repo"
export D1_REPO="$repo"
export PYTHONPATH="/opt/d1-tools${PYTHONPATH:+:$PYTHONPATH}"
eboot="$repo/dumps/patch-01.29-CUSA00219/eboot.bin"
expected=7271fcb926401df8defb126cb8eb2b247134138b81e401bbacb2ab60791b7954
[[ -f "$eboot" ]] || { echo "REFUSING: missing decrypted CUSA00219 01.29 eboot: $eboot" >&2; exit 2; }
[[ "$(stat -c %s "$eboot")" == 29148264 && "$(sha256sum "$eboot" | cut -d ' ' -f 1)" == "$expected" ]] || {
    echo 'REFUSING: executable does not match the pinned decrypted 01.29 build' >&2; exit 2;
}
packages="$repo/dumps/patch-01.29-CUSA00219/packages"
[[ -d "$packages" && -n "$(find "$packages" -maxdepth 1 -type f -name '*.pkg' -print -quit)" ]] || {
    echo 'REFUSING: installed Bungie content packages are missing' >&2; exit 2;
}
bash "$(dirname "${BASH_SOURCE[0]}")/Install-D1WSLEntrypoints.sh"
python3 -c 'import capstone, unicorn'
mkdir -p capture-rig/manifests capture-rig/schemas
bin="$repo/d1-server/target/release"
"$bin/d1-pkg" controls
"$bin/d1-pkg" manifest -o capture-rig/manifests/full_en.bin
[[ "$(stat -c %s capture-rig/manifests/full_en.bin)" == 52797 &&
   "$(sha256sum capture-rig/manifests/full_en.bin | cut -d ' ' -f 1)" == 0c5bba3c63e9b68021b536513639cfc8896ec736950323e4ab06c5d7ed0770be ]] || {
    echo 'REFUSING: generated manifest differs from the pinned full English manifest' >&2; exit 2;
}
python3 -B capture-rig/oodle/extract-entry.py 0x80A5E37A -o capture-rig/schemas/investment-message-definitions.bin
"$bin/d1-eboot" controls
"$bin/d1-oracle" control
"$bin/d1-activity-host" controls
python3 -B capture-rig/eboot-atlas.py
echo 'Static preparation completed. This is not a native boot or gameplay acceptance.'
echo 'Continue the remaining fixture generators and canonical Windows/WSL launcher work in BOOTSTRAP.md.'
