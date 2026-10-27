# Explorer recovery: preserve learned fatal directions

`d1-explore.py --lethal` remembers a fatal move as a grid cell and direction.
Before this change, exhausting the candidate directions cleared every blocked
entry in that cell, including the entries loaded from the lethal file. Recovery
then jumped backwards without consulting the planner. A known death drop could
therefore be tried again despite the saved evidence.

Normal candidate selection now checks the lethal set independently of the
transient wall counters. Dead-end recovery clears only wall evidence, retains
fatal directions and returns to normal direction selection. It no longer makes
an unchecked jump/back-step. A cell with all 16 directions recorded as fatal
stops before keepalive movement or heading calibration and preserves the route
collected so far. Other cells' collision evidence is untouched.

The Windows movement driver loads only after argument parsing. Portable policy
tests and `--help` therefore require neither Windows input nor a running game.
The tests include a full main-loop stop with a fake driver that rejects any
attempt to move. They do not establish a new native route or mission completion.

The existing limits remain: death detection is based on a large tracked-position
jump, not an authoritative death event; short respawns, heading-calibration
probes and tracking correctness require separate native checks. Keep the
validated route separate from new exploratory trails and retain original
private evidence. The tested Wall route still ends at the container-hall
frontier recorded in the 26 October journal.
