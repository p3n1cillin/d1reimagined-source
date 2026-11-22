#!/usr/bin/env bash
# Bridge the separately built Linux tools to paths used by the supplied generators.
set -euo pipefail
repo=${D1_REPO_LINUX:?set D1_REPO_LINUX to the WSL path of the workspace folder}
for tree in d1-server d1-trace d1-observe; do
    target="$repo/$tree/target/release"
    mkdir -p "$target"
    if [[ "$tree" == d1-server ]]; then
        names=(d1 d1-activity-host d1-bap d1-eboot d1-forge d1-harness d1-live d1-loadout d1-oracle d1-pkg d1-platform d1-replay d1-responder shadnet-server)
    else
        names=("$tree")
    fi
    for name in "${names[@]}"; do
        source="/opt/d1-target/$tree/release/$name"
        destination="$target/$name"
        [[ -x "$source" ]] || { echo "Missing built Linux tool: $source" >&2; exit 2; }
        if [[ -e "$destination" || -L "$destination" ]]; then
            [[ -L "$destination" && "$(readlink -f "$destination")" == "$source" ]] || {
                echo "Refusing existing unrelated entrypoint: $destination" >&2; exit 2;
            }
        else
            ln -s "$source" "$destination"
        fi
        [[ -x "$destination" ]] || { echo "Entrypoint is not executable: $destination" >&2; exit 2; }
        printf '%s -> %s\n' "$destination" "$source"
    done
done
