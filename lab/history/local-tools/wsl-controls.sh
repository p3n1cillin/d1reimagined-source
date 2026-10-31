#!/bin/bash
cd /mnt/f/d1/d1preservation-main
B=/opt/d1-target/d1-server/release
for t in "d1-responder controls" "shadnet-server controls" "d1 controls"; do
  $B/$t > /tmp/c.log 2>&1; rc=$?
  echo "$t: exit $rc | $(grep -ciE 'pass' /tmp/c.log) pass lines, $(grep -cE 'FAIL' /tmp/c.log) fail lines"
  grep -E '\[ FAIL|^FAIL' /tmp/c.log | head -2
done
