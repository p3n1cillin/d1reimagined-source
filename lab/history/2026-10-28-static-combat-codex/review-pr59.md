# Independent static review of PR59 (main728599c)

No host build, runtime replacement, control publish, debugger attach or game input was performed.

1. **Connection isolation is broken (high priority).** `sense.rs::worker` holds a process-wide
   `OnceLock<Option<SenseWorker>>`; `listener.rs::on_client_notification` submits and drains it
   from every connection. `SenseDecoder` documents one decoder per connection. With two connections,
   their matching generation/name keys share overlays, and whichever connection drains first consumes
   the other's `SquadCount`s. A reconnect also inherits the old generation/name images. The worker
   should belong to a connection/activity lifecycle, with a connection epoch on results if necessary.
   Keeping Unicorn wholly inside that worker is correct; it need not be sent across threads.

2. **The cleared gate still succeeds without a cleared report (high priority for an ending).**
   `advance_mission` uses `cleared || elapsed >= timeout` for `OnSquadsCleared`. The existing test
   only exercises reports before the deadline. A blocked/failed/missing decoder will advance once
   `!GROUP|N` expires. `N=14400` postpones this; it does not turn it into a strict kill gate.
   Dock13's outro must remain disabled until a fail-closed/no-timeout form is implemented.

3. **An observed subset is not encounter completeness (high priority for an ending).**
   `squads_cleared` checks all *seen alive* kind1 rows, not all expected current-wave rows.
   Reports17=1 then17=0 suffice even if another required squad was never decoded/spawned. Conversely,
   an earlier wave's alive flag remains sticky, with no generation/revision/connection epoch in the
   `SquadCount`, and can stall a later stage or let a later same-group gate reuse an old clear.
   The correct expected cohort must come from the authored stage and be known before arming the gate.
   Explicitly zero/never-active squads must be distinguished from missing information.

4. **The changed-offset filter is a sensible conservative overlay filter, with limits.**
   `changed_offsets` is the bytewise difference from the prior observation (or zero seed), not a
   native field-presence map. Checking all four count bytes correctly catches256->0, not just1->0.
   A first zero seed remains ignored. However a generation reset followed by a zero report is also
   ignored, and the state machine keeps the prior epoch's positive count. A refused packet leaves
   the old baseline and counts in use; the one logged Breach decode error is relevant here. Typed
   B-tag validation is present, but an explicit `kind==1` guard, nonnegative/range validation and
   schema-conflict rejection would make the contract clearer. `<=0` treats corrupt negative counts
   as dead; the requested semantics say exactly0.

5. **Thread ownership is sound locally, but queue/lifetime handling is unfinished.**
   The non-Send Unicorn decoder is constructed and retained inside the worker. Channels move only
   bytes/counts. `submit` does not wait for decode; draining on subsequent notifications avoids
   decode on the heartbeat path. Both channels are unbounded, though; a slow/failed worker has no
   bounded back-pressure/health/flush epoch contract. Do not drop a delta silently to bound a queue:
   overflow must invalidate that connection's baseline and close the progression gate. No explicit
   shutdown/join exists, and the static sender keeps the worker alive for the process lifetime.

This does not invalidate the measured Vandal death or the value at B+0x0C. It limits promotion of
that observation into an authoritative, multi-connection mission completion gate.
