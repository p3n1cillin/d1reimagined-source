# Destiny 1 outer-wake 59.94 and fixed-30 wall gate

This default-off experiment is restricted to CUSA00219 01.29 and source SHA-256
`7271fcb926401df8defb126cb8eb2b247134138b81e401bbacb2ab60791b7954`. The outer-wake
transaction changes the disp8 at guest `0x5938ee` in the full pinned instruction
`48 03 4e 08` to `28`, selecting the existing CFG+0x28 16,683-us outer deadline. It does not
alter CFG+0x08, CFG+0x38, CFG+0x40, the fixed scalar, or Havok `h`.

The outer-wake-only profile is a coupled-cadence diagnostic. Its original hypothesis was that
accelerating the outer deadline could increase presentation opportunity while D1's existing time
control retained the fixed domain near 30 wall-Hz. A5 falsified that hypothesis: the complete
observed fixed/action/Havok set advanced at the accelerated outer cadence with stock `1/30` scalar.

Run `20260830-165526-D1-OUTER-WAKE-5994-A5-CLOCK-B2` ended orderly with a custody-valid focused
receipt, 99,303 records, and zero drops. Its final counts were:

```text
F1 phase / extract scalar              17,040 / 17,040
fixed publication / action sweep       13,955 / 13,955
Havok begin / result                    13,955 / 13,955
animation collection                     9,403
```

In one 10.304267502-second in-world interval, F1/extract increased by 372 (36.10/s), fixed
publication/action/Havok begin/result each increased by the same 372 (36.10/s), and animation
collection increased by 426. Extract retained `0x3d088889`, binary32 `1/30`. Thus the one-byte
outer-wake transaction alone over-advances the fixed domain whenever the host presents above 30;
it is not a decoupling patch.

## Fixed-30 wall-credit seam

`SHADPS4_D1_FIXED_30_WALL_GATE=1` adds a second exact-build gate at the pinned terminal CALL
`0x59395a -> 0x592820(now_us)`. The gateway captures immutable guest state and accepts one explicit
replacement-RDI result. Its stub writes only that result into the saved guest RDI slot, then
restores every other GPR, RFLAGS, x87 state, MXCSR, and YMM byte before the stock CALL executes.

The gate accumulates host steady-clock credit at the exact rational rate 30/1:

- no-credit outer: return the previously selected `now_us` and let D1 execute its stock
  zero-advance frame lifecycle;
- due outer: advance selected `now_us` by one rational `1/30` second;
- excess credit: retain at most eight quanta and spend at most one quantum per outer;
- host discontinuity: clamp one observed wall delta to 250 ms;
- second callback thread: pass through the original RDI and raise both mismatch and fault telemetry.

A due decision advances the selected input by exact `1,000,000/30` microseconds, alternating
33,333 and 33,334 microseconds. This is not CFG+0x08's stock 33,366-us or approximately 29.97-Hz
deadline and differs from that grid by about 1 ms per second; CFG+0x08 itself remains unchanged.

The gate never loops or directly calls F, X, Y, a network job, a render job, an animation routine,
or Havok. The untouched stock outer builder still creates one F/X/Y graph per outer opportunity.
The three timing literals and Havok `h` remain stock. At outer rates below 30, exact 30-Hz
authority is impossible without repeating a complete proved tick; this gate therefore saturates at
one due decision per outer and exposes bounded debt rather than claiming otherwise.

This is a live discriminator, not proof that every authoritative atom is below D1's zero-advance
gate. The focused B2 evidence must establish that relationship before the mechanism can be called
a completed simulation/presentation split.

## Build and activation

Configure the emulator with both experiment options. Outer-wake now brings in the outer gateway
substrate automatically:

```sh
cmake -DENABLE_D1_OUTER_WAKE_5994=ON -DENABLE_D1_FLIP_RATE_60_PROBE=ON <other-options> ..
```

The launch profile must leave `SHADPS4_D1_FIXED_STEP_ENABLE` unset. For the fixed-30 discriminator:

```sh
env -u SHADPS4_D1_FIXED_STEP_ENABLE \
    SHADPS4_D1_OUTER_WAKE_5994=1 \
    SHADPS4_D1_FLIP_RATE_60_PROBE=1 \
    SHADPS4_D1_FIXED_30_WALL_GATE=1 \
    ./shadps4 <normal-game-arguments>
```

Each experiment variable must be the literal value `1`. The runtime refuses an outer-wake-only
request without the paired flip probe, any title/build/source mismatch, a null or non-covering
EBOOT image, a full-instruction pin mismatch, a failed volatile readback, a failed outer-gateway
transaction, or any fixed-step-owner disposition other than `Disabled` or `StockPassthrough`.

When the fixed-30 variable is absent, the outer-wake transaction retains its A5 diagnostic
behavior and logs that the fixed domain remains outer-driven.

## A6 acceptance and stop conditions

The one-second presentation log includes a separate wall-gate line with outer, seed, hold, due,
thread-mismatch, fault, selected guest microseconds, and pending credit. Run B2 with the focused
keep-set `{257,259,260,261,272,295,296,297}` and require an orderly, zero-loss receipt.

`[O]` Run `20260830-172004-D1-OUTER-WAKE-5994-FIXED30-A6-CLOCK-B2` reached
`activity:in_world`. Over a 9.998970796-second in-world interval it recorded 300 each of
fixed-publication return (272), action-registry sweep (295), Havok begin (257), and Havok result
(259), while F1 entry/extract recorded 470/469 and outer telemetry ranged from 43 to 48/s. Due
telemetry was 30/s with zero mismatch/fault. The exact instance was reported visually excellent.

Probe 296 recorded 2,002 calls across 95 RDI identities, including one resident identity at 300.
Those calls occupied 300 of 471 consecutive-F1 buckets, matching the fixed-witness count. This
establishes per-instance collection multiplicity and supplies supporting fixed-cadence evidence;
without an explicit per-outer due identifier it does not prove each individual hold. Probe 297 was
absent. The final trace SHA-256 is
`069f58d12b55ad7bfafe43c7a4e9637fd95d7eea3d25327f8db85f56a97981d8`: 70,876,304 bytes,
210,941 complete records, no trailing bytes, contiguous sequence `0..210940`, and observer ticket
equal to sequence throughout. TTY shutdown prevented an orderly receipt, so this is an
uncustodied positive diagnostic rather than an acceptance candidate; it does not replace the
receipt requirement.

Accept the seam for continued testing only if all of the following hold after entering the world:

- outer/present may vary with host capacity, while wall-gate due decisions converge to 30/s when
  outer capacity remains at or above 30/s;
- fixed publication, action-registry sweep, Havok begin, and Havok result are pairwise balanced and
  match due decisions over aligned stable intervals;
- hold decisions produce none of those four authoritative witnesses;
- extract scalar and probe 297, when observed, remain `0x3d088889`;
- F1/extract may remain outer-owned, but no fixed witness may follow that higher cadence;
- thread-mismatch, gate fault, trace drops, rejected samples, and unbalanced Havok pairs remain zero;
- pending credit stays bounded and drains when outer capacity exceeds 30/s.

Stop on any fixed/action/Havok count that follows outer rather than due cadence, any authoritative
witness on a hold outer, changed fixed scalar, duplicate or missing Havok return, gate fault,
thread mismatch, unbounded debt, game-time drift, queue growth, receipt failure, crash, or network
regression. A visually smooth run does not override these gates.
