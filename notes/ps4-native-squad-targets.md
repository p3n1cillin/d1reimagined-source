# Native active squad members on PS4 build 56578

`tools/ps4_setup/d1_squad_tracking.py` reads current squad membership and physical
positions without a debugger, game calls, game input or memory writes. The caller
supplies the reader and base returned by `d1-winguest.open_client()`.

```python
from d1_squad_tracking import SquadReader, Unavailable

tracker = SquadReader(reader, base)
sample = tracker.snapshot(0x062dd95d)  # Poll again; do not reuse old positions.
for target in sample['targets']:
    for part in target['parts']:
        print(target['squad'], part['role'], part['position'])
```

The single-shot CLI prints JSON on stdout and the existing client guard on stderr:

```text
python tools/ps4_setup/d1-live-enemies.py --group 0x062dd95d
```

## What the reader follows

1. Resolve the current content through `0x25d6ff8` and the self-relative lifetime
   table at `0x1ad5000`. Its kind-1 component list contains the squad components.
2. Resolve each component's `+0x1a8` sensor handle, and verify kind 1 plus A/B
   schemas `0x8080071e` / `0x80800357`. The physical list count is at `+0x278`;
   eight-byte serial/datum pairs begin at `+0x27c`.
3. Follow the native `0x71f60` pair validation, including the pool capacity and
   serial. Resolve the member in `0x25d79a0`; require self datum `+0x38`, and
   creation-counter membership `+0x50 == -1` (`0x103f70`). Also require the native
   sense predicate's generation match (`0x138190`). Its permissive missing-state
   fallback is an error here, not permission to target an object.
4. The member's `+0x3c` identifies its transform entity in `0x2692220` (stride
   240). Entity `+0x54` identifies the allocation parent. Look up the common
   component type `0x2b1bb31b` in that parent's definition map (`0x324870`), then
   read supported physical components at their authored runtime offsets.
5. Validate each physical component's self handle through the object table at
   `0x26897f0`, its transform link `+0x2c`, and finite position `+0x1a0`.
   Recheck identities, serial, sense state, member list and current content after
   sampling. No component addresses are cached across calls.

The member's `+0x40/+0x44` handles are **not** direct body/torso positions at
`+0x1a0`. They resolve different components. Similarly, the sensor's replicated
B buffer is a copy: subtracting `0x1ac` does not locate the squad component.

## Meaning and limits

`active` means native squad membership, and `sense_matches` means the native
count predicate matches. Neither is a measured health fraction. `health` is
explicitly `null`. Corpses that no longer belong to an active squad are not
selected merely because a physical object remains or recently moved. A stalled
member can still satisfy the native predicates; this reader cannot certify its
visibility, vulnerability or line of sight.

Body tags currently supported are `0x809dcef6` / `0x809dcce5`; torso tags are
`0x809de137` / `0x809ddc8b`. Only components belonging to the member's resolved
parent are included. A torso under a different child entity is not followed;
callers must not assume every target includes one or label a body position a
head position. No guessed aim-height offset is supplied.

An unavailable sample raises `Unavailable` or `OSError`. A member whose binding
cannot be established appears in its squad's `errors` and has no target. **An
empty target list, a missing body or a read error is never kill evidence.** Keep
the host's explicit cohort gate and its positive-to-zero admission rules.
This is a guarded non-atomic process snapshot, not a simulation lock.

## Validation

Eleven synthetic controls cover the pointer walk, serial/object reuse, absent
bindings, nonfinite positions, native membership/sense filtering, missing sense
state, group isolation, list/content changes and a death-state change during
sampling. They run without Windows or game files.

On 28 October a bounded read-only check resolved all eight Dock squads and their
member bindings in the running client. Later samples observed three squads with
empty lists while others retained physical bodies. A 25-sample check at 200 ms
intervals took about 3.4–5.5 ms per sample with no unavailable sample. This is a
reader timing/control result, not an automated combat or mission-completion test.
Raw evidence and live addresses remain private.

The earlier description of squads 8/10/11/12 as never spawning is not established
for the current run: their member and physical object bindings were found. There
is no evidence here to omit them from the mission-clear cohort or publish new
spawn/encounter state. Historical reports include squads 8 and 10 reaching zero
in an earlier run. Health and the cause of any stalled or invisible member remain
open.
