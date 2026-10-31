#!/bin/bash
# Full Linux build of the d1preservation Rust trees (source on F:, output in /opt/d1-target).
. /etc/profile.d/rust.sh
set -o pipefail
R=/mnt/f/d1/d1preservation-main
for t in d1-server d1-trace d1-observe; do
  echo "=== $t $(date -u +%T)"
  (cd $R/$t && CARGO_TARGET_DIR=/opt/d1-target/$t cargo build --release 2>&1 | tail -3)
  echo "exit ${PIPESTATUS[0]}"
done
python3 -m pip install --break-system-packages --target /opt/d1-tools capstone unicorn 2>&1 | tail -1
ls /opt/d1-target/*/release/ | grep -v '\.d$' | grep -vE '^(build|deps|examples|incremental)$'
