# 01.24 mission continuation — 3 October 2026

Continued from Claude's JOIN88 / `0aca012`. Target remains BLES01857 01.24,
activity 229, checksum `0x8fe07299`. The supplied specifications are behavioral
acceptance references; extracted final-build identities and live protocol evidence
take precedence. See [the acceptance checks](guardian-rises-acceptance.md).

## Concrete repairs

- **Teleport target deltas:** type 21 can name a target before the operator queues
  the host teleport, then omit that unchanged target in later state reports.
  JOIN88's state-4 body did exactly this. The stand-in now retains explicit slice
  fields per connection and uses the retained target to schedule finish state 3.
  Explicit target changes/clears invalidate the match and pending finish timer.
  The cleared `{0,0}` pair is not a Skywatch arrival. Six encrypted-session
  regressions cover this ordering, invalidation, and connection isolation; the
  old handler fails the reproduced JOIN88 case.
- **Host reassignment:** the AHP reply must echo the request's u64 ID. The former
  fixed zero reply was accepted at initial join but rejected after JOIN88's
  request changed to one. The endpoint stays unchanged. Four encrypted-session
  regressions cover nonzero/high-bit/repeated IDs, the existing zero recipe,
  and malformed experimental bodies. This repairs recovery, not by itself the
  initial Breach transition. See [the verified consumer](private-host-investigation.md).
- **Recorded route:** the guarded runner now retains JOIN88's approach and
  narrow-climb points and uses its 1.5-unit arrival tolerance. The old every-third
  selection omitted the turn beside the pit. Its default still stops for loading,
  cleanup, a new slice, rejoin, or a position jump.
  The explicit `--allow-breach-preload` mode permits only the observed engine-3
  request for slice 24/hash `0x68935472` while native and logged current remain
  slice 112. A continuous fresh player hook must identify the same entity.
  Engine 6, unexpected targets, cleanup, rejoin, respawn, or an expired budget
  still stop input. Final-build follow-player code `0x352968` runs in engine
  states up to 3 and sets state 3 through `0x352138` for this area request;
  JOIN88's successful approach retained state 3. JOIN90 validated the guarded
  approach and climb through that state, to within 2.51 units of the endpoint.
  Its final doorway crossing stopped on the native slice reversal described below.
- **Slow startup:** the join wrapper keeps its launcher/input observation alive
  for the advertised eight-minute sign-in window. Previously the inner 250-second
  observation could end the outer wait early.

## Failure diagnosis

Cleanup reason `0x4d` does not uniquely establish an IPAHP initialization wait.
The final-build table maps session states 9, 37 and 38 to it. States 37 and 38
come from separate local transition callbacks pending for 120 seconds. State
38's teleport callback needs incoming host finish state 3. The missing finish
publication in JOIN88 is verified; its exact selecting failure state was not
captured. [The final-build trace](ipahp-timeout-0124.md) includes failure-only
watches and both pending-state paths.

The observed UDP `ENOTCONN` came from an ignored peer-address metadata query on
an unconnected socket. It preceded successful world entry and is not evidence
that a peer packet failed. Incident echo remains off; no new ACK is invented.

## JOIN89: live control and strict route guard

The run used the earlier host handler, before the retained-target repair.
Startup selected the stored character without help. The resurrection area
displayed an Urgent Message; it was dismissed with Cross. Current slice 112
was reported at 16:06:43.306. Sampled outdoor frames were clean, and the character
screen rendered the Titan model without the previous rainbow distortion.
The test profile still shows level zero and incomplete equipment UI; those
progression/profile gaps are not rendering fixes.

Inactive bubble 14 generation 2 and a highway Ghost squad generation were sent
after arrival. The guarded route started from a fresh player hook, walked to
about `(336, -898, 4.3)`, and released all keys when the engine entered state 3
at 16:11:41. It did not complete the recorded route. The client subsequently
requested slice 24 at 16:12:32.628. A manual host teleport was queued shortly
afterwards; current 24 was reported at 16:15:08.355, followed by idle state 0
at 16:15:16.984. This earlier-handler success must not be attributed to the
new target-cache code.

