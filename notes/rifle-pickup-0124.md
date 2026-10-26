# 01.24 Breach rifle: correct object, pickup still open

Updated 2026-10-08, continuation after JOIN205–207 on source `37430bd`.

The rifle cue previously created `o_chest_primary_weapon`, whose authored
placement is about 137 horizontal units away from the rifle waypoint. The first
mission's `o_weapon` matches the waypoint instead. The mission now creates it
alongside `seq_d_o2_80`, 30 seconds after the automatic Ghost-light trigger.
The old chest alias remains available for diagnostics.

| Object | Group / kind / index | Authored position | Unit asset |
| --- | --- | --- | --- |
| Previously selected `o_chest_primary_weapon` | `fc0e45a3 / 4 / 7` | (534.865, −736.800, 16.507) | `80cb82a8` |
| Corrected `o_weapon` | `992bb471 / 4 / 6` | (410.644, −794.385, 17.723) | `80cb828c` |
| Rifle waypoint | `992bb471 / 2c / 78` | (410.780, −794.410, 18.520) | — |

These identities come from the latest disc/update package chains, corrected
relative name pointers and each definition's embedded replicated object ID.
The weapon and marker differ by about 0.14 horizontal units. This correction
does not establish an inventory grant or a visible pickup prompt.

## Native evidence

JOIN208 used the earlier chest selection. An obstructed initial route and a
later position discontinuity prevented a useful rifle test. Its cleanup passed.

JOIN209 used `37430bd` plus the rifle correction, the committed Titan starter
kit and existing C5 emulator. No guest writes, teleport, renderer rebuild or
manual mission-state changes were used. The route crossed naturally into the
Breach. At 19:04:22 UK (18:04:22 UTC), the platform trigger scheduled the automatic
chain; at 19:04:53 UK, the host sent `o_weapon`.

The native creator received unit asset `80cb828c` at the authored rifle position
and returned a non-absent unit datum. A later read-only sample verified its
unit-pool bank, index and occupancy bit, with the row and registry unchanged
during rechecks. The three control/light units also passed these checks. This
establishes allocation, not illumination, usable collision or pickup.

The successful stair approach runs south through (383, −797), (387, −799),
(390, −795), (394, −794.5), then (395, −791.5). Approaching north directly from
the lower corridor hits geometry. From the upper walkway, JOIN209 again reached
the grey barrier near y −791; an ordinary jump did not pass it. Side approaches
are diagnostic exploration, not an accepted route to the rifle. The east attempt
stops near x 417.6 and the south attempt near y −788.8. Neither establishes that
the authored path is inaccessible. JOIN209 closes cooperatively at 18:12:27 UTC;
all nine exact owned identities and three baseline configuration hashes verify
closed/restored, with XInput restored, RPCN preserved and no cleanup errors.

The route helper had a separate close-range defect: its two-unit intermediate
waypoint tolerance could release movement before reaching a requested one-unit
final tolerance. The final waypoint now uses the caller's tighter tolerance.
A slow-pad-poll regression fails before this repair and passes afterwards;
existing arrival, guard and facing checks remain intact. This improves test
precision, not the world's collision geometry.

## Open checks at JOIN209 (superseded by the barrier result below)

The host's object-stream encoder emits State A and always marks optional State B
absent. Kind-4 props have secondary state type `80800387`; its actuation semantics
have not been established. Native creation therefore does not rule out a
mission-state fault in the lamps. Do not send the secondary type in the State A
slot: that changes wire framing.

Next: identify the barrier and authored path or gate actuation, confirm the
visible weapon and interaction prompt, then verify unarmed-to-Khvostov pickup,
ammo, grimoire, objective handover and durable inventory grant. Room illumination,
the black companion Ghost, extra mission banners, Nav Mode and combined
character/world flicker and distortion remain open.

Verification: 198 server tests, 166 script tests and final 01.24 schema preflight
(1 timed / 55 reactive changes) pass. Recordings, extracted package data, unit
handles, probe outputs and production inputs remain private.

## JOIN210–211: barrier opens through the prop activation command

The barrier is opened by State A `+0x10` typed command `0x8080279a` on the Breach control,
door-light and gating-light props, with all three channels at (1, 1, 1.0) (JOIN210, manual).
The mission sends it automatically 2 s after creating those props; JOIN211 then reaches the
rifle without any control lines, trigger `0x1d:35` reports and Fight Through the Wall starts.
A visible weapon, pickup prompt, ammo, grimoire and inventory grant remain unverified; the
fixture still pre-equips a rifle.
## JOIN212–215: no pickup prompt at the chest

With the barrier open, the closed chest renders at the rifle point, but holding Square (Z) from
either side, with the camera tilted down, produces no prompt and no client request; activating
`o_weapon` with the barrier command changes nothing. The pickup is probably a loot object the
host must place there; see [loot and rewards](loot-and-rewards.md). A second Message of the Day
dialog can block input after control (JOIN212); automated drives now dismiss it.
## JOIN216: equipped Khvostov renders and fires, magazine 12

Art record for item 3598: appearance 19, arrangement 2364, dyes (5, 6036) and (6, 6037). Equipped
in slot 6 from login, the rifle renders in first person with an ammo HUD reading 12 (reference
25/184). R1 (key E) fires: one tap takes the magazine from 12 to 11; R2 does nothing in this layout. In the reference the rifle leans on the chest front
and is taken by walking up to it, with no prompt.

## Loot investigation after JOIN216

