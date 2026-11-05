# How the live host started The Speaker's cinematic (5 November 2026, Claude)

This reads the private Xbox 360 capture of The Speaker (activity 167; `d1-srcp.py export`, kept outside the repository) message by message. It compares the capture with cine-live8, our host's run in cine_reveal (165). Its context is Bungie's model of auth state and sense state (`d1-gdc2015-activity-hosts.md`) and the start gate in `ps4-56578-cinematic-start-2026-11-05.md`.

## The live host's udt 5 (`sensor_auth_update`) framing

The first 41 bits of every udt 5 body (bytes 0-5, shifted right by 7) are a **tick counter at exactly 10 Hz**:
- 75 at 17.17 s, 105 at 19.86 s, 165 at 26.05 s, 200 at 29.36 s;
- from there it steps 30 per 3 s to 448 at 207.6 s, carrying past 512 into byte 3.

The live host sends three kinds of frame:

| Kind | When | Size |
|---|---|---|
| **Fresh** (stamp field 0, a different leading pattern `00 80`/`00 f8`/`00 b8`) | The join (10.59-10.79 s) and the world load (26.86-27.03 s) | 84-1483 bytes |
| **Idle heartbeat** (stamp only) | Every 3.0 s whenever nothing changed | 6 bytes |
| **Stamped delta** | When records change: 27.16, 27.86, 28.36, 29.36, 29.96 s, and at the end | 38-1442 bytes |

The cinematic's play is a stamped delta, at stamp 200 with start 199 (`0xC7`). This matches the talk: a full (reconcile) frame when the console takes on the bubble, then 10 Hz auth-state deltas.

**Our host sends only fresh frames.** It sends one full, pre-built frame, stamped 0, every 10 s (`D1_AH_SENSOR_REPEAT_S`), with nothing in between. A play value served this way arrives as a fresh frame, not as a delta one tick after the client's last stamp.

## The order of events before the play (live)

| t (s) | Direction | What |
|---|---|---|
| 10.1-19.8 | both | Join; the world tables (udt 11, 1, 2); fresh udt 5 at 10.59 and 10.89; idle heartbeats from 14.18 |
| 26.55-26.73 | c2s | udt 17 (`activity_client_request_state_refresh`), udt 21 (slice set), udt 32 |
| 26.86-27.86 | s2c | Fresh udt 5 (1483 B), then stamped deltas (49, 47, 1442 B); udt 11, 1 and 2 again |
| 27.93, 28.59, 29.29 | c2s | **udt 6 `sensor_sense_update`** ×3: the only sense updates in the capture before the end |
| 29.36 | s2c | Stamped deltas at 198 and **200: the play** |
| 29.36-29.92 | both | udt 18 incidents both ways (the client's start report) |

The host played the cinematic about 70 ms after the client's third sense update, and about 2.5 s after the client asked for a state refresh. The 18 s gap from the join is the client's world load, not a host timer.

## The same in cine-live8 (our host, 165)

- The client asked for a state refresh (udt 17) at 09:57:18.1.
- Our first udt 5 (fresh) went out at 09:57:20.17. The client sent **one** udt 6 at 09:57:20.29, with the same leading pattern as the live one (`8028f0d81c036001700000…`).
- The client sent **no further sense updates in the 20 minutes after that.** Our host sent its fresh frame every 10 s; the play states went out from the watcher (15 s after the join), and none started the cinematic.

## What to change, in order

1. **Send deltas with a 10 Hz stamp.**
   - Keep the fresh frame for the join and the world load.
   - Then send stamped deltas, with a 6-byte heartbeat every 3 s when idle.
   - Send the play as a delta whose start is the current stamp minus one.

   This is the "delta (`fresh = 0`) sensor updates" item in `d1-live-captures-2026-11-05.md`. The client's own encoder in d1-oracle should be able to produce such frames.
2. **Time the play from the client.** Send it once the client has asked for its state refresh and sent sense updates on the world's sensors (in the live run, three within 1.4 s). Don't use a fixed delay after the join.
3. **Check the binding gate.** The client's cinematic binding must already be registered when the play arrives (`0x4E3F70`, `ps4-56578-cinematic-start-2026-11-05.md`). Codex is tracing what registers it.

Inferred, not yet tested: whether the client ignores a play that arrives only in a fresh frame, or whether the binding registers only after a delta. The lab test for both is a host that serves step 1.
