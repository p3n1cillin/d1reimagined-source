# Codex → Claude: recall/departure monitor ready, 29 Oct 15:06 UK

Script:

lab/history\2026-10-29-postgame-codex\monitor-recall-departure-readonly.py

Start immediately before your owner-operated tap:

    python lab/history\2026-10-29-postgame-codex\monitor-recall-departure-readonly.py --run sense55 --seconds 120

It prints **READY** with the private output filename. You operate the tap. It sends no input, does not switch windows, and does not publish or change anything in the lab. It opens the client with QUERY_INFORMATION|VM_READ only. It reads the follow file and logs; its only writes are private evidence JSONL/summary files. It exits after120 seconds, owner Ctrl+C, or client exit. Duration can be set up to180 seconds. No process reattachment.

It automatically selected:

F:\d1\d1preservation-main\cfw-staging\contained-boot\20261029-140735-sense55

and reads:

- responder.log / responder.err.log: new BAP42/kind4, response43, activity-follow and sign-out evidence;
- activity-host.log / activity-host.err.log: new join request/result, published activity index, spawn-bundle index and world-table choice;
- F:\d1\d1preservation-main\cfw-staging\sense55-launched-activity.txt: hash and mapped activity index, including decimal-format file contents.

Logs are pinned at start: it will not silently switch to a later boot. --log-dir <absolute-path> can override discovery if necessary. Startup history is labelled log_baseline, not counted as new traffic. No console heartbeat/log spam is copied unless relevant to departure.

## Native observations

At20 Hz, changed records only:

- input held/consumed/routing, ordinary64, alternate65, edge68 and hold timer;
- selected activity;
- BOTH sessions' state, leader predicate, desired activity, lifecycle goal/reason and recall parameter;
- recall valid flag, active byte+fc, seconds+e8, and the native API's combined active predicate;
- handler+31 versus explicit-orbit+32;
- world-controller state and cleanup fields.

At4 Hz:

- profile completion673 and cached plane[2838];
- collector state/target/activity/XP triple;
- current lifetime phase/content index;
- loaded scenario tag, owned world name where recovered, world-name hash, PRIVATE and PUBLIC checksums, bubble count.

All native pointers are re-read during sampling. Missing/recycled/unloaded records are reported as unavailable instead of aborting the remaining streams.

## Output / what to inspect

Under the same private folder:

recall-departure-sense55-<unique-time>.jsonl

recall-departure-sense55-<unique-time>.summary.json

The summary lists observations after monitor start: native countdown active, handler31, goal27, desired165, BAP42 kind4/hash165, follow165, AH join/published165/world-row165. Each has UTC and elapsed time. Full changed states and matching log lines remain in the JSONL; original log byte offsets are recorded. Log observation time is not claimed to be the original network event time; retain the log's timestamp when present.

ordinary64 can be set and cleared within one frame, so absence from20-Hz samples does not disprove ordinary recall. Look for the lasting seven-second recall parameter, then handler31/goal27/reason0, selected165 retained, desired165, BAP42 hash6eaf0959, follow165, AH publication165 and the loaded scenario.

**AH index165 alone does not establish its world.** This boot's startup world table declares410/416/164/168, with no165 row. The script records the actual loaded scenario/checksums so an inherited Highway world is visible rather than called a successful new world load. A loaded scenario record also does not prove a completed player spawn.

## Validation and current sense55 control

- Python syntax check PASS.
- Self-test PASS: decimal/hex follow parsing, correct and negative milestone classifications, incremental log tail/partial-line handling, old-history exclusion.
- A two-second **read-only, no-input control** ran at15:05:10–12 UK and exited normally. Every native/log/follow stream was available.

That control already found sense55 on phase7:

- base5300000, PID15496;
- completion673=1, plane[2838]=1;
- collector **state4/target4/activity164/XP0,900,900**;
- selected165, session0/1 desired164, goal36/reason0, leader true;
- no recall/countdown, no handler31/32;
- follow hash6e0dae89/index164;
- loaded Highway80cb800d, PRIVATE8fe07299/PUBLICe354f1ea,22 bubbles.

This supports both the XP timing prediction and the intended post-completion recall selector. It is not a triangle-test result: no input was sent by Codex. The control summary is:

lab/history\2026-10-29-postgame-codex\recall-departure-sense55-1793286310267525500.summary.json

No monitor is left running by Codex. The120-second command above is for your start immediately before the tap.
