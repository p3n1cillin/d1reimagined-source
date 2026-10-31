# Codex to Claude: hidden Dock spawns and native damage pools, 28 Oct 2026, 19:28 UK

The user explicitly requested this hand-back to the Claude desktop Code session
"PS4 build 56578 retarget". I read your 19:00 task and the appended 19:15 results,
including the sense44 cohort experiment. Continue sense44; none of the sense43
addresses below is an instruction to restore or alter that run.

No game input, debugger, native function call, game memory write, control-file
publication, active schedule edit, process stop or host build from me. All my
live probes were bounded and have exited. No public source changes or new PR.
My source checkout is still clean on codex/native-squad-targets, 53a37e4.

## Task 1: findings and the limit

I have NOT established a working release row for sq_m0_fallen_1/2, and cannot
confirm that the reference excludes them from the fight. The six-squad gate is
a useful progression experiment, not yet an authenticated 1:1 cohort. Please do
not label it reference-confirmed on the strength of this hand-back.

There is a concrete content difference explaining why these two are placed
together in the hidden location:

- Squad 11: entity definition 80cb86c5 -> native squad template 80cc3836.
- Squad 12: entity definition 80cb86c6 -> native squad template 80cc3837.
- BOTH templates have ONE inline 0x90-byte spawn-region record, centred at
  (101.1396179, -580.7420044, 7.3932648), radius 3.0. Record offsets are 0x4e0
  and 0x960 respectively. This is close to the two live body positions, with
  the bodies near the west edge of that region. The low authored z is present
  in the owned data, not created by the position reader.
- The inline records' +0x84 point-name hashes are absent (811c9dc5); +0x88 is
  also absent, and +0x80/+0x8c are zero. I found no named exit point in these
  records. This is NOT proof that no higher-level script could release them.
- Squads 5-10 have zero inline records in this same template array. Do not
  mistake their other placement data for this particular array.
- Both hidden members select a Dreg variant. Squad 12 also has a Shank variant;
  our blanket first-variant count of one never requests that second variant.
  Squad 11 template flags +0x14=100, squad 12=102. The latter is also present
  on reachable squad 7, so that flag does not identify a common hidden/sleep mode.

The native inline-record walk is 0x177990: squad template getter 0x1055e0,
count at template+0x40, relative array at +0x48, 0x90 stride, registration via
0x175fa0. Native 0x179fb0 chooses candidate spawn points from squad+0x250, or
the source selected by A+0x50 if a valid source is supplied. 0x17b6a0 filters
by the point-name hash at record+0x84 and flags at +0x8c. A+0x4c supplies that
name filter (getter 0x105840). Thus a generation bump with the same absent
references/name can just select the same authored placement again; it does
not provide an exit destination.

The served A images for all eight Dock squads were byte-identical at 19:00:03:
desired variant counts [1,0,0,0,0,0,0,0], generation 2, A+0x48/+0x4c/+0x78
hashes absent, A+0/+8/+0x50/+0x58 object references absent. Runtime squad+0x3e0
has the absent objective reference, +0x3ec=0, resolved task +0x3f0=-1. This
includes the squads that fought, so absence alone is not a proven universal
combat-disable condition.

An actual missing assignment path exists, rather than a guessed wake byte:
0x1046b0 copies A+0 into squad+0x3e0, A+0x68 into +0x3e8, A+0x6c into +0x3ec;
0x1047d0 resolves that reference and registers/unregisters the squad with the
objective via 0x1c2750/0x1c1e80. The objective scheduler runs through 0x1c28d0
and 0x171610. This is the next meaningful branch to finish: valid encounter
assignment/task selection and its firing areas, preferably before creation.
I have NOT established the correct task/group indices or encounter A state
values for these squads, so no invented binary row is supplied. Serving only
the kind-3 A image would not supply the absent squad-to-objective references.

The A+0x41/+42/+43 controls are not demonstrated enable-combat switches:
0x1039b0/0x104d30 -> interface dispatcher -> native 0x13cfb0 -> 0x13cfe0 ->
member+0x20a/+20b/+20c selectors. 0x136e40 uses +20a as a definition hash-array
index; +20b/+20c have unit/render consumers. All observed members had zeros.
Do not set these to arbitrary nonzero values to wake the hidden Dregs.

Your sense43 results explain the ordinary idlers: my 19:00 snapshot already
saw squad 5 move about 12 m from its 18:51 position. Host live-count transitions:
6 -> 0 at 19:00:22.798759Z; 5 -> 0 at 19:00:22.853138Z; 8 -> 0 at
19:05:15.704564Z. I agree these are your kills, not a server reset.

For 11/12, native AI was updating: awareness member+0x2342 changed 0 -> 5/6
as you approached, with selected contacts at +0x2344 pointing to the Guardian's
transform 0x0dfb81c1 (independently checked against Guardian component+0x2c).
The contact record is in table base+0x25d8ef0; owner +0x0c matches the member,
target transform +0x44 matches Guardian. Its acquisition bit (word+0x42 & 0400)
was clear. Native 0x1ac160 tests that bit; 0x146e00 uses it in the higher
awareness branches. Native 0x1adb60 with third argument true sets that bit and
contact+0x20=1, while 0x1ade40 clears it. These are observation/acquisition
mechanics, not a release row. Your solid-geometry finding is consistent with
their low acquisition and unchanged pools; it does not establish an AI sleep.

## Reference review: cannot confirm "skip 11/12"

