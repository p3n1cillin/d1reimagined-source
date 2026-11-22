#!/usr/bin/env bash
# Run the existing shadNet server and d1-responder in WSL for a Windows shadPS4 client, bound to
# 127.0.0.1 ONLY (mirrored networking makes WSL loopback reachable from Windows loopback; binding
# 0.0.0.0 would expose the listeners on every mirrored LAN interface). Root is needed for the
# responder's :80/:443. Runs in the foreground until <out>/stop exists, then stops only the
# children it started and records that they are gone.
#
#   wsl -d Ubuntu-24.04 -u root -- bash wsl-servers-loopback.sh <linux-out-dir> <manifest> [bind-ip]
#
# bind-ip defaults to 127.0.0.1. With the isolated "D1 Lab" adapter (d1-lab-net.cmd) pass
# 10.77.0.2: the game rejects a loopback-only transport (marionberry), and that virtual adapter
# has no route off the PC. Any other address is refused. shadNet stays on loopback: it is the
# emulator's own NP connection, not a guest socket.
set -euo pipefail
OUT="${1:?usage: wsl-servers-loopback.sh <out-dir> <manifest> [bind-ip]}"
MANIFEST="${2:?manifest path required}"
BIND="${3:-127.0.0.1}"
[[ "$BIND" == 127.0.0.1 || "$BIND" == 10.77.0.2 ]] || { echo "REFUSING: bind address must be 127.0.0.1 or the lab 10.77.0.2" >&2; exit 2; }
if [[ "$BIND" != 127.0.0.1 ]] && ! ip -4 -o addr show | grep -q " $BIND/"; then
    echo "REFUSING: $BIND is not present in WSL (apply d1-lab-net.cmd; mirrored networking required)" >&2; exit 2
fi
BIN="${D1_SERVER_BIN:-/opt/d1-target/d1-server/release}"
REPO="${D1_REPO_LINUX:?set D1_REPO_LINUX to the WSL path of the workspace folder}"
[[ -d "$OUT" && ! -e "$OUT/servers.pids" ]] || { echo "REFUSING: missing or reused output $OUT" >&2; exit 2; }
for f in "$BIN/shadnet-server" "$BIN/d1-responder" "$MANIFEST"; do
    [[ -e "$f" ]] || { echo "missing: $f" >&2; exit 1; }
done
for port in 80 443 8080 8788 31313 31315 32556; do
    if ss -Hltn "sport = :$port" | grep -q .; then echo "REFUSING: TCP $port already in use" >&2; exit 2; fi
done

openssl req -x509 -newkey rsa:2048 -nodes -keyout "$OUT/tls.key" -out "$OUT/tls.crt" -days 2 \
    -subj "/CN=signon.deadorbit.net" \
    -addext "subjectAltName=DNS:signon.deadorbit.net,DNS:signon.gravityshavings.net,DNS:www1.signon.deadorbit.net,DNS:www1.signon.gravityshavings.net" \
    >/dev/null 2>&1

# The canonical responder knobs from capture-rig/run-boot.sh (recorded 01.29 plan contract),
# with the transport addresses pointed at loopback for the Windows client.
export D1_ACCOUNT_SOID="${D1_ACCOUNT_SOID:-0xD100000000000001}"
export D1_CHARACTER_SOID="${D1_CHARACTER_SOID:-0xD100000000000101}"
export D1_503_SOID="${D1_503_SOID:-1}"
export D1_CHARACTER_LIFECYCLE="${D1_CHARACTER_LIFECYCLE:-1}"
export D1_PROFILE_FILE="${D1_PROFILE_FILE-$REPO/capture-rig/profiles/profile-old-russia.bin}"
export D1_PUSH_CHARACTER="${D1_PUSH_CHARACTER:-1}" D1_PUSH_FAMILY="${D1_PUSH_FAMILY:-4,3}" D1_PUSH_AT="${D1_PUSH_AT:-echo}"
export D1_ACTIVITY_HOST=0 D1_IP="${D1_IP:-$BIND}" D1_BAP_IP="${D1_BAP_IP:-$BIND}" D1_CLIENT_IP="${D1_CLIENT_IP:-$BIND}"
export D1_MANIFEST_FILE="$MANIFEST" D1_UPLOAD_DIR="$OUT/uploads"
export D1_CHARACTER_TRANSCRIPT_DIR="$OUT/character-transcripts"
env | grep -E '^D1_' | sort > "$OUT/server-environment.txt"

OWNED=()
stop_owned() {
    local pid
    for pid in "${OWNED[@]}"; do kill "$pid" 2>/dev/null || true; done
    for _ in $(seq 1 30); do
        local alive=0
        for pid in "${OWNED[@]}"; do kill -0 "$pid" 2>/dev/null && alive=1; done
        [[ "$alive" == 0 ]] && break
        sleep 0.1
    done
    for pid in "${OWNED[@]}"; do kill -9 "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true; done
    for pid in "${OWNED[@]}"; do
        if kill -0 "$pid" 2>/dev/null; then echo "child $pid still alive" >> "$OUT/servers-stopped.txt"; fi
    done
    echo "stopped $(date -u +%FT%TZ): ${OWNED[*]}" >> "$OUT/servers-stopped.txt"
}
trap stop_owned EXIT
trap "exit 130" INT
trap "exit 143" TERM

"$BIN/shadnet-server" --bind 127.0.0.1:31313 > "$OUT/shadnet.log" 2>&1 &
OWNED+=("$!")
"$BIN/d1-responder" --bind "$BIND" --cert "$OUT/tls.crt" --key "$OUT/tls.key" \
    > "$OUT/responder.log" 2>&1 &
OWNED+=("$!")
echo "${OWNED[*]}" > "$OUT/servers.pids"

ok=0
for _ in $(seq 1 100); do
    if ss -Hltn "sport = :31313" | grep -q . && ss -Hltn "src $BIND and sport = :443" | grep -q .; then ok=1; break; fi
    sleep 0.1
done
ss -Hltnu > "$OUT/listeners.txt" 2>&1 || true
if [[ "$ok" != 1 ]]; then echo "servers did not start listening" >&2; exit 1; fi
# Every server listener must be on loopback or the isolated lab address.
BIND_RE="${BIND//./[.]}"
if awk '{print $5}' "$OUT/listeners.txt" | grep -vE "^(127\.0\.0\.1|\[::1\]|127\.0\.0\.53%lo|127\.0\.0\.54|$BIND_RE):" \
        | grep -E ':(80|443|8080|8788|3075|31313|31315|32556)$' > "$OUT/nonloopback.txt"; then
    echo "REFUSING: a server listener is not loopback-only" >&2; exit 2
fi
echo ready > "$OUT/servers.ready"
while [[ ! -e "$OUT/stop" ]]; do
    for pid in "${OWNED[@]}"; do kill -0 "$pid" 2>/dev/null || { echo "server $pid exited" >&2; exit 1; }; done
    sleep 1
done