The self character has a separate 64-slot pending-loot table. The native loot
manager requires both a matching nonce/position cache entry and the current world
ID before it can create the world pickup. The chest prop and a pre-equipped rifle
do not supply this state. The functions previously described as host messaging in
the loot note actually queue local callbacks; the pickup callback then reaches the
service-601 path. See [the corrected loot mapping](loot-and-rewards.md).

The new read-only `rpcs3_final124_loot.py` probe reports these prerequisites using
an exact owned-process manifest and an observed self-character address. Its output
does not claim unit occupancy, visibility or a successful grant. The durable database
also now rejects grant-key reuse across owners and moves into another account's
character. Those repairs are tested independently of native pickup.

## JOIN217: pending records reach the manager; no native pickup

An unarmed starter fixture adds two synthetic pending rifle records, one each
with source kinds 3 and 5. It does not supply an equipped weapon, position cache
or authentic reward nonce. The native `0x2b36f8` and `0x2b3240` entries read both
records. The new probe agrees with these entries at revival, during the Wall
route and after approaching the rifle: two records, no cached nonces, no matching
slot positions and both unit references absent.

Ordinary guarded input reaches `[410.5794, -794.0434, 19.2190]` at
**22:03:04.975 UTC on 8 October**, after the production mission's automatic
Breach prop activation. No manual control-file command or guest-memory write is
used. Watches on the position-recording callbacks and pickup service entry have
no hits; the backend log has no service-601 request. The inspected final view
remains dark and has the Level Up coach; it establishes no graphics repair.

This validates the diagnostic and the missing cache prerequisite for this
synthetic fixture. It does not establish the original rifle nonce, how its action
is created, or a successful grant. The run closes at **22:05:38.946 UTC**;
independent verification confirms all nine exact owned processes closed and all
three baseline configuration hashes restored. Next: recover the action that
records the rifle's nonce/position, then verify the native claim and durable grant.

## JOIN221–226: native pickup and service-601 grant (synthetic event)

JOIN221–223 traced the explicit selector-2 rifle event from queue submission to a
live claim ([incident diagnostics](incident-diagnostics.md#join223-the-rifle-spawns-in-the-breach-and-the-client-claims-it-service-601)).
Two body fields decide it: `0x80800e70`+40 must hold the activity player ID (1),
and the world ID must be the Breach's (3). With both set, the loot manager
allocates a unit for the pending Khvostov record. Walking onto
`(410.644, -794.385)` sends one service-601 request carrying the record's source
kind (3) and nonce.

The stand-in now answers 601 (`bap_server.py`). A claim that matches a pending
self record by source kind and full nonce clears that slot, equips the item with
its art record values (Khvostov: slot 6, appearance 19, arrangement 2364, dyes
5/6036 and 6/6037), and records one durable instance when the player database is
in use (idempotency key per nonce). It then publishes the next self and peer
(family 0) versions. The reply echoes the nonce. Unmatched claims receive the same
reply without a grant.

| Run | Server change | Observed |
| --- | --- | --- |
| JOIN223 | none (default reply) | claim sent once; pending record and unit remain; no grant |
| JOIN224 | grant, self update only | "Khvostov 7G-02" acquisition notification; the client drops the pending record; no weapon or ammo counter visible afterwards |
| JOIN225 | grant, self and peer updates | same claim/grant/update sequence; no recording was produced |
| JOIN226 | grant, self and peer updates | notification, then the rifle **in first person with crosshair and ammo counter**; R1 fires (muzzle flash, holographic sight; counter 12 → 5); Fight Through the Wall is shown |

Fight Through the Wall begins after the rifle trigger, as before. The urgent
"servers will go offline" dialog appears again right after the self update and
must be dismissed. The extra "New Mission: The Taken King: Prelude" banner is also
shown. The magazine is still 12, not 25. Whether the rifle model is visible on the
chest before pickup is not established: it is dark and outside the recorded framing.

Limits: the pending record and the event are still a private synthetic fixture
(`--character-overlay` and the control file), not original reward authority. The
art values are a stand-in table for one item. JOIN224 versus JOIN226 suggests the
peer update is needed for the in-world weapon, but JOIN224 was not inspected at
the same moment. Next: create the pending starter reward on the server, send the
event from the mission script on Breach entry, and start unarmed without private
inputs.

## JOIN227–228: the mission announces the rifle; no private inputs

The mission script now owns the drop. `missions/a_guardian_rises.json` has a
`loot` table (`starter_rifle`: item 3598 at the chest, world 3) and a `loot_drop`
step 30 s after the light-up trigger, alongside `o_weapon`. The activity host
encodes the selector-2 body itself: recipient 1, the drop's world and position,
and a nonce derived from the character ID and loot name. The sign-in stand-in,
given the same mission with `--world-loot`, publishes the matching pending
record until it is claimed (or the item is already owned in the player
database). Neither the private overlay nor the control file is used.

JOIN227 exposed a host crash. The reaction-match log line treated the loot step as
an object change, raised `TypeError` and ended the activity-host connection when the
trigger matched. The log line now describes loot drops. JOIN228 runs the fixed
code: the drop is sent at +30 s, the unit is allocated in the Breach, walking
onto it sends the 601 claim, and the grant is published. The recording shows the
crosshair and ammo counter afterwards; R1 fires (counter 12 → 5). One difference
remains from the private-overlay runs: the client kept the granted pending record
because the update omitted the pending state. The stand-in now publishes the
cleared slots explicitly (unit test only; not yet observed live). JOIN227 closes at
09:49:19.8 UTC and JOIN228 at 10:08:16.8 UTC on 9 October, each with independently
verified closure.