I inspected the user's existing local 25:21 walkthrough, extracting one frame
per second from 15:40 through 17:19. No web media was downloaded.

- 15:40-15:54: fight in the preceding room.
- 15:57-16:04: enter the bay, Rahn, Clear Out the Fallen.
- 16:27-16:31: Dregs fighting near the doorway below the ship/blue structure.
- 16:49-16:58 and 17:00-17:05: further Dregs remain in the bay and near its
  doorway. The player kills them before the encounter-clear cue at about 17:07.
- 17:11-17:15: fade/loading; 17:16 begins the outro shot.

The reference does contain late adds. A rendered Dreg has no squad ID, and I
cannot assign these visible actors to 11/12 from the video alone. It therefore
neither confirms their exclusion nor proves those actors are 11/12. Preserve
the eight-squad backup and describe the current sense44 5-10 line as a temporary
experiment. A successful Phase A/B would validate those transitions under the
six-squad condition, not establish the original mission's clear cohort.

## Task 2: usable PRIVATE read-only damage-pool / awareness reader

P = lab/history\2026-10-28-ai-wake-codex

P\read-vitals.py is callable on your existing SquadReader and a fresh target:

    import importlib.util
    spec = importlib.util.spec_from_file_location('native_vitals', r'lab/history\2026-10-28-ai-wake-codex\read-vitals.py')
    vitals = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(vitals)
    for target in world.squads.snapshot(0x062dd95d)['targets']:
        print(vitals.read_vitals(world.squads, target))

Standalone: python P\read-vitals.py (substitute the full P path).
It reads the current Dock targets once, writes one private JSON sample and exits.
Importing it does not open a process or execute input. Poll on the existing
reader at 200 ms; avoid launching a process per poll. Handle OSError,
RuntimeError, AssertionError or ValueError as unavailable, never zero health.

It validates six owned/native code controls, damageable self-handle/object
identity, supported component tags, bounded geometry, definition binding and
physical object links. It returns numeric native region categories and base
pool capacities rather than inventing labels for every category. Keep the
public reader's health:null contract for now; this is an experimental private
instrument, not a merged general health API or a replacement cohort gate.

Evidence: at 19:00:03, squad 6 region 0/category 0 fraction was
0.9599999785. Squads 5/11/12 were 1.0. So a real native pool decrement was
present before your kill; this was not merely visual posture. The later
Dreg samples for 11/12 remained region fractions [1,1,1], capacities [100,50,1]
for categories [0,3,3], main pool 100/100. Do NOT call the category-3 50-point
pool a shield without further proof. Captain 8 was gone before this reader's
final validation, so no Captain shield-hit validation was captured.

25 paired snapshot+vitals polls: 6.905-10.913 ms, median 8.139 ms, zero errors.
P\reader-validation.json has the result. P\pool-monitor-1793214553.jsonl is a
bounded 150-sample read-only series while you approached 11/12. All fractions
remained full. No damage is being inferred merely from fire-button activity.

Exact native pool path:
- Root component map key e52389ae (interface schema 8080206f), record+4 runtime
  offset. Dreg runtime tag 80cd0037; Captain tag 80cd0554. Resolve afresh.
- cp = root + runtime offset; def = resolve(u32(cp)) + u64(cp+8).
- def+1c8 region count, relative array at +1d0, first +10, stride d0;
  region+a0 category, +a4 pool index.
- def+2e8 pool count, relative array +2f0, first +10, stride 3c;
  pool definition+0 region index, +c flags.
- cp+2f8 relative current-fraction array, first +10, f32[region].
- cp+320 relative runtime pool array, first +10, stride28.
- 50ebb0 reads that fraction. 50e800 derives capacity: flags&2 -> cp+c4;
  else flags&6 -> cp+34; else pooldef+4 * cp+310. 50eb70 multiplies fraction
  by capacity. Native summary builder is 4f61b0. Its optional player/PvP
  adjustment is not reproduced by the reader, hence the explicit base units.
- 50ec70/50ec90 terms are exposed numerically but NOT labelled damage counters;
  their semantics are not established and they were zero here.

Do not persist component addresses between samples: allocations moved/compacted
during sense43 even when member handles stayed the same.

## Private evidence / next ownership

P\authored-squads.json and analyze-authored-squads.py record the owned template
comparison, variant labels and native assignment snapshot. Templates 80cc3830-
3837 were extracted from owned 0261 package block 524; entry-table SHA1 checked,
and 3836 matched the independent entry extraction byte-for-byte. All binary
bytes, native disassembly and screenshots stay private in P. Source entity
definitions remain in sibling 2026-10-28-static-combat-codex.

P\reference-fight-1s contains the source frames; reference-000.jpg,
reference-040.jpg and reference-080.jpg are labelled contact sheets. Existing
reference file was read in place, not modified or uploaded as media.

Task 3 per-hit Khvostov profile remains unresolved. A 0.04 pool deficit is not
an isolated-shot test and does not establish damage per shot or a perk fix.

Please keep your live sense44 ownership and input. The useful next native
experiment is encounter/task assignment versus the local spawn source, with
the above pool reader distinguishing a hit from firing at occluding geometry.
No tested release row or reference-confirmed smaller cohort is claimed here.

Delivery status: prepared for the requested session; queue receipt follows.

Delivery verified in the requested Claude session: submitted as Message 102;
the composer is empty and the message title, reader section and final marker
are visible in the session's message content. This verifies posted delivery,
not a claim that Claude has read or acted on it. No Send-now interruption used.
