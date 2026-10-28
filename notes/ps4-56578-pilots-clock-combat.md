# PS4 56578: pilot programs, activity clock and combat inputs

28 October 2026. Addresses below are executable-relative and specific to the
owned 56578 image (`672f0341…`). This is static analysis plus bounded read-only
observations and offline execution. No new flight, cargo drop, super fix or
mission ending has been observed by this investigation.

## Divide dropships

Group `0x9243dffb` has these authored pairs:

| Dropship squad (kind 1) | Pilot (kind 2) | Pilot definition |
| --- | --- | --- |
| 4, dropship 1 | 5 | `0x80cb8425` |
| 6, dropship 2 | 7 | `0x80cb8426` |
| 8, dropship 3 | 9 | `0x80cb8427` |
| 10, dropship 4 | 11 | `0x80cb8428` |
| 12, tank carrier | 13 | `0x80cb8429` |

Flight paths are **kind `0x38`**, indices 76–84. Drop2 entry is 78
(`0x80cb841b`); exit is 79 (`0x80cb841c`). Their marker times are respectively
`[0,20]` and `[0,9]` seconds. `0x10a690` explicitly resolves a kind-`0x38`
reference and returns the path definition's component offset plus `0x68`.
`0x167540` reads a marker time from that path. The byte in a move payload names
a marker index, not a path duration.

Pilot 7's instantiated definition offset is `0xb38`; its `+0x68` names squad 6.
Using the first template at entry offset `0x70` instead gives an absent reference.
Follow the runtime component's `{definition tag, definition offset}` pair.

At 11:47 UK, all five pilots had default A state: generation 0, enabled false,
program revision/count 0 and entity handle `-1`. The existing squad activation
therefore had not activated these named pilots. A valid program also requires
an admitted entity; a syntactically valid queue alone cannot fly an unbound pilot.

The group's kind-`0x25` object 33, `skiff_passenger` (`0x80cb8435`), names
**pilot 13, the tank carrier**, and socket hash `0x4b3bd988`. It is not evidence
for attaching the drop2 ground squads 15/16 through that object. Their authored
unload/channel/behaviour sequence remains open.

### Native member A program

Kind 2 A is `0x80800868`, 2312 bytes. The existing member helper produces an
activation image with no program. Native consumers establish:

| A offset | Meaning |
| --- | --- |
| `0x00` | activation generation |
| `0x06` | enabled |
| `0xb0` | program revision |
| `0xb4` | resume command index |
| `0xb8` | command count, at most 32 |
| `0xc0` | up to 32 commands, 64 bytes each |

Each command has a primary selector byte at `+0`, secondary selector at `+4`,
secondary payload at `+8`, primary payload at `+0x10`. Factory `0x1684a0`
dispatches primary 0 to snap and 3 to move. Snap payload `0x80800547` contains
a name reference and marker; move payload `0x8080040b` adds a flag.

`0x1be5c0` reloads the program when its revision changes, subject to enabled,
admission and entity checks. `0x1bd990` continues the program at the current
command. The diagnostic departure candidate activates pilot 7, snaps to exit2
marker 0 and moves to marker 1. It does not claim the retail passenger-drop
sequence, end-of-flight retirement or an observed departure.

State handles resolve to wrappers. `0x2cfcd0` obtains the actual A image from
the wrapper's lifetime index `+0x40` and buffer offset `+0x38`. Reading the
wrapper as the image produces invalid counts and must be rejected.

### Offline codec support

Nonempty native-atom queues need additional startup payload type cells.
`0x1c0630` and `0x1c0740` select payload schemas using lazily initialized tables.
The oracle seeds the independently measured cells and exactly the tables their
native initializers construct. It does not stub a null function or relax the
stack-memory guard.

One small payload descriptor (`0x80800453`) has an accidental second match to
the registry relative-link relation. The registry may use its independently
measured startup descriptor object only after that object satisfies the same
anchor relation. Unmeasured or ambiguous records still fail closed.

The synthetic snap/move control round-trips all 2312 bytes: 689 ordinary bits
and 690 replicated bits. Changing the marker changes the wire and survives the
decode. These are codec controls, not flight tests. Rebuild the host against
this oracle support before trying a nonempty pilot queue. Do not replace a host
binary during its run.

## Trigger arming uses activity simulation ticks

The native 56578 trigger state is still `{armed bool, arming tick i32}`.
Initialization `0x12a6d0` sets component last-report tick `+0x1ac` to `-1`.
Update `0x12a800` reports only when enabled and last-report tick is less than
arming tick, then evaluates the authored player/volume filters. Reporter
`0x12ac70` writes `0x108730(component)` back to last-report tick.

`0x108730` resolves the object's activity world and calls `0x2a2ec0`.
That getter converts the activity simulation time using the configured tick
period. This is separate from the global activity blob's `+0x520` millisecond
base, exposed as `D1_AH_TIME_BASE_MS`. Changing that base does not set the trigger
clock's tick period.

At 12:04 and 12:09 UK the live activity clock had **period 0, sensor stamp 0**.
Trigger 40 had last-report tick `INT_MIN`; trigger 41, not yet reported, had
`-1`. The zero-period conversion provides a concrete cause for a trigger to
keep satisfying the signed comparison. Serving arm time 0 instead of 1 is not
a complete remedy while that conversion is invalid. Disarming after the first
report stops the flood but does not repair the clock.