The frame after that early teleport was black without a loading glyph. The
player had not reached the actual entrance; the cause of the black frame is
not established. Protocol arrival alone is not a playable Breach check.
A fresh bubble-3 reset and the correctly named Ghost squad were sent only as
diagnostics; both reset masks retained bit 3, indicating that baseline readiness
had not cleared the waiting mask. No illumination or combat is claimed.
The connection remained open beyond the earlier failure interval. The exact
owned stack was stopped at the end of this diagnostic run, with logs preserved.

Raw screenshots, native watches, control lines, process identities and route
audits remain in the private runtime under `FINAL124_JOIN89*`.

Verification before JOIN90: 137 server tests and 107 scripts tests pass; both
mission scripts encode/decode against the corrected final-build schema, and
the modified PowerShell join wrapper parses without errors. JOIN90 starts a
fresh stack with the repaired host handler and will exercise the opted-in
preload route before its host teleport.

## JOIN90: guarded approach and the final doorway

The repaired handler completed resurrection's 113 -> 112 request: client state 4
at 16:34:44, current 112 at 16:34:47, idle at 16:34:50, and finished request
dropped at 16:34:55. An inactive bubble-14 reset and highway Ghost generation
were sent after arrival. The preload route started at 16:36:01 and retained
all narrow-climb waypoints. The Enter-the-Wall trigger fired at 16:40:02;
Find a Weapon became active at 16:40:12. These labels do not prove illumination
or weapon access.

At 16:40:51 the route stopped with the same player at about
`(351.464, -797.333, 13.929)`, 2.51 units from the recorded endpoint. Native
current had become 24 / `0x68935472`, with target 112 / `0x811c9dc5`, while
the AH log still reported current 112 / target 24. Its report caught up at
16:41:31. No manual host teleport was sent in this run. The guard correctly
caught the actual local change before the network report; the player had
not completed the final target, so this is not a full Breach-entry result.

The client departed at 16:55:50, almost exactly 15 minutes after its last
route input. The error-post watch captured category `0x19`, reason `0x12`,
through `0x35fa80`. The final-build leave-condition-1 path selects this reason
only after its configured in-world elapsed and local-input thresholds pass.
No failure-state-37/38 callback was captured. This departure is an inactivity
timeout, and must not be conflated with JOIN88's reason `0x4d`.

The opted-in route now permits that exact native 24 -> 112 reversal only at
the canonical route's last waypoint, with a fresh unchanged player, in-world
stage 36 / engine 0 or 3, within five units of the endpoint, and for at most
20 seconds. The accepted AH pairs are exactly 112/24 or 24/112. Other changes,
cleared targets, position jumps, respawn, cleanup and rejoin still stop input.
Two new regressions exercise these bounds; all 109 script tests pass.

## JOIN91: repeat malformed shader-cache boot

The next boot stalled before initializing the 01.24 bootflow singleton. Its
RSX worker log contained repeated `Invalid Src type 3` and unexpected precision
messages while precompiling cached fragment programs. The same symptom occurred
in JOIN78, despite normal subsequent live frames. The owned JOIN91 stack was
stopped. The cache's 210,251 files were preserved by renaming only its private
`shaders_cache` directory to `shaders_cache.before-JOIN92-2026-10-03`.

JOIN92 boots again with a fresh shader cache. This recovery does not repair the
cache writer or loader; a bounded cache-validation fix is being investigated.
The SPU colour-buffer changes and required Read Color Buffers remain enabled.

## Remaining acceptance work

Reach the actual Wall entrance during the natural preload, finish the slice
handover, and verify a visible player inside the Breach. Then validate the
correct Ghost/light objects, rifle pickup with persistent inventory, encounters,
mission rewards/results, Tower arrival, and relog. A changed objective label,
squad report, or protocol arrival cannot substitute for these checks.