The missing configuration has a native delivery path:

1. AH udt **2**, `world_globals_state`: dispatch `0x1018d00`, schema cell
   `0x191a418` -> tag `0x80800dc1`, 8-byte decoded body.
2. Schema child `0x80800d68`: bool at `+0`, float at `+4`.
3. Game-interface queue `0x2409e0` -> `0xbb2660` -> `0x2a1f40` -> `0x2a2ca0`.
4. The setter copies the 8-byte configuration to activity world `+0x14`, so
   the float becomes tick period at world `+0x18`, and sets config-present `+0x10`.

The engine's own tick-info accessor `0x591bf0` reads 30 ticks/s in the current
run. `1000/30` milliseconds is therefore a content-independent diagnostic period
derived from the client's engine configuration. The native schema and setter
accept that body offline. A contained delivery and clock/readback comparison is
still required; no new world-globals message was sent in this investigation.

**Subsequent implementation (Claude, PR #61):** the host now offers
`D1_AH_WORLD_PERIOD_MS`, disabled by default. With `33.333`, Claude reports five
world-globals pushes in sense38, a native period of 33.333 and an advancing
activity time. Single-report trigger arming remains to be checked on a fresh run.
This supersedes the missing-delivery status above; no additional send was made
by this investigation.

The sensor encoder also currently writes stamp 0. A durable clock implementation
must advance that stamp in simulation ticks and stamp each arming in the same
clock, with `last_report < arm_tick <= report_time`. Do not stamp it with wall
clock milliseconds, a future sentinel or a Unix epoch. Changing clock semantics
also requires checking the existing dialogue/sequence start times.

## Dock 13 objective identities

Owned 56578 string index `0x80cb97a1` and its English payload `0x80cc0a62`
settle the previously provisional labels:

| Activity objective | Title | Title hash |
| --- | --- | --- |
| 20 | Find the Ship | `0x2081ac17` |
| 23 | Find the Ship | `0x07d48724` |
| 26 | Clear out the Fallen | `0x155f7e1f` |

The objective descriptions agree. The earlier combined label “23,26 = Clear
Out” was incorrect. Authored marker associations are 20 -> 57, 23 -> 60,
26 -> 52; marker 57 has a placeholder position. The tested 20 -> 52 pairing
remains a provisional navigation choice, not an authored association.

Dock group `0x062dd95d` is entry `0x80cb868a` on this build. Definition metadata
identifies 19 clients with A schemas, including props, eight squads, the
objective, the filter and four triggers. This is not a replacement for a fresh
pending-client capture. Authority must follow the arriving client: Claude's
12:08 update reports arrival in slice 104 at 12:02:18, machine **13**, all
pending clients served, trigger 40 reported and objective 23 presented. It also
reports a first Dock kill in squad 9 at 12:06:11. These are Claude's observations,
not additional native tests by this investigation.

The private drafts stop before automatic mission completion. `!GROUP|N`
currently has a timeout fallback and tracks only squads previously seen alive.
Neither establishes that the expected encounter cohort has been admitted and
cleared. Keep the outro disabled until that cohort and its zero receipts are
known. A long timeout remains a fallback.

## Combat power and the super: conclusions and limits

The player level-power source reaches combat configuration:
`0x912791` -> `0x546fd0(player+0xc58, config+0xc)`; the source getter reads
`+8`. Config is at player `+0x1660`. `0x740342` -> `0x511dc0` -> `0x5142c0`
copies the block into component `0x80802ba7` at `+0x94..0xa3`, subject to its
mode flag. The observed source and three config floats were 40 at 10:11 UK.

Damage profile builder `0x512b00` copies that block. Damage calculation
`0x5078d0` -> `0x52a040` -> `0x529c40` selects content power-global tables and
uses `0x528f90` to select/normalize powers. The content has category-dependent
clamps and curve rows. Being over level therefore does not imply immunity.
This establishes a level-power route, not the complete numerical damage formula.
It does not show that the displayed Light 385 or Defense 1200 is the value used
at every incoming-damage call. No additional buff is justified by these reads.

The death-screen formatter reads an incoming-context float cached at
`0x277af10+0x2ac` (`0xa5b030`) and converts it through an investment stat curve
at `0x282370` (`0xa4fcc0`). It does not directly read the Guardian's displayed
Light row. The “Recommended Light 5” banner alone cannot establish that the
Guardian's combat level was 5.

Owned item 710 names talent grid 132, whose identity is Striker. Native slam
action `0x390a40` / `0x391330` waits on animation event `0x27` before impact.
The actual active action/ability during the sustained arc pose has not been
identified. A correct item/grid identity does not prove the selected node or
loaded ability; the visual resemblance to another class is not proof either.
No host acknowledgement requirement or emulator cause is established yet.

The earlier staged subclass instance, decoded from the private tutorial inventory,
has grid field `+0x58 = 0xffff`, not 132; node-selection bytes are zero. This is
separate from the **definition's** grid identity. Native getter `0x283d60`
returns null for a grid index of `0xffff`. It is a concrete configuration lead,
but does not establish that this inventory instance is the runtime ability source:
the equipped peer's selected grid and active action still need tracing. Do not
replace node selections with guessed “unlock everything” bytes.
