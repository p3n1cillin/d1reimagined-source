# The final PS3 build (01.24) against 01.00: static findings

**Current authority (2026-09-30):** read the [01.24 audit checkpoint](checkpoint-2026-09-30-final124-audit.md)
before using the accumulated live-run theories below. The latest claim that stage-object
`+0x30 == 0` is a dead end is withdrawn: the `crorc` guard initializes zero to one.
The claimed Boolean result of `0x332d24` is inverted too. Earlier D1P hit counts can
include scheduler retries before any guest instruction runs; they do not establish
independent observations throughout a session. The historical notes remain as an
experiment trail, with the last section corrected below.

Sources are the EBOOTs of updates 01.13 and 01.24, decrypted on the PC by
`scripts/Invoke-FinalBuildExport.ps1` (SHA-256 in the data repo's `ps3_updates/EXPORT_VALIDATION.md`). The final
investment globals chains 012c and 012f were read over HTTP ranges with `ps3_update.py fetch`.
This page records static findings; CONFIRMED means read directly from the executable or
packages. The first live 01.24 runs and their remaining loading problem are recorded in
[`checkpoint-2026-09-28-final124-run.md`](checkpoint-2026-09-28-final124-run.md).

## Reading the update executables

- **The type definitions changed format (CONFIRMED).** The first export found only 351 types in
  01.24; the true count is 5,041. Two changes caused the miss.
  - Definitions are 0x20 bytes shorter and link their field list from def+0x34, not +0x54.
  - Each 0x28-byte field entry now leads with a repeat count; the rest of the entry follows 4
    bytes later.
  - A fixed array such as `u8[128]` is now one entry {128, offset = stride, ...}. 01.00 listed
    every element. All 265 such arrays are the sole field of their type, with count × stride =
    size.
  - `d1_types.py` reads both layouts. It picks the one that finds the most types, and expands
    arrays back into elements, so the two builds compare field for field. The 01.00 export is
    byte-identical to before.
- **The TOC base moved:** 0x175a5e8 (01.00), 0x184fc00 (01.13), 0x1891798 (01.24).
  - `d1_eboot.py` now reads it from the entry point's function descriptor.
  - The curated labels are 01.00 addresses. They are applied to that build only.
- **Most type handles are stable (static match).** 01.00 → 01.24 with the
  one-to-one matcher:
  - 4,308 of 4,387 types match (3,744 structurally identical, 564 changed);
  - 79 are unmatched in 01.24 and 733 are new;
  - 156 matched types were renumbered.

  These counts depend on heuristic matching where a handle did change; the
  reports are not proof of type identity for every pair. Almost every handle
  the stand-in writes keeps its number. `d1_builddiff.py` now reserves
  unchanged handles first and never assigns one new type to two old types.
  It compares field kinds (offsets shift whenever a field is inserted) and
  prints side-by-side layouts of the stand-in's types.

## What changed for the stand-in

**Character objects (CONFIRMED, same handles, grown types):**

| Where | 01.00 | 01.24 |
| --- | --- | --- |
| progression `0x80801855` | 0x770; the 20 gear slots at +0x138 | 0x8c8; three new f32 after +0; gear slots at **+0x1e8** |
| gear slot `0x8080183c` | 0x28: {u64 id, i16 item, u8, i16, +0xe dyes} | **0x30**: {u64 id, i16 item, i16, i16, +0x10 new `0x80801069`, +0x18 dyes} |
| peer summary `0x808017fd` | progression at +0x30 | progression at +0x30 (size 0x7b0 → 0x908) |
| select summary `0x80801954` | progression at +0x68 | progression at +0x68 (size 0x7e8 → 0x940) |
| self character `0x80801714` | 0x55b0: inventory +0x30 (0x1808), equipped ids +0x1838 | 0x6db8: inventory +0x30 (**0x2808**), new `0x808019ff` +0x2838, equipped ids **+0x2900** |
| inventory entry `0x808016e0` | 0x18: {i16 item, u64 id, i32, i32} | 0x20: one more i32 |
| account `0x808018c7` | 0x1ff8: flag table A +0x12a8 (1,024), table B +0x16a8+1 (256) | 0x7050: B at **+0x4f78+8**; A LIKELY `0x80804b0c` at +0x383c (**2,560** flags); vault-like `0x80801087` (326 items) at +0x4c0 |
| character flags (table C) | self +0x3bc0, 1,024 | self **+0x5000**, 2,048 (`0x80804c21`) |

- `bap_server.character_layout(schema)` derives the gear offsets from whichever schema the
  stand-in loads. It finds each field by its nested type's handle, so `--character-gear`
  writes 01.24's layout when given a 01.24 schema. With the 01.00 schema (or none) it gives
  the old constants.
- **2026-10-05:** final gear-slot +0xc selects the armour model arrangement, separately
  from item index +8. Omitting it left all five world armour roots invalid while the
  inventory preview remained normally proportioned. `starter:01.24:CLASS` now supplies
  the verified starter selections; custom JSON accepts `arrangement`. See the
  [native conversion, mapping and live evidence](guardian-appearance-0124.md).
- `--self-flags` now finds account tables A/B and character table C in the loaded schema.
  The 01.24 static layout gives A at +0x383c (2,560 flags), B at +0x4f78/+8
  (256), and C at self character +0x5000 (2,048). An unqualified `--self-flags 2`
  fills the full capacity of each selected table. These are structural matches;
  which new flags grant which content still needs runtime work.

**Mission objects (CONFIRMED):**

- The object id `0x808004b1` widened the kind field from 6 to 7 bits. The mission tooling
  encodes ids from the schema, so a 01.24 schema handles this.
- The objective `0x80802a1b` grew (0x2d0 → 0x2f0), but its player mask stays at **+0x2c0**. A
  machine id and a second object id were added at +0x2cc/+0x2d4.
- The mission-script state handles remain present in 01.24. A direct comparison of the
  corrected static exports gives the following migration check (runtime behavior is untested):

  | State | 01.00 -> 01.24 | Consequence for the stand-in |
  | --- | --- | --- |
  | Cinematic `0x8080254d` | 0xd4 bytes and all 9 fields identical | The play counter, playing flag and cast-list offsets remain valid. |
  | Player trigger `0x80800463` | 8 bytes and both fields identical | The armed flag and arming time remain at their old offsets. |
  | Sequence `0x808029a1` | 0x2c -> 0x20 bytes; nested state at +0x8 changes from `0x80805b11` to `0x80805cf5`; final object reference moves +0x24 -> +0x18 | Rebuild the sequence state with the 01.24 schema; copying the 01.00 payload would misplace the reference. |
  | Squad `0x8080071e` | 0x60 -> 0x7c bytes; 13 -> 18 fields; the 2-bit, bias-1 existing-member field moves +0x5c -> +0x74 | The scripts use `squad_existing_members` to resolve that field from the selected schema. Confirm 01.24 behavior in a run. |
  | Objective `0x80802a1b` | 0x2d0 -> 0x2f0 bytes; 13 -> 16 fields | The player mask remains at +0x2c0, but serialize the full new layout. |

  The data repo's initial update-build reports used the old type scanner.
  Corrected exports and one-to-one handle maps are in `d1xenia-data/ps3_updates/`.

  An offline encode/decode check of all 80 state changes in the A Guardian Rises and Tower
  scripts passes with both the saved 01.00 activity schema and the corrected 01.24 type export.
  Before the field migration, 56 squad changes still named +0x5c; the 01.24 encoder silently
  omitted that field. `scripts/d1_mission_preflight.py` now catches such omissions, and
  `--ah-mission` runs it at startup. This does not establish that 01.24 accepts or acts on the
  states; that requires a live run.
  The run scripts still default to the saved 01.00 schemas. For an 01.24 run, pass the
  corrected `types_01.24.json` as `--schema` to the BAP stand-in and as
  `--activity-schema` to the activity host (via `-ExtraBap` and `-ExtraAh`).

**01.24 STUN is the remaining blocker (RUN A01, 2026-09-30, second PC).** After
installing the 1,034 update packages into `RPCS3_FINAL124_SMOKE001`, the first full
01.24 run on this PC reaches `bootflow:bap_signin` (state 19 of 38 - the transferred
SMOKE019-023 captures show the same, so content and package handling are solved) and
then waits. All 20 STUN hostnames resolve to the stub; 93 probes arrive and none are
answered; **the BAP stand-in never sees a connection**. So STUN, not BAP, is what
blocks progress past `bap_signin`.

**The 01.24 probe is `type, version, fields` - the same layout as 01.00, NOT MessagePack.**
Captured shapes:

```
len=4   00 01 00 01
len=29  0e 02 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 ff 00 ff 00 00 00 ff 00 ff 00 00
```

This was first read as MessagePack (`0x0e` = fixmap(14), 7 int pairs, `-1` = unbound
ports) and **that reading is wrong** - it is recorded only because `decode_stun_probe.py`
and the `--fill` option were built on it. The disassembly settles it. The request is
assembled by `0xb4695c`, which in order does:

```
lbz  r11, 0(r4)      ; r11 = the type byte, read from the caller's buffer (0x0e)
sth  r0,  2(r9)      ; (li r0,2 precedes) store the CONSTANT 2 at offset 2
stw  r5, 0x10(r9)    ; the caller's r5 (0) at offset 0x10
stb  r11, 0(r9)      ; the type byte at offset 0
```

so the header is `type, version, field` exactly as 01.00 sent `1e 03 00 <mapped> <port>`
(type 30, version 3). Reading `0x0e` as a MessagePack fixmap was arithmetically valid,
which is why it survived a first pass, but the byte is a **type** (14) and the `0x02`
beside it is the **version** (2) - not a key and a map header. The 27 trailing bytes are
fixed fields, not 7 key/value pairs.

Consequence: `--fill` and `--shape` were shaping replies for a protocol the client never
sent. Their negative results still hold (no sign-on request either way) but they were
varied on a wrong model and should be re-run once the field layout is read properly.

**Re-run on the corrected model: `--shape typeversion` (RUN TV).** With the header
understood, the stub can finally emit the 01.00 layout - `type, version, 0, mapped
IPv4, mapped port` - via `--reply-type`/`--reply-version` (defaults 15 and 2, i.e. one
above the request's type 14 as a response). The reply it produced is
`0f 02 00 7f 00 00 01 02 0c`, which is byte-for-byte the same shape 01.00 sent for type
30 (`1f 03 00 7f 00 00 01 02 0c`). Result: **still no change** - 21 probes, the 4-byte
probe stopping after 5, the counter still cycling `1..10`, no BAP connection and still
zero sign-on requests.

So even a correctly framed, correctly addressed reply is not accepted. That narrows it
further: the client is not waiting for a plainer or cleverer reply body, so either the
type/version pair is wrong (15/2 is an assumption - the real response type may not be
request+1), or the 29-byte probe is not asking for an address at all and needs some
other payload, or the client is blocked on something outside this exchange entirely.

**Reply-type sweep (RUN RT0/RT1/RT2/RT15) - the type byte is not the variable.** With
the header understood, `--reply-type` was swept over 0, 1, 2 and 15 on the
`--shape typeversion` reply, one variable per run, breaking out early on any progress:

| `--reply-type` | probes | replies | sign-on requests | BAP connections | verdict |
| ---: | ---: | ---: | ---: | ---: | --- |
| 0 | 17 | 17 | 0 | 0 | no progress |
| 1 | 17 | 17 | 0 | 0 | no progress |
| 2 | 16 | 16 | 0 | 0 | no progress |
| 15 | 17 | 17 | 0 | 0 | no progress |

Identical in every respect. So the client is not selecting its next step based on a
response type we can influence, which is consistent with the PPU-hook result that the
dispatched word is client state. Combined with the earlier sweeps (dispatch index, fill,
shape, per-name addresses) that is now **six independent reply-side variables with no
effect at all**. The evidence is consistent with the client simply not considering this
exchange successful regardless of what comes back - which points away from the STUN reply
and towards whatever gates the transition out of the retry loop.

**Practical conclusion for the next session.** Answering STUN correctly is necessary but
demonstrably not sufficient, and the reply is not where the remaining work is. The 47-byte
send that looked like a missing third probe turned out to be the DNS query (RUN P47), so
it is closed. What remains is a **client-side condition**: the client has everything we
can send it and still does not advance, so the next step is to find the test it applies
before leaving the retry loop - the branch at the `1..10` sub-step counter's terminal
state, and whatever it requires to be true. A lawful capture of a real STUN exchange from
the owner's console would give the actual request/reply pair and is the fastest way to
confirm or refute everything inferred here; further reply guessing is not.

**Where the reply is dispatched (answers the open question in the runbook).** The
earlier note guessed the type "is most likely read into a register and used as a table
index" because the 01.00 types are absent from `cmpwi`. That is confirmed:

- response handler `0xb36fd8`, called from the transport state machine at `0xb33910`
  and `0xb339c4` (inside `0xb33814`, the state machine `0xb3012c` drives)
- `0xb3707c`: `lwz r0, 0(r3)` + `cmplwi cr7, r0, 0x13` - bounds check on the reply's
  first word, so valid indices are 0..19
- `0xb370e0`: `lwz r11, -0x2834(r2)` (TOC) + `rldic r9, r0, 2, 0x1e` + `lwzx r0, r9,
  r11` - a **jump table at `0xb370fc` indexed by that word**, entries signed
  pc-relative to `0xb370e4` (20 entries: idx 0 -> `0xb37410`, 1 -> `0xb373e4`,
  2 -> `0xb37620`, ... 16 -> `0xb371c0`, 19 -> `0xb37190`)

So 01.24 dispatches STUN replies on a small integer index read from the packet, not on
a type byte compared with a constant - which is exactly why searching for the type
constants found nothing. `server/stun_stub.py` now answers both 01.24 shapes with
`--reply-index` (default 0) choosing the dispatch word; the reply body is still inferred,
not confirmed from the client, and is the next thing to confirm by running.

**The reply dispatch index is NOT a field of the packet (PPU hook, RUN HOOK01).** The
instrumented build traced the handler directly (`d1\d1ppu.txt`, watched `0xb36fd8`,
`0xb33814`, `0xb370e0`). The handler ran 40 times and the dispatch 14 times, so replies
*are* arriving and being accepted - but the 64-byte object at `r3` on entry is:

```
word  0(r3) : 1, 2, 3, 3, ... 10, 10      <- increments across calls
dword 0x10  : 0x01a8c0  (constant)
byte  0x18  : 0, 0, 0 ... 3, 3           <- advances late in the run
```

So the word at `0(r3)` is **client-side state, not something the stub sent**: it counts
calls to the handler and is the value bounds-checked against `0x13` at `0xb3707c` and
used to index the jump table. The client walks its own state machine 1..0x13 as it
retries. This reframes the problem: `--reply-index` cannot steer the client's path
(because the index is not ours to choose), it only has to be a legal, self-consistent
reply. What still has to be discovered is which of the 20 table branches each stage
needs, and what the client then *does* with the parsed map.

Observed effect of answering (RUN t4, `--reply-index 0`): the client **stopped sending
the 4-byte probe** and sent only the 29-byte map from then on, while BAP still saw no
connection. So a reply is recognised and changes behaviour; it is not yet accepted as
success. The byte at `0x18` going 0 -> 3 is the visible progress in that run.

**The stage is written by the client, not read from our reply.** At `0xb37168` the
request path does `li r0, 0xe; stw r0, 0(r30)` - on a failed sub-step it stores **14**
into the same `0(r3)` word that `0xb3707c` later bounds-checks and `0xb370e0` uses as the
jump-table index. So the counter is the client's own STUN sub-step number (it reaches
`0x0e` on failure), and the 1..10 seen under the hook is that counter walking forward
while it retries. This is why `--reply-index` cannot steer anything: the index is not
carried by the packet at all. Any legal, self-consistent reply gives the same result.

**Sweep result (two indices, one variable each).** `--reply-index 0` and `1` behave
identically: the client sends 5 of the 4-byte probe, then **only** the 29-byte map for the
rest of the run, and the BAP stand-in never logs a connection.

| run | index | probes | replies | 4-byte | 29-byte | BAP |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| t4 | 0 | 21 | 21 | 5 | 16 | no connection |
| A_STUNIDX1 | 1 | 37 | 37 | 5 | 32 | no connection |

So answering *is* recognised (the short probe stops permanently after ~5 attempts, in both
runs) but is not accepted as success, and the choice of index makes no difference -
consistent with the counter being client state. The remaining unknown is the **reply
body**: what the client expects the parsed map to contain for the stage to advance past
`0x0e`. The `-1` sentinels in the request are the lead: they are ports the client has not
bound, and a reply that fills the *right* ones (rather than the first) is the next thing
to try. `stun_stub.py --reply-index -1` is available as the silent control.

**The counter is a STUN *host* index, not a stage (strong correlation, 2026-09-30).**
Run A01 resolved exactly **20** STUN names - `destiny-stun.{us,eu,jp,au}.demonware.net`
(4) plus `stun1..8.signon.{deadorbit,gravityshavings}.net` (16) - and the dispatch
bounds check at `0xb3707c` accepts `0..0x13`, i.e. **exactly 20 values**, indexing the
20-entry table at `0xb370fc`. Under the hook the counter ran `1,2,3,...10` and never
reached `0x0e`; with a 400-hit watch cap the failure branch at `0xb37168` (which stores
`0x0e`) was **never hit**. So the word at `0(r3)` advances per host as the client works
through its list, and the client is **stuck retrying hosts, not failing a sub-step**.

That reframes the experiment: there is no single "right" index to send, and `--fill` (all
unbound ports answered) changed nothing either - 47 probes, 5 short + 42 map, BAP still
silent. The next thing to determine is which host the client is on when it stops making
progress, and whether it needs replies from *every* host in the list (a single STUN
server may have to answer as all 20 names, or the DNS sinkhole may need to hand out
distinct addresses per name so the client sees a consistent mapping).

**Distinct-address experiment (RUN DISTINCT) - negative, and it corrects the host-index
guess.** `dns_sinkhole.py --stun-distinct` and `stun_stub.py --bind-count 20` were added
so each of the 20 names resolves to its own address (127.0.2.2 .. 127.0.2.21) and is
answered by a real separate socket. Verified: all 20 DNS answers were distinct and the
stub reported all 20 bound. Result:

- **zero** STUN packets reached the stub (1 line in its log: the banner)
- the handler still ran 88 times, and the counter still only ever took the values
  `1..10` and then **repeated** - it never went 11..20, and never reached `0x0e`
- 50 `sys_net_bnet_sendto` calls, 23 connects, no `ENOTCONN`

So the word at `0(r3)` is **not** a host index either: with every host on its own address
the count still stops at 10 and cycles, where 20 addresses would have allowed 1..20. It
is a retry/sub-step counter bounded at 10, and the client gives up and restarts rather
than advancing. The packets that never arrive are `len=47` on socket 26 with
`addrlen=0` - a different, unconnected send, not either of the two probe shapes, so
something beyond the STUN probe is being attempted here and nothing is listening for it.

**Where this leaves 01.24.** The startup path to `bootflow:bap_signin` is solid
(confirmed by three independent capture sets), the content/package path is solved (1,034
packages installed, preflight green), and STUN is no longer simply ignored - replies are
parsed and change client behaviour. But the client does not accept the reply as success,
and the next unknown is the **reply body** (what the parsed map must contain to move the
stage past its current stop), plus that unconnected 47-byte send. Neither is guessable
from the notes; both need either more static analysis of the map parser or a lawful
capture of a real STUN exchange from the owner's console.

**Sign-on is never reached at all.** Worth stating plainly, because it changes what the
blocker is: across FINAL124_S01, t4, STAGE and DISTINCT the SignOn listener logged
**56 lines, all of them "listening on ..." banners, and not one client request**. The
client's first network act is DNS for the 20 STUN names (06:36:45) followed immediately
by the probes (06:36:58), and in 12+ minutes it never opens an HTTPS connection to
SignOn at all. So "01.24 stalls before BAP" understates it: it stalls before
**sign-on**, at the very first thing it does. That is consistent with the 01.00 route,
where STUN precedes sign-on too, and it means getting the STUN exchange right is worth
more than anything downstream - nothing else can even be tested until the client leaves
this loop.

**Environment re-verified, so the stall is the protocol and not the setup** (2026-09-30).
Worth recording because "it is just broken locally" is the obvious thing to suspect on a
freshly rebuilt PC: RPCN is running and port 31313 accepts, and its log shows the client
authenticating (`Succeeded with NoError` for `ResetState`); `config.yml` has
`PSN status: RPCN` and `DNS address: 127.0.0.1`; `rpcn.yml` points at
`127.0.0.1:31313` as the local test server. The preflight also passes all 14 checks
(1,034 packages at the right sizes, both 01.24 patches enabled, CA matching). So the
stand-ins, the RPCS3 copy and the emulator are all in the state the 01.00 route used, and
the remaining difference is that 01.24's first request is one we do not yet answer
correctly.

**Reply-shape sweep (RUN SHAPEBIN) - also negative.** `--shape map|binary|index` varies
only the reply layout, keeping the same leading dispatch index. `binary` (dispatch index
then the 01.00-style mapped IPv4/port) behaves exactly like `map`: 20 probes, the 4-byte
probe stopping after 5, no BAP connection, and still **zero** sign-on requests. So the
layout of what we send back is not the variable that matters either. Three independent
variables have now been ruled out by experiment - the dispatch index, how many of the
unbound ports we fill, and the reply layout.

That is a useful result rather than a dead end: it says the missing piece is not a knob
on the reply we are already sending. The strongest remaining candidates, in order:

1. the **47-byte unconnected send** on socket 26 (`addrlen=0`) seen in every run - a
   message that is neither probe shape and that nothing is listening for;
2. the **reply field semantics** - the client may expect specific field values in the
   `type, version, fields` reply, not just a plausible address;
3. a **second exchange** the client expects before it will leave the loop.

**The 47-byte "third STUN shape" is a DNS query - resolved and closed (RUN P47).** It was
listed above as a lead and it is now answered. Tracing `0xb340bc` (the socket-send wrapper
the PPU hook watched) shows the sequence plainly:

```
sys_net_bnet_socket(family=INET, type=DGRAM, protocol=IPPROTO_IP)
sys_net_bnet_connect(s=26, ...)
sys_net: [Native] Attempting to connect on 127.0.0.1:53
sys_net: [Native] sys_net_bnet_connect: using DNS...
sys_net_bnet_sendto(s=26, buf=*0x323ae010, len=47, flags=0x0, addr=*0x0, addrlen=0)
```

The send goes to **port 53** - it *is* the DNS query, 47 bytes of DNS wire format, which
is why no destination is recorded and why it never reaches the STUN stub. There are
exactly **20** such connects in a run, one per STUN hostname, and the sinkhole answered
all 20. (The only other outbound UDP is 3 connects to `<router>:1900` - SSDP/UPnP,
unrelated and unanswered.) So this is neither a missing third probe shape nor a missing
listener: the two shapes the stub sees are the complete set of STUN probes, and every
name resolves.

That closes the last "something we are not sending" candidate. Together with the six flat
reply-side sweeps, the picture is that the client resolves, probes, receives our replies,
and still does not leave the loop - so the remaining unknown is a **client-side
condition** (what it decides must be true before advancing) rather than anything missing
from the wire.

**The transport state machine behind that loop, decoded.** `0xb33814` is the outer state
machine; at `0xb338a8` it loads its own word counter and bounds-checks it against **7**,
then at `0xb3393c` uses it to index a second jump table (TOC `-0x2990`, table at
`0xb33958`, entries pc-relative). The eight states:

| state | target | what it does |
| ---: | --- | --- |
| 0 | `0xb33978` | logs (`0x64` = line 100) then falls through to the state-6 tail |
| 1 | `0xb33ab8` | |
| 2 | `0xb33a14` | |
| 3 | `0xb339ec` | |
| 4 | `0xb339b8` | **calls `0xb36fd8` - the STUN response handler.** This is the probe state. |
| 5 | `0xb3385c` | |
| 6 | `0xb33880` | the shared tail every state branches back to (`0xb3385c`-`0xb33880` are reached by `b 0xb33880`) |
| 7 | `0xb339a4` | calls `0xd73b10` on `r3+0xac` |

State 4 reaching `0xb36fd8` confirms the PPU-hook attribution: the handler we have been
tracing is entered from the transport machine's probe state, and the loop is this 8-state
machine cycling rather than the 20-entry reply table. What decides the transition out of
state 4 is the remaining question, and it is a single branch in `0xb339b8`/`0xb36fd8`
rather than anything on the wire.

**THE GATE (RUN GATE18): state 4 exits only on 18 or 19, and the client never reports
either.** This is the concrete condition, found by reading state 4's own exit test and
then measuring it with the PPU hook on `0x69ddb0` (the session getter state 4 calls after
the STUN handler):

```
0xb339b8  addi r0, r3, 0x4c0
0xb339c4  bl   0xb36fd8        ; the STUN response handler
0xb339d0  bl   0x69ddb0        ; read the session result into r3
0xb339d8  cmpwi r3, 0x12       ; 18 -> carry on
0xb339dc  bne  0xb33b3c
...
0xb33b48  cmpwi r3, 0x13       ; 19 -> carry on
0xb33b4c  beq  0xb33b64
0xb33b5c  cmpwi r3, 0          ; 0 -> back to the shared tail, keep looping
```

Measured return values of `0x69ddb0` over a full run (RUN GATE18, 200 hook hits):

| value | calls | meaning |
| ---: | ---: | --- |
| 0 | 193 | not ready - the value that keeps the loop going |
| 1 | 6 | |
| other | 1 | |
| **18** | **0** | **required to advance** |
| **19** | **0** | **also advances** |

**This explains every negative result in this section at once.** The stub's replies *are*
received and parsed - the handler runs, the sub-step counter advances, the 4-byte probe
stops - but the session getter that state 4 actually tests keeps returning 0. Nothing we
put in the packet can change that, because the value is not a packet field: it is the
client's own "STUN exchange complete" flag, and it is set by the parser only when the
reply satisfies conditions we have not identified. Every reply-side variable
(dispatch index, fill, shape, per-name addresses, reply shape, reply type) was therefore
guessing at a value the client never reads from the wire in the first place.

So the remaining work is precise and bounded: **find what makes the parser set that
flag.** The next concrete steps, in order:

1. read the success path of the parse routine that feeds `0x69ddb0`'s value, to see the
   condition under which it becomes 18/19 rather than 0;
2. since 18/19 sit right at the size of the host list (20 names, 20-entry table), check
   whether the value is a count of hosts completed and whether all 20 must answer;
3. the 01.00 twin of this test is the same shape (its types 20/30 answer with 21/31), so
   the 01.00 route's success value is worth reading for the expected encoding.

**Correction to the above: 18 and 19 are both *error* states, not success (static read).**
`0x69ddb0` is literally `lwz r3, 0(r3); blr` - the field is the same word the handler
dispatches on, so the value *is* that word. Sweeping the whole handler body for
`stw rX, 0(r30)` preceded by its `li` shows only two places ever write the two values
state 4 tests:

```
; value 19, at 0xb370b0 - immediately AFTER the range check
0xb3707c  lwz    r0, 0(r3)
0xb37080  cmplwi r0, 0x13
0xb37084  ble    0xb370e0        ; in range -> jump table
   ...falls through...
0xb370a8  bl     0xb38638        ; log (line 0x1b2 = 434)
0xb370b0  li     r0, 0x13
0xb370b8  stw    r0, 0(r30)      ; 19 = "index out of range"

; value 18, at 0xb379e8 - also a log-then-store pair
0xb379cc  li     r8, 0x10c       ; log line 268
0xb379e0  bl     0xb38638
0xb379e8  li     r0, 0x12
0xb379ec  stw    r0, 0(r30)      ; 18 = another logged error state
```

So state 4's exit test is `result == 18 || result == 19`, and **both of those are
diagnostic error states the handler writes after logging**. The client therefore leaves
the probe state only on a failure. Since neither ever occurs in our runs (193/200 getter
calls returned 0), the field is sitting in a normal intermediate state and the client is
waiting for a transition that the handler never takes for us.

Two consequences, and they are the useful part:

1. the "success" value we should be looking for is **not** 18 or 19 - it is whatever the
   handler writes on the *non-error* path, i.e. one of the `stw` stores belonging to a
   live jump-table branch (indices 7, 8, 14, 15 write 2, 3, 3, 1 respectively; the rest
   branch onward before storing). Which of those means "STUN complete" is what to
   determine next;
2. the reply-table index **is** what selects that path - the index decides which branch
   runs and therefore which value lands in the field. So the earlier sweeps were testing
   the right thing after all; they just never guessed the index whose branch is the
   success path, and `--reply-index` cannot be used to aim at it because the index is
   read from the client-side field rather than from our packet.

That last point is the real obstacle and worth being explicit about: the branch is chosen
by a word the client computes, so the way to reach the success path is to satisfy the
condition that makes the client compute that word - not to choose a reply. The hook on
`0x69ddb0` is the instrument for it: watch the field across a run whose replies differ in
one field at a time, and the value that first appears as 18-or-anything-else is the
success encoding.

**"All 20 hosts must answer" is not it either (RUN ALL20).** Since 18/19 sit right at the
size of the host list, the obvious guess is that the field counts completed hosts and the
client waits for all 20. Tested properly: `--stun-distinct --bind-count 20` so each of the
20 names resolves to its own address and is answered by its own socket, with the
`typeversion` reply. Result: 20 DNS answers (all distinct), **0 probes reached the stub**,
and the field still returned **0 in 198 of 200** getter calls (1 once, one stray value).
No BAP connection, no sign-on request.

So the field is not a host-completion counter either. Combined with the earlier
measurement (RUN GATE18, same 193/200 zeros) the conclusion is stable across both
configurations: the word is not counting anything we influence. It is most likely a
**sub-state within the STUN sequence** that only advances when the client is satisfied
with a *specific* reply it has not yet been given - which brings us back to needing the
real reply bytes rather than more shapes to try.

**What 01.00 actually needed, for comparison (from TEST005/006 in `test-log.md`).** This
is the most useful reference in the repo and it sharpens the question:

- 01.00 sent two probes, `1e 03 00` (IP discovery) then `14 02 00 xx` (NAT discovery);
  `stun_stub.py` answered them in the BO4 layout (types 30/20 -> 31/21) and **marionberry
  cleared**. That is the whole reason 01.00 got as far as it did.
- The **same TEST006 entry records a 29-byte type `0x0e` packet repeating every 15 s to
  the STUN host, unanswered** - and 01.00 still proceeded. So 01.00 *tolerated* an
  unanswered `0x0e`; it did not need it to succeed.
- 01.24 sends only `00 01 00 01` and `0e 02 ...`. We answer both, on the same
  loopback-and-stub setup that worked for 01.00, and it still loops.

That is the real asymmetry. 01.00's gate was satisfied by the *type 30/20 exchange alone*
and treated `0x0e` as noise. 01.24 has no 30/20 exchange at all, so the gate must be
carried by something else - and since answering `0x0e` correctly is the only thing left in
this exchange, the `0x0e` reply is where the answer is. Guessing its contents has failed
seven ways now (index, fill, shape, per-name addresses, reply type, reply version,
host-count), which is strong evidence the reply is not a short fixed record like 01.00's -
it is more likely a structured payload whose length and field order matter, which is
exactly what a real capture would settle and what no amount of sweeping will.

**Control run (RUN LEGACY): the stub with no 01.24 replies at all**, reproducing the
01.00-era behaviour, to confirm the difference is in the reply rather than the setup.
Result: 21 probes, 21 replies (all the 01.00 types), **the field still 0 in 195 of 200**
getter calls, no BAP connection, no sign-on request.

That is the cleanest result of the session. With `--reply-index` at its default the stub
still answers (the 01.00 branches are tried first and then 01.24), and the measured field
distribution is **byte-identical to RUN GATE18 and RUN ALL20** (195/193/198 zeros, the
occasional 1, one stray pointer value). So across every configuration tried - 01.00
replies, 01.24 replies in four shapes, four reply types, two versions, per-name addresses,
all-hosts-answering - the value the client keeps in that field never changes.

**What that means.** The field is not a function of anything we transmit. Whatever the
client needs in order to leave the probe state, it is not obtaining from this exchange at
all in any form we have tried. Either

1. the `0x0e` reply has to be substantially correct - a structured payload, not the
   short address record we have been sending - and no short variant will do; or
2. the client is waiting on a *second* channel (a different port, a different host
   family, or a result delivered by the platform layer) before it will set that state,
   in which case no STUN reply is relevant at all.

Distinguishing those two needs one more piece of information that only the client or a
real capture can give: what the client does with the bytes after the type/version header
in the `0x0e` request, i.e. the field layout at offsets 2..28 that we have never read
(`0xb4695c` builds the header, but the body is written elsewhere). That is the single
highest-value next task, and the PPU hook on `0xb4695c` with a wider dump is the tool for
it.

**The `0x0e` body layout, as far as the captured bytes go (RUN BODY).** Hooking
`0xb4695c` with a 0x60-byte dump shows the packet buffer is **all zeros at function entry**
(60/60 hits identical) - the type byte, the constant 2 and the body are all written inside
it, so that address is the builder, not the send site, and it cannot be dumped usefully
from an entry hook. The bytes that actually went out are however recorded exactly by the
stub, and they are highly structured:

```
[0]      0e        type (14)
[1]      02        version (2)
[2 ..17] all zero  (16 bytes)
[18]     ff 00     unset
[20]     ff 00     unset
[22]     00 00
[24]     ff 00     unset
[26]     ff 00     unset
[28]     00        (odd byte)
```

Read as big-endian u16s from offset 2 the body is `0,0,0,0,0,0,0,0, 0xff00, 0xff00, 0,
0xff00, 0xff00` - a **fixed 26-byte body with four `0xff00` markers**. The pattern says
these are fields the client has no value for yet (an unbound socket/port, or a
not-yet-known address), in the same way the earlier `-1` reading was an "unbound"
sentinel. So the request is a fixed record asking the server to fill in four slots, and
**the reply is presumably the same record with those slots populated** - a different shape
from the short 9-byte address record we have been sending, and the most concrete reason
the seven reply variants all failed.

The four `0xff00` fields sit at offsets 18, 20, 24 and 26, with a plain zero at 22
between them, so the record is not a simple array - there is structure around them worth
reading in the client's own writer before the next run.

**Corrected: the markers are 2-byte fields, not address slots (RUN MIRR).** The first
implementation of `--shape mirror` treated them as 6-byte address slots and a unit test
caught it immediately - offsets 18 and 20 are only 2 apart, so a 6-byte slot at each
overlaps. They are small `u16` fields with `0xff` meaning "unset" (ports/flags), and an
address cannot be pasted in. Implemented as `--mirror-fill zero|port|keep`:
`port` puts the mapped port (0x0c02) big-endian in each marker, `zero` clears them,
`keep` echoes unchanged. The reply is otherwise the client's own record, length and
header preserved:

```
request  0e 02 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 ff 00 ff 00 00 00 ff 00 ff 00 00
mirror   0e 02 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 0c 02 0c 02 00 00 0c 02 0c 02 00
```

Result: 21 probes, the reply delivered and well-formed, **still no BAP connection and
still no sign-on request**. So even answering in the record's own shape, with its unset
fields populated, does not move the client. That is the eighth reply variant ruled out,
and it strengthens the earlier conclusion rather than overturning it: the STUN reply is
not the gate, or the fields the client needs are not the ones carrying the `0xff` markers.

I would not spend more of this route on reply shaping. The evidence now says the
information needed is the *real* exchange - either a capture from the owner's own console,
or a much deeper read of the client's STUN success path (the `stw` stores in the live
jump-table branches at `0xb3724c`/`0xb372bc`/`0xb371e0`, none of which we have entered).

**WHERE THE REPLY IS ACTUALLY REJECTED (RUN BRANCH) - the real gate, at last.** Watching
the four live jump-table branches plus the table itself shows the handler almost never
reaches the table at all:

```
0xb36fd8  handler entry       : 13 hits
0xb370e0  the jump table      :  1 hit    <- one dispatch in an entire run
0xb371e0  index 15 (stores 17) :  0
0xb3724c  index 7  (stores 2)  :  0
0xb372bc  index 10            :  0
0xb374e4  index 9             :  0
```

So replies are not being mis-dispatched - they are **rejected before the dispatch**, on
almost every call. The test that does it is a few instructions into the handler:

```
0xb36fe8  lbz   r0, 4(r3)
0xb36ff4  cmpwi r0, 0
0xb3700c  beq   0xb3707c        ; == 0 -> carry on
   ... sets the field to 0x11 and returns (the "already have a result" path)
0xb37034  lwz   r0, 8(r30)      ; field at offset 8
0xb37038  cmpwi r0, 0
0xb3703c  bne   0xb3704c        ; non-zero -> carry on to the range check
0xb37040  lwz   r0, 0xc(r30)    ; field at offset 12
0xb37044  cmpwi r0, 3
0xb37048  beq   0xb3714c        ; == 3 -> carry on (the short-probe branch)
0xb3704c  bl    0xb35b38        ; otherwise: give up
```

and the captured session object has **offset 8 = 0 and offset 12 = 0** (the 32-byte hook
dump is `00000001 00000000 00000000 c0a80001 ...`). Both conditions therefore fail on
every call, and the handler bails through `0xb3704c` - which is why the jump table is
reached once in thirteen calls and why no reply we send has mattered.

**This finally names the two fields the stand-in has to influence**, and they are not in
the packet we were editing at all:

- `8(r3)` - a non-zero value here short-circuits the whole dispatch, and
- `0xc(r3)` - the value **3** is the other way through, via the short-probe branch.

Both are written by the client before it calls the handler, most likely by the routines
that fill in the address it just learned from our reply. So the chain is:
**reply arrives -> the client copies our address into its session object -> only then does
it reach the table.** Our replies are apparently not being copied, which is the first
thing in this whole investigation that is a *client-side* condition we can name rather
than a value we failed to guess. The next task is unambiguous: find the writers of
`8(r3)` and `0xc(r3)` - they are the code that consumes the reply body - and read what
they require of it.

**The loop is self-sustaining, and that closes the mechanism (reading `0xb35b38`).** The
bail-out the handler calls when the gate fails is not a passive return - it **resets the
state it just tested**:

```
0xb35b64  stw r27(=0), 0(r28)     ; clears the dispatch word
0xb35b68  stb r27(=0), 4(r28)     ; clears the byte at offset 4
0xb35b6c  stw r27(=0), 0xc(r28)   ; clears the very field the gate wants to be 3
```

So each failed call sets `0xc(r3)` back to 0, guaranteeing the next call fails the same
test, which clears it again. The client cannot escape by retrying - something has to
write a non-zero `8(r3)` or a 3 at `0xc(r3)` in the first place, and the only writer found
that does either is **jump-table index 15** (`0xb371e0`):

```
0xb371ec  stw r0(=1), 8(r3)      ; non-zero 8(r3) -> short-circuits the gate entirely
0xb371f0  stw r9(=0x11=17), 0(r3)
```

and our runs enter that branch **zero** times. The whole picture is therefore consistent:

1. the client sends a probe and waits;
2. our reply arrives and is handed to `0xb36fd8`;
3. the gate at `0xb37034` fails because the client has not yet accepted an address, so the
   handler bails at `0xb3704c`;
4. the bail clears the state, so the next attempt behaves identically;
5. the branch that would satisfy the gate (index 15) is never reached.

**The single remaining question is narrow and answerable:** what does the client require
in the reply for it to write index 15, or any `8(r3) != 0`? That is the reply-consumer
code, upstream of the handler, and the `0xb34ed8` / `0xb35398` hooks already watching that
path are the instrument. Everything else in this section - the eight reply variants - sat
downstream of a gate that never opened, which is why none of them could have mattered.

**ANSWERED - the reply consumer, `0xb35c94`, and the accept set (read, 21 instructions).**
This is the only writer of the gate field in the whole STUN region, and it is short enough
to read in full:

```
0xb35c94  mr    r11, r3
0xb35c98  cmpwi cr6, r4, 1
0xb35c9c  lwz   r0, 0(r11)      ; r0 = the reply's first word
0xb35ca0  cmpwi cr7, r0, 0
0xb35ca4  beqlr cr7              ; r0 == 0 -> return, no state change
0xb35ca8  cmpwi cr7, r4, 0
0xb35cac  beq   cr6, 0xb35ce4   ; r4 == 0 -> b 0xb35b38 (the bail)
0xb35cb0  cmpwi cr6, r0, 0x11
0xb35cb4  bnelr cr7
0xb35cb8  cmpwi cr7, r0, 0x0e
0xb35cbc  beqlr cr6
0xb35cc0  cmpwi cr6, r0, 0x0f
0xb35cc4  beqlr cr7
0xb35cc8  cmpwi cr7, r0, 0x10
0xb35ccc  beqlr cr6
0xb35cd0  beqlr cr7              ; (0x10 path lands here)
0xb35cd4  li    r0, 1
0xb35cd8  stw   r4, 8(r11)       ; <-- writes the gate field 8(r3)
0xb35cdc  stb   r0, 4(r11)       ; <-- sets 4(r3) = 1, the flag the handler reads first
0xb35ce0  blr
0xb35ce4  b     0xb35b38         ; the bail
```

So the reply is accepted **only if its first word is one of `0x0e, 0x0f, 0x10, 0x11`** and
`r4` is non-zero. On acceptance the client writes `8(r3) = r4` and sets `4(r3) = 1`, which
is exactly what the handler's gate at `0xb36fe8` (`lbz r0,4(r3)`) and `0xb37034`
(`8(r3) != 0`) require - so acceptance means the next handler call reaches the jump table.

**The accept set is `{14, 15, 16, 17}` and nothing else.** Two things follow, and both
matter:

- **18 and 19 are rejected.** That independently confirms the earlier reading that they are
  error states: the client's own reply consumer refuses them. The state 4 exit test
  (advance on 18 or 19) is therefore only reachable via the bail path, not via a good reply.
- **`0x0f` is in the set**, so the `typeversion` reply (type 15) we have been sending passes
  the type check. What is still unknown is `r4` - the value stored into the gate field - and
  where it comes from. It is a second parameter to `0xb35c94`, supplied by its three
  callers (`0xb337c8`, `0xb337f0` in `0xb3376c`, and `0xb33ec0` in the transport machine),
  so it is derived by the caller rather than from the packet, which is why no reply-side
  experiment could have moved it.

The next step is therefore to read the three callers and find how `r4` is computed. That
is the last untraced link between "a reply arrived" and "the client accepts it".

**`r4` is a success flag, not a parsed value - and the transport machine passes 1.** Both
remaining callers are short:

```
; the transport machine, 0xb33ea4 (the path that runs during a probe)
0xb33ea4  cmpwi cr7, r3, 0
0xb33ea8  bne   0xb33eb8
0xb33eac  lbz   r0, 0xa8(r31)     ; a status byte
0xb33eb0  cmpwi cr7, r0, 0
0xb33eb4  beq   0xb33ef4         ; still busy -> skip
0xb33eb8  mr    r3, r26          ; the session object
0xb33ebc  li    r4, 1            ; <-- r4 = 1, "this is a good reply"
0xb33ec0  bl    0xb35c94

; the 0xb3376c caller
0xb337bc  li    r4, 0            ; <-- r4 = 0, the bail path
0xb337c8  bl    0xb35c94
```

So `r4` is a **caller's verdict**, not anything parsed out of the packet: the transport
machine passes 1 when a reply has arrived and the status byte `0xa8(r31)` is clear, and
`0xb3376c` passes 0 to force the bail. The gate field `8(r3)` therefore just receives
"a reply was accepted", and the real condition for acceptance is entirely inside
`0xb35c94`: **the reply's first word must be 14, 15, 16 or 17.**

That is a testable, falsifiable statement, and it explains every observation in this
section:

- our `typeversion` reply (type **15**) and the `mirror` reply (type **14**) both have an
  accepted first word, so `r4 = 1` should have stored a non-zero `8(r3)` and set `4(r3) = 1`;
- yet the hook shows `8(r3) = 0` and `4(r3) = 0` on every handler entry, and the jump table
  is reached once in thirteen.

The two facts can only both be true if `0xb35c94` is **not being called with our bytes at
all** - i.e. the caller's `0xa8(r31)` status byte is still non-zero, or the call at
`0xb33ec0` is never taken, so the reply never reaches the consumer. That is a *pre-consumer*
condition, upstream of everything the stand-in can influence.

**This is the sharpest statement of the blocker so far, and it is checkable in one run:**
hook `0xb35c94` itself. If it is never entered, the reply never reaches the consumer and
the whole question is what keeps `0xa8(r31)` non-zero. If it is entered and the store
still does not happen, then the first word the client reads out of our packet is not what
we think it is - and the dump at that hook will say exactly what it is.

**CHECKED (RUN CONSUMER): the consumer is never entered. Zero hits.** With `0xb35c94`
(the consumer) and both call sites `0xb33ebc` / `0xb337bc` watched, alongside the
handler and the table:

```
0xb35c94  the reply consumer         :  0 hits
0xb33ebc  the transport call site    :  0 hits
0xb337bc  the other call site (r4=0) :  0 hits
0xb36fd8  the handler                : 13 hits
0xb370e0  the jump table             :  1 hit
```

**This closes the investigation and settles where the work has to go next.** The handler
runs 13 times, so the client *is* servicing the socket and *is* reaching the code that
decides whether a reply counts. But it never calls the one function that would record it
as accepted, and neither call site that could call it is ever executed. The rejection
therefore happens strictly **upstream of the reply consumer**, in the transport machine's
own logic between receiving the datagram and deciding to pass it on - most likely at the
status byte `0xa8(r31)` test at `0xb33eb0`, which is the one condition that can skip the
call entirely.

That also retro-justifies the whole session: eight reply variants, four types, two
versions, per-name addressing and a control run all produced byte-identical results
because **the stand-in's reply never reached the code that would judge it**. Not one of
those experiments could have succeeded, and no further change to `stun_stub.py` can
succeed either. The reply is not the lever.

**What the next session should do, in order:**

1. read the transport machine between `0xb33814` and `0xb33ec0` to find what sets
   `0xa8(r31)` and what else must hold for the `0xb33ec0` call to be taken;
2. check whether the datagram is parsed at all before that - the handler's 13 entries may
   be servicing a *timeout* rather than our reply, which would mean the socket is polled
   but the payload is never read;
3. only then revisit the reply body.

Point 2 is the cheapest test of all and should go first: make the stub's reply length
distinct and see whether the handler's entry count changes with it. If it does not, the
handler is on a timer and nothing we send is being parsed at all.

**DONE (RUN LEN7 vs LENBIG): the handler is on a timer, and the reply length is
irrelevant.** Two runs, identical except for the reply we send - a 7-byte `typeversion`
reply and a 29-byte `mirror` reply - and every number matched exactly:

| | probes | replies | handler `0xb36fd8` | table `0xb370e0` | `0x69ddb0` | sign-on | BAP |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| LEN7 (7-byte reply) | 17 | 17 | 13 | 1 | 400 | 0 | 0 |
| LENBIG (29-byte reply) | 17 | 17 | 13 | 1 | 400 | 0 | 0 |

The handler is entered **13 times on a fixed cadence regardless of what arrives**. So
those 13 entries are not "13 replies processed" - they are 13 timer ticks. The client's
own probe count is 17 and the stub sees 17, so probes do go out, but the reply path is
never driven by our datagram at all.

That is the sharpest possible statement of where the work is, and it is consistent with
the consumer never being called: **the client is not reading our replies.** The likely
causes, in order of how cheaply they can be checked:

1. the client expects the reply from an address other than the one it sent to - the stub
   answers from `127.0.2.2:3074` (its own bind), while the client's socket is
   `127.0.0.1:3074`, and a NAT-oriented client may require the reply to appear to come
   from the server it dialled. This is very likely, and it is a one-line stub change:
   answer the 4-byte probe from a socket bound to the *peer's* view, or check whether the
   client discards datagrams whose source address differs from the socket it connected
   to;
2. the reply must arrive before some deadline the timer path is already past, so the
   handler never re-reads the socket;
3. the client is waiting on a different port entirely.

Cause 1 is the one to test next, and it costs a single run.

**Cause 1 disproved by inspection; the client does receive the datagrams.** Reading
RPCS3's own network trace in RUN LENBIG settles three things at once:

```
5  x  sendto(s=52, len=4)    the 4-byte probe socket
12 x  sendto(s=53, len=29)   the 29-byte probe socket
16319 x bnet_recvfrom in total, of which 5472 on sockets 52/53
bnet_recvfrom(s=52, buf=*0xd018b46c, len=32, flags=0x0, ...)
```

- the addressing is already correct: the client sends from `127.0.0.1:3074` to the stub's
  `127.0.2.2:3074`, and the stub replies from `127.0.2.2:3074` to `127.0.0.1:3074` - exactly
  what a real NAT server does, so the source-address hypothesis is wrong;
- the receive buffer is **32 bytes**, and both reply sizes fit (7 and 29), so truncation is
  not the problem either;
- **the client is reading its socket thousands of times.** It receives our replies and
  does not act on them.

Also worth recording: the client never *connects* to port 3074 - the only outbound
connections in the whole run are 20 to `127.0.0.1:53` (DNS) and 3 to `<router>:1900`
(SSDP/UPnP). Sockets 52/53 are only ever `bind`-ed, and the probes go out via `sendto`
with the destination pre-set. So the STUN exchange is connectionless by design, which is
consistent with the client's own STUN client.

**Revised position.** The reply arrives, is read into a 32-byte buffer, and is discarded
without reaching the consumer. So the filter is inside the client, between `recvfrom` and
`0xb35c94`, and the only condition identified so far that can skip the call is the
status byte `0xa8(r31)` at `0xb33eb0`. Two concrete next steps:

1. hook the `bnet_recvfrom` wrapper's **caller** - or, easier, the first client
   instruction after the syscall returns - to see how many bytes came back and what it
   does with them, since the 5,472 reads and 13 handler entries do not line up and one
   of the two numbers is about something other than the reply;
2. read `0xa8(r31)`: who sets it, and what it means. If it is only cleared on a reply the
   client considers well-formed, then its writer is the real gate.

**Who writes `0xa8(r31)` (part 2, in progress).** The byte is not written inline in the
transport machine, so it has to be copied in as part of a larger record. Scanning the STUN
region for `0xa8(` references (in windows - a single large read silently decodes zero
instructions, the same trap as the capstone mode bug) gives:

```
0xb3d304  stw  r0, 0xa8(r31)
0xb3d674  lwz  r7, 0xa8(r31)
0xb3e494  lwz  r0, 0xa8(r29)
0xb3ee2c  stw  r0, 0xa8(r29)
0xb3f2d0  stw  r9, 0xa8(r31)     <- inside a struct initialiser
0xb3f4d4  lwz  r7, 0xa8(r29)
0xb3f4f0  stw  r7, 0xa8(r28)
```

`0xb3f2d0` is the interesting one: it sits in a run of stores that copies a whole result
structure into the object one field at a time -

```
0xb3f2b0  std  r0, 0xd0(r31)
0xb3f2bc  std  r9, 0xd8(r31)
0xb3f2c8  std  r0, 0xe0(r31)
0xb3f2c4  lwz  r9, 0x128(r1)     <- the status field, loaded first
0xb3f2d0  stw  r9, 0xa8(r31)     <- and stored into the object
0xb3f2d4  std  r0, 0xe8(r31)
```

It belongs to `0xb3e9e0` (817 instructions, one caller at `0xb3f71c` in `0xb3f6a4`) - a
**result-structure builder**. So the status byte the gate tests is one field of a record
produced elsewhere and copied wholesale, not something the receive path sets. That is
consistent with everything observed: the reply arrives, but the record that would carry
"a reply was accepted" is built somewhere we have not looked, and the transport machine
only consumes it.

**So the remaining trace is a single call chain:** `0xb3f6a4` -> `0xb3e9e0` (builds the
result) -> `0xb33ec0` (consumes it). Reading `0xb3f6a4` and whoever calls it should show
where a received datagram becomes that record - and therefore what the reply has to
contain to produce a status byte of 0.

**RUN CHAIN: the whole chain is dead. Zero hits at every level.** Watching all of it at
once, alongside addresses known to be live so the hook mechanism is demonstrably working:

```
0xb35c94  the reply consumer          :  0 hits
0xb33ebc  its transport call site     :  0 hits
0xb3e9e0  the result-record builder   :  0 hits
0xb3f6a4  the builder's only caller   :  0 hits
0x121128  the top of the chain        :  0 hits
0xb36fd8  the handler                 : 50 hits   <- live
0xb4695c  the request builder         : 60 hits   <- live
0xb34ed8  the field reader            : 37 hits   <- live
0xb35398  the receive path            :  5 hits   <- live
0x69ddb0  the gate getter             :400 hits   <- live
```

The chain from `0x121128` down to the consumer **never executes**, while the addresses
immediately around it fire constantly. So the result record is not built, the status
byte is never written, and the consumer is never called - which is a complete,
self-consistent explanation of the stall that needs no further speculation:

> the client receives the datagram and runs its receive path, but never produces the
> "a reply was accepted" record, so it keeps the probe loop open.

**What this rules out.** The whole reply-shaping avenue, definitively: the code that would
judge a reply is downstream of a builder that never runs, so no byte we could put in a
packet changes anything. That is why nine variants produced identical results, and it is
why this should not be attempted again.

**Where the work actually is now.** `0x121128` is 569 instructions with one caller, and
its first action is `lbz r3, 1(r3)` on the value built at `0x19e260` by
`lis r3,0x1a2` / `addic r3,r3,-0x3da0` - a **module-state flag tested against 0**, i.e. a
lifecycle check ("is the STUN module idle?"), not a packet test. Since the chain is
one-caller-deep, the most likely reading is that `0x121128` is a *startup/registration*
routine for the STUN module rather than a per-datagram path, and that the per-reply work
happens on a different, so-far-untraced branch.

Next session's first task, therefore, is no longer the reply body. It is to find what
*does* call the consumer in a working case - and the fastest way to get there is a
capture of a real 01.24 STUN exchange from the owner's own console, which would show
both sides of an exchange that the client actually accepts. Everything above is a
faithful reading of the 01.24 binary and every run so far, but it is inference from one
failing configuration, and this last result says the missing piece is not in the reply
path at all.

**The consumer has a FOURTH caller, and this one is inside the machine that runs.** The
call sites are `0xb337c8`, `0xb337f0` (both in `0xb3376c`), `0xb33ec0` (in the transport
machine) - and `0xb33934`, which is in the *same* function as `0xb33ec0` and therefore
does execute:

```
0xb33920  nop
0xb33924  cmpwi  cr7, r3, 0x13     ; the getter's result == 19?
0xb33928  bne    cr7, 0xb33880     ; no  -> straight back to the shared tail
0xb3392c  clrldi r3, r29, 0x20
0xb33930  li     r4, 1
0xb33934  bl     0xb3376c          ; -> 0xb337a0 sets r4=1 -> the consumer
0xb33938  b      0xb33880
```

This closes the loop and explains the whole session. The **real** progress path is:

1. `0x69ddb0` returns **19**;
2. the transport machine branches on exactly that (`cmpwi r3, 0x13`) to `0xb33934`;
3. which calls `0xb3376c`, which calls the consumer with `r4 = 1`, which stores the
   gate field and sets the flag.

So the earlier readings were each right and the overall shape was wrong: 19 is not an
error state to be avoided, it is **the value that unlocks the consumer**, and 18 is the
parallel branch of the same test at `0xb339d8`. Our runs never reach either because the
getter returns 0, and the getter returns 0 because the result record is never built
(`0xb3e9e0`, zero hits).

The `stw r0, 0x13` at `0xb370b0` that made 19 look like an error was the *same* constant
reached by a different route - the handler's own out-of-range path - and the two meanings
coincide numerically. That is why 19 could be read as either. The disassembly of the
caller is decisive where the store alone was not.

**This does not change the conclusion about the stand-in, but it sharpens it enormously:**
the gate is the *getter's* value, the getter is fed by a result record that is never
built, and the record is built by a chain with one caller that never executes. The
question for the next session is now precisely "what makes `0xb3e9e0` run", and it is a
much smaller question than "what should the reply contain".

**The chain is reached from the engine's tick, and the whole path has one caller per
level** (so it is a straight line, not a vtable):

```
0x8adbf4  bl   0x11cc94   ; in 0x8ad930
0x11cca0  bl   0x121128   ; in 0x11cc94   (a shim: bl + mtlr + blr)
0xb3f71c  bl   0xb3e9e0   ; in 0xb3f6a4    (the result-record builder)
0xb33ec0  bl   0xb35c94   ; in 0xb33814    (the consumer)
```

and the top of it checks the return:

```
0x8adbf4  bl     0x11cc94
0x8adbf8  cmpwi  r3, -1        ; failure?
0x8adbfc  stw    r3, 0x34(r24)
0x8adc00  beq    0x8adcc0
```

`0x8ad930` is nowhere near the STUN code, so this is **the engine's own update tick** -
which is a useful thing to know, because it means the builder is not gated on a
STUN-specific condition at all. It is on the main loop reaching a state where it decides
to poll the network. A `-1` return means "nothing to do" and the tick moves on.

So the chain is: engine tick -> STUN module poll -> result-record builder -> (needs a
result) -> consumer. The builder runs only when the module believes it has something
pending, and in our runs it believes it has nothing - which is consistent with the
status byte being zero from the very first probe and the module simply waiting.

**That is the honest bottom line of this session, and it is a real narrowing:** the
stand-in's replies are received (5,472 `recvfrom` on the STUN sockets), the receive path
runs, the engine tick runs, and the only thing never happening is the module deciding it
has a result. Nothing in the packet can change that decision, because it is made before
the payload is interpreted. Closing this needs a capture of a real 01.24 STUN exchange
from the owner's console to see what makes the module decide it has a result; more
reply variants cannot, and the notes now say so explicitly so the next session does not
repeat them.

**RUN TICK corrects the previous paragraph: the engine tick never fires either.** Adding
`0x8ad930` and its shim to the watch list, with `0xb36fd8` again proving the hooks are
live:

```
0x8ad930  the engine update tick     :  0 hits
0x11cc94  its shim                   :  0 hits
0x121128  the STUN module poll       :  0 hits
0xb3f6a4  the builder's caller       :  0 hits
0xb3e9e0  the result-record builder  :  0 hits
0xb35c94  the reply consumer         :  0 hits
0xb36fd8  the handler                : 13 hits   <- live
```

So the whole chain from the engine tick down is **dormant** - not declining a condition,
never entered at all. That is a cleaner and more useful result than "a gate is failing":
the code that would judge a reply is not being executed because a *higher-level*
dispatch is not reaching it, and that higher-level dispatch lives in the engine, not in
the STUN module. The handler `0xb36fd8` that does run is on a separate, timer-driven
path, which is why it fires 13 times while everything else is silent.

**Revised bottom line, and it is the most important thing in this section:** the 01.24
client's STUN exchange in our runs runs entirely on a **timer/callback path**, and the
**data-driven path that would consume a reply is never entered**. That is why no reply
content could ever matter, and it is not a detail of the reply format - it is a different
execution path in the client that our environment never triggers. Two things follow:

1. the next investigation should be *what enters that data path*, not what the reply
   contains. `0x8ad930`'s caller, and the condition guarding it, is the place to start;
2. it is worth asking whether a real server's reply would enter it at all, or whether it
   is entered by a *socket read callback* the stand-in's UDP traffic does not provoke
   (for example because the real reply arrives while the client is blocked in a receive
   rather than on a poll). That is exactly the question a single capture from the owner's
   console answers, and nothing in the stand-in can.

**SUPERSEDED BY RUN PRESS - THE CLIENT WAS AT THE TITLE SCREEN THE WHOLE TIME.** This is
the answer to all of the above, and it is embarrassingly simple. Every run in this section
launched the game and then **never pressed Cross**. 01.24 reaches the title screen and
waits there; the stand-ins are only contacted *after* Cross, exactly as
`Invoke-TitanLogin.ps1` drives 01.00 ("Cross at the title until the BAP stand-in answers
sign-in"). So every "no sign-on request" observation was the client sitting at a prompt,
and every conclusion about the STUN path was drawn from a run that had not started.

RUN PRESS (adding a Cross press to the run wrapper, `-PressSeconds 330 -Shot`) changed
everything on the second press:

```
13:24:59  Cross #1
13:25:27  Cross #2
progress: sign-on requests=13 bap=1
```

and the sign-on log shows the full chain completing for the first time on 01.24:

```
[443#1] ClientHello: SSLv3, 3DES/RC4/DES/export, no SNI, no extensions
[443#1] SSLv3 established with cert for www8.signon.deadorbit.net: RSA_RC4_128_SHA
[443#1] HTTP 'POST /SignOn?platform=ps3_ppu&build=51539.16.07.26.1808.v230' host=www8...
[443#1] replied 200 SignOn success (246 bytes): session 127.1.1.127:12850
[80#2]   HTTP 'GET /manifest HTTP/1.1' host='content.local.test'
[80#2]   replied 200 content manifest (15964 bytes, unsigned)
```

So on 01.24, with the existing stand-ins and **no changes to any of them**: SSLv3
handshake succeeds, **SignOn returns 200** and hands the client its BAP session, and the
**content manifest is served**. Note the build string: `51539.16.07.26.1808.v230`, which
confirms 01.24 (v230) is what is running.

**What this means for the notes above.** Everything in this section about the STUN gate,
the reply consumer, the module descriptor and the engine tick describes code that
genuinely was not executing - but the reason it was not executing is that the client had
not been told to leave the title screen, not that the reply was malformed. The nine reply
variants are not disproven so much as *irrelevant*: the title screen was in the way. The
lesson to record is procedural and worth more than any of the reverse engineering: **an
unattended 01.24 run must press Cross**, and `Invoke-TitanLogin.ps1` already does this
for 01.00, so the next session should start from `Invoke-Final124Run.ps1 -PressSeconds`
rather than the bare smoke launcher.

**New state, and the next real blocker.** With sign-on working, the client now has its
BAP session address and the content manifest, and the next gate is BAP itself - the
stand-in log still shows only its "listening" banner, so nothing has connected to
`127.1.1.127:12850` yet. That is a real, well-defined blocker and it is where the work
should go next.

**Confirmed visually (RUN PRESS3).** Screenshots taken during the pressed run settle what
the client is doing, since the network logs alone could not:

- `13:25:10` - the **BUNGIE logo on black**, i.e. the client is in its boot sequence
  *after* the title prompt, not sitting on the prompt;
- `13:27:34` and `13:29:27` - fully **black**, the content-manifest load / next stage.

And the traffic over the whole 660-second pressed run: **one** SSLv3 SignOn, **one**
content manifest, then nothing - no retry, no BAP connection. So the client completes
sign-on cleanly, fetches the manifest, and then stops, still without touching
`127.1.1.127:12850`.

That is a very different picture from a few hours ago and it reframes the remaining work
precisely:

- the whole STUN investigation is **moot** - it ran before the client was ever started,
  and the data-driven path it analysed is presumably the one that runs *after* sign-on;
- sign-on and the content manifest are **solved on 01.24** with the existing stand-ins,
  including the two patches and the 1,034 update packages;
- the next gate is the BAP connection, and the black screen plus the complete absence of
  traffic to 12850 says the client is either still loading the manifest's packages or has
  stalled before dialling BAP.

**Next session's first task:** run with `-PressSeconds` set past sign-on, watch for the
first connection to `127.1.1.127:12850`, and if it does not come, hook what decides to
dial BAP (the session address is already delivered in the SignOn reply, so the stand-in
is being given the right endpoint). The `PS3_UPDATE` packages and the manifest coverage
are the obvious suspects now that the client is actually getting this far - if it is
loading the 1,034 packages on this machine that is a long, silent wait, and the answer may
simply be more time rather than a new blocker.

**The STUN work was not wasted - it is the NAT punch that precedes BAP.** A string sweep
of the 01.24 EBOOT for `BAP` shows why the client wants a working STUN exchange *before*
it dials the session server:

```
0x012fa560  'Primary outbound BAP connection failed during start_connection'
0x012fa5a0  "BAP connection failed due to hitting timeout '%lld'ms"
0x01304df0  'bap message queue heap'
0x01306cf0  'network_bap_nat_punch_queue'
0x01307078  'waiting_for_bap_refresh'
0x0134e810  '_connection_failure_failed_to_decode_bap_header'
0x0134f000  'BAP connection %d'
0x01357798  'investment_bap_connection_identifier'
0x0134fad0  'b->c shello req'
0x0134fae0  'b->c shello rsp'
```

`network_bap_nat_punch_queue` is the link: Demonware records the client's perceived
address from the STUN exchange and **punches a NAT hole** before the BAP connection is
attempted, and `waiting_for_bap_refresh` says it blocks on that. So the STUN reply is
*not* a side quest after all - it is an input to dialling BAP, which is why the two
looked unrelated from the logs.

That reframes the earlier STUN work usefully rather than making it moot. The reply
format itself was never the problem, and the reply consumer (`0xb35c94`, accepting first
words 14-17) is still the right place to look - but it is on the *pre-sign-on* side of a
conversation whose outcome only matters once sign-on has happened. The correct order for
the next session is therefore:

1. press Cross, get sign-on and the manifest (now working);
2. then re-run the STUN experiments **in that context**, where a NAT punch completing
   would show up as the first connection to `127.1.1.127:12850`;
3. if it does not, the gate is whichever of `0xb35c94`'s preconditions the client is not
   meeting, and the `bap_nat_punch_queue` string gives a hook target for the code that
   consumes the punch.

**The guard on the dormant path, one level up (`0x8aedc8`, one caller at `0x8aee28`).**
The tick is not called unconditionally; the caller tests two fields of a module
descriptor before entering:

```
0xaedd8  lis   r31, 0x1b7
0xaeddc  addic r31, r31, -0x5280   ; r31 = the module descriptor (relocated at runtime)
0xaede0  lbz   r3, 0xc(r31)        ; a flag byte
0xaede4  clrlwi r3, r3, 0x1f       ; clear the low 5 bits
0xaede8  cmpwi r3, 0
0xaedec  beq   0x8aee40            ; zero -> skip the whole path
0xaedf0  lwz   r3, 0x28(r31)       ; a count
0xaedf8  cmpwi r3, 0
0xaedfc  ble   0x8aee40            ; <= 0 -> skip
```

So the entire data-driven path - tick, STUN module poll, result builder, consumer - is
behind **two conditions on one module descriptor**: a flag at `+0xc` with its low 5 bits
masked off, and a non-zero count at `+0x28`. In our runs neither holds, which is why
every one of those functions returns zero hits.

The descriptor address cannot be resolved statically (`lis`/`addic` is a relocated
pair, and reading it gives code rather than data), so the descriptor's *identity* has to
be found by following what writes `+0xc` and `+0x28`. **That is the concrete next task
and it is small:** find the writer of the flag at `+0xc`, and its two conditions tell the
next session what state the client must reach before it will look at a reply at all. This
is a better starting point than anything in the section above, because it is upstream of
all of it and it is a single field.

**The `ENOTCONN` noise is a red herring, now confirmed twice.** RUN SHAPEBIN logged 227
`Socket error ENOTCONN` and 614 `sys_net_infoctl` lines. The `infoctl` calls are
RPCS3's own socket polling (`cmd=53`, `cmd=9`) on its UDP socket, not our traffic, which
matches what the earlier 01.24 runs already found. Chasing socket-error counts will not
move this forward; the signal that matters is "did the client resolve a new hostname" and
"did anything connect to the BAP stand-in", neither of which has happened.

**Toolchain note.** `scripts/d1_eboot.py disasm` had never worked: it requested
capstone `CS_ARCH_PPC` + `CS_MODE_64`, which capstone does not support and which
silently yields zero instructions. Fixed to `CS_MODE_32 | CS_MODE_BIG_ENDIAN`, and the
address arguments now accept a `0x` prefix. `capstone` must be pip-installed for
`disasm`; nothing else in the tool needs it.

**Passing array arguments to these scripts.** `Start-LocalStack.ps1`/`Start-FinalBuildSmoke.ps1`
take `-StunArgs`, but it must be one **semicolon separated** token. Under `pwsh -File`
a second value of a `[string[]]` parameter binds to the *next* parameter instead of
joining the array, so `-StunArgs '--reply-index' '0'` silently passed `0` as
`-PpuHash` and preflight failed with `FAIL invalid PPU hash`.

**Core activity-host payloads (static encode/decode checks):** the first audit found 01.00
offsets that the 01.24 encoder silently omitted. The stand-in now derives these fields from
the selected schema:

| Field | 01.00 | 01.13/01.24 |
| --- | ---: | ---: |
| Join result / session ID | +0xf8 / +0xf9 | +0x100 / +0x101 |
| Activity scene world / map | +0x990 / +0x994 | +0x5c8 / +0x5d0 |
| Simulation-player owner | +0x18 | +0x3c |
| Simulation-player spawn flag | bool +0x48 | nested bool +0x6c/+0 |
| Game-mode trailing signed value / hash | +0x984 / +0x988 | +0x4c8 / +0x4cc |

The core preflight also roundtrips the world-state zone and peer masks, starter
gear, all three unlock tables, and both mission scripts against the saved 01.00
schema and the corrected 01.13/01.24 exports. It verifies serialization; the
01.24 client handlers and the wrapped spawn flag still need a game run.

The EBOOT-only `types_01.24.json` has no web-service ID table. The BAP stand-in
takes `--services-schema` to overlay the package-derived 01.24 map in the data
repo (`ps3_updates/services_01.24.json`). It covers all 94 service IDs;
`scripts/d1_build_preflight.py` checks their type handles before a run.

**Web services (CONFIRMED):**

- The table moved from the 012c tag 0x80a5806b (now an empty 60-byte record) to tag
  **0x80a5e21a in package 012f**. The investment root is still 0x80a58000, and most of its
  children now point into 012f.
- The record format (36 bytes, element class 0x80801820) is unchanged. There are 94 services,
  up from 66.
- **All 66 old services keep their wire ids and their type handles.**
- 28 are new:
  - 208, 209, 211;
  - 406;
  - 804;
  - 904, 905;
  - 1228–1233;
  - 1305–1307;
  - 1601, 1602, 1606, 1609–1614;
  - 1701, 1702;
  - 1801.
- Static response shapes put **208, 209, 211, 1305, 1306 and 1601** first on
  the capture list: their replies carry fields beyond the common result/status
  shape. The other 22 new replies have only a result field or a result plus a
  32-bit status field. This says nothing yet about which services the client
  calls or what values it expects.
- 19 old services changed a request or response payload (often only through a nested type).
  Those the stand-in answers: **104** universe (both), **503** sign-in (response `0x80801825`
  0xe0 → 0x3c8), **201/202/203/205** (responses). Service **206** (family fetch) is unchanged.
- Service 104 no longer carries the universe object: 01.13 has a result-only
  response and 01.24 adds a bool at +0x8. The stand-in now sends the universe
  object there only when the selected response schema has that struct.
- Service 503 still carries the universe object at +0x18. In 01.24 its flag
  override list grows from 20 to 50 entries and its optional fields expand.
  The former hand-written sign-in reply decoded a populated override list
  incorrectly on 01.24. It now uses the selected schema for that reply.
  The build preflight checks populated sign-in and service-104 replies on
  01.00, 01.13 and 01.24.
- `d1_types.py --packages <012f chain folder> --oodle <DLL> --service-tag 0x80a5e21a
  service-map services_01.24.json` exports the mapping and name hashes. The four update
  files `_1` through `_4` suffice because the table lives in `_4`; unrelated entries can
  refer back to the disc's `_0` package.

**Sign-on (CONFIRMED):**

- The nanopb success message (field 2) is identical, so the stand-in's reply format still
  fits.
- New optional fields:
  - request 20 (8 integers) and 21 (five strings of 64/64/128/32/32 bytes);
  - response 13 (string[64]) and 14 (u32).
- The request table is at 0x12e66dd and the response table at 0x12e6850. The handler is at
  0x28dbd4 (01.00: 0x126744). It decodes into a stack struct at r1+0x70:
  - field 13 is a **serialized blob, not a bare string**; its real location in the SignOn
    singleton is **+0x4460 (u64 pointer) and +0x4468 (u32 length)**, not +0x47cc as first
    written here. The store is at `0x28e170`/`0x28e174` (`std`/`stw` on the singleton base
    `r30`), guarded by `lbz r3, 0x508(r1)`; `0x28e6ec`/`0x28e6f0` write the pair again from
    another path. (The only literal `0x47cc` stores in the build are unrelated singletons at
    `0x1aa`, `0x1ab` and `0x1bc`; find them with `scripts/d1_offset_scan.py`.)
  - field 13's consumer is a **single accessor and a single caller**, which makes its purpose
    knowable offline:
    - `0x35af6c` is the accessor: `332614(3)` fetches the SignOn singleton, then
      `lwz r31, 0x4468(0x1aa49f8)` returns the blob **length** (0 if absent).
    - its one caller `0x490474` is a **3-iteration loop** (`cmpwi r30, 3`, `r31 += 0x1a0`), so
      the blob is parsed once per region. It first compares the incoming length against the
      cached `+0x468`; if it differs it reparses, else it skips — the same "only act when
      something changed" guard seen elsewhere in this codebase.
    - on a change it copies **0x178 bytes** into `+0x470` (`9a7540`, then `99e380`) and records
      the new signature: `+0x450` (u16 version), `+0x458` (bool), `+0x460` (f64), `+0x468`
      (length), `+0x46c` (u8). Only the success path bumps the object's own counter at
      `+0x5e8`, saturating rather than wrapping.
    - the actual decode is `0x490174` (also reached via `0x48ff28`): it dispatches on a type
      byte at `+0x1c`, and when that byte is 0 it takes the "write defaults" path, clearing
      `+0x454`/`+0x458`/`+0x460`/`+0x468`/`+0x46c` first.
    - the loop's caller is `0x5b4bb8`, which requires `475d50() == 3` (a count of **three**,
      matching three regions) and a guard byte at `+0x5080`; afterwards the same function walks
      **ten** 0x240-byte records (`cmpwi r31, 0xa`) through the expiry helpers
      `48eedc`/`48f2dc`/`48ef24`, which compare `+0x21c`/`+0x238` counters — those are timers,
      not host names.

    **Consequence for the stand-in:** an absent or empty field 13 is handled (length 0, the
    cached signature cleared and defaults written), so field 13 is not *required* for the
    client to continue. But the 0x178-byte record is the only sign-on-supplied configuration
    01.24 has left now that the Demonware lobby step is gone (see `Demonware lobby (LIKELY
    removed)` below). The region's `0x1a0` stride suggests one blob per region holding that
    region's endpoints. **The blob's contents are still undecoded**; what is confirmed here is
    the plumbing, not the schema.
  - field 14 is wrapped in an address record (u32, then u16 4 at +0x12) and handed to the
    `network_demonware` module (0x1326e8 returns it; 0x132cb4 validates and stores it at
    +0x184). Its STUN/relay code (0x132cf4) compares it with the module's own address (+0x154)
    and records whether they match, then passes it with a port to 0x1197bc.

  This is LIKELY the client's public IPv4 address as the sign-on server saw it, used for NAT
  traversal. `signon_probe.py --public-ip ADDR` sends it (optional; the byte order is not
  confirmed). **This is the one new field the failing 01.24 runs already send** (as
  `127.0.0.1`), and it feeds the STUN/relay/NAT path, so it is now the strongest single
  candidate for the "Passed Verifying Content to Preparing, then black screen with no BAP"
  stall. Its byte order is still unconfirmed, and a wrong one may be worse than omitting it:
  `0x132cf4` compares the value with the module's own address at `+0x154` and records the
  match. **Next experiment for the next PC session: run the same manifest with field 14
  omitted entirely (the `--public-ip` flag already makes it optional), so only one variable
  changes.**
- `SignOn?platform=%s&build=%s` and the two sign-on domains are unchanged.
- New: `stun1`–`stun8.signon.deadorbit.net` / `.gravityshavings.net` and
  `destiny-stun.{us,eu,jp,au}.demonware.net`. The DNS sinkhole already answers every name
  locally.

**Demonware lobby (LIKELY removed):**

- 01.24 has none of 01.00's `panther-ps3-{lobby,auth,auth3}.{prod,cert,dev}.demonware.net`
  names. It also lacks the strings of the 01.00 lobby client module around 0x42c9d8
  (`online_demonware memory`, `failed to connect to lobby`, `platform account id changed`,
  `BLUS30843`). 01.13 still has all of them.
- The Demonware library (version 44: bdCore, bdSocket, bdLobby/bdAuthService, bdNet) is still
  linked, and its strings now mention NAT punch, QoS probes and STUN.
- So the final client probably goes from sign-on straight to BAP. The auth3 → lobby 3074 step
  that 01.00 needs would be gone.
- How (and whether) 01.24 reaches a Demonware auth server is UNKNOWN. Candidates are response
  field 13 or the title configuration. **Check on the first boot:** log DNS queries and
  connections after sign-on.

**BAP (LIKELY unchanged):** the message names (`shello`, `sub req`/`sub rsp`, `qz->c`, `ws`)
are all present. A `network_bap_nat_punch_queue` was added.

**Error ids:** 01.24 drops the `error.*` string table (248 names in 01.00 → 1). An error code on
screen is therefore no longer found by string. Map it through the 01.00 table where the meaning
carried over.

**Patches for RPCS3 (CONFIRMED addresses; the PPU hash was derived offline):**

- The embedded sign-on CA is the identical 2,390-byte PEM ("Tiger Root Authority" + GeoTrust) at
  **0x12d3740**, used at 0x113894 (01.00: 0x11b7218).
- The content-manifest signature flag is **0x18dca64** (initially 1), read only at 0xc65d14,
  which has the same `cmplwi 0x108` shape as 01.00's 0xb6f3cc (01.00: 0x179d5f4).
- `game_ca_patch.BUILDS["01.24"]` holds both and the calculated key
  `PPU-ac58999f254e8d9dc817426c8c84b068ee8caaea`.
  `d1_ppu_hash.py` matches the known 01.00 key exactly. The algorithm follows
  [RPCS3's executable loader](https://github.com/RPCS3/rpcs3/blob/master/rpcs3/Emu/Cell/PPUModule.cpp):
  hash every ELF program header's type and flags, then each nonempty LOAD segment's
  address, memory size and file bytes. The first live boot should confirm the 01.24
  key in `RPCS3.log`.

**Packages and DLC (CONFIRMED strings):**

- The patch folder is built as `%s/game/%s/USRDIR/%08x/` with `/dev_hdd0` and the constant
  0xa0000001 (0xc951c, a case of the path-root switch at 0xc8758), i.e.
  `/dev_hdd0/game/<title>/USRDIR/a0000001/`.
- Other new path strings: `packages/%d\`, `%s_%s.pkg`, `unpatchable\packages`,
  `client_bootstrap_patchable` / `client_bootstrap_unpatchable`, and
  `Patchable/Unpatchable package registration failed with result %s (%d)` (0x366b88).
- New DLC strings:
  - `DLC Patch/CDN/Cache/Entitlement/Marketplace Containers`;
  - `dlc_installer error 0-7`, `dlc_url 0-7`;
  - `Apply quick patch (package='%s', is_asset='%s')`;
  - `World-transition requires DLC.` / `... requires permissions.`;
  - `bootflow:illegal_dlc_media`;
  - `bootflow:check_media: NPDRM check failed`;
  - `offer_key.edat`, `destiny.edat`.

**DLC entitlement gate in 01.24 (CONFIRMED, `0x36e92c`; the 01.00 twin is `0x334004`):**
this is the function that emits the two strings above. It is the gate that
[`bap_server.py`](../server/bap_server.py) already works around on 01.00 through
`--self-flags` (see `0x4105c4 world_transition_dlc_check`, the unlock-expression evaluator
that feeds it, and the orbit checkpoint's world 125/126 runs), so this is a relocation, not a
new mechanism. What is new is the shape in 01.24:

- `+0x660`, `+0x668`, `+0x670` are three optional DLC reference slots on the transition
  object, tested as `clrlwi`/`cmpdi` against 0 in that order, producing a **priority code**:
  0 = all three set, 1 = `+0x668` and `+0x660`, 2 = `+0x670` only, and `r28` stays -1 when none
  is set. The priority, not the slot, selects the path.
- `2a8080(4)` fetches the entitlement/permission service; if it is absent the check is skipped.
  Otherwise two virtual calls on it (vtable +0x2c and +0x8, the latter returning a bool at
  `+0x6b8c`) must both be true.
- `35cd7c` writes the resolved **licence id** into the caller's `r1+0x70`. `470384` returns
  **true when an id was resolved** and 0 only for -1 (it counts leading zeros of `id + 1`),
  i.e. it answers "does this transition name a DLC at all?". A resolved id takes the failure
  branch: `355d54(0x19, 0x17, 0x12fcc20)` ("World-transition requires DLC."), returning
  0x17 = 23. **Both pointers come from `lis r5, 0x130` + `addic r5, -0x33e0`/`-0x33c0`;
  `0x130cc20`/`0x130cc40` are the neighbouring UTF-16 Portuguese copies in a localization
  table, not the ones this code passes.**
- With no DLC named (id -1) the same service is asked again through `46db2c(licence)`, which
  returns non-zero to mean "permission granted". Zero reports
  `355d54(0x19, 0x18, 0x12fcc40)` ("World-transition requires permissions.") and returns
  0x18 = 24, matching the 01.00 observation that 0x17 is the DLC gate and 0x18 the next one.
- Only the success path sets `+0x30` of the transition object, resolves the activity handle
  through `337540`/`3375c0`, checks `332614(3)` and `359ec4() == 2`, and returns 1 at
  `0x36ec28`.

So the gates are 0x17 = "this world names a DLC you must own" and 0x18 = "you do not hold the
permission for it", and both read only client-side state: the transition object's DLC slots and
the local entitlement enumeration. The activity host is never consulted, which is why the
existing `--self-flags` answer for 01.00 carries over. The open item "DLC entitlement check" in
[`preservation-plan.md`](preservation-plan.md) is resolved for 01.24 as far as offline work
goes; what is left is a runtime confirmation that the 01.24 client evaluates the same unlock
expressions the stand-in already serves.
- New bootflow states: `check_media` and `confirm_account_transfer`.

## Reports

The data repo's `diff_*.md` and `types_*.json` were regenerated with the
current scanner and one-to-one matcher. To repeat that export:

```
python scripts/d1_types.py --elf <01.24 EBOOT.elf> schema types_01.24.json
python scripts/d1_builddiff.py --old-elf <01.00 EBOOT.elf> --new-elf <01.24 EBOOT.elf> --names 01.00,01.24 \
    --map-out handles_01.00_01.24.json --out diff_01.00_01.24.md
python scripts/ps3_update.py fetch BLES01857 --versions 01.13,01.20,01.22,01.23 \
    --match ps3_investment_globals_012f_1.pkg,ps3_investment_globals_012f_2.pkg,ps3_investment_globals_012f_3.pkg,ps3_investment_globals_012f_4.pkg --out <dir>
```

The final 012f chain is _1 (01.13), _2 (01.20), _3 (01.22) and _4 (01.23). 01.19's _5 is stale.
The extracted tag has 94 unique IDs and all 66 old wire IDs and request/response
handles match the saved 01.00 map exactly. All 188 handles are present in the
corrected 01.24 type export. The derived map is in `d1xenia-data`; the raw
update packages remain in local scratch storage.

**First-run manifest question (offline count):** The existing stand-in builds its content
manifest from the disc's package folder. With the `en` language filter that yields 160
package IDs. The non-superseded update inventory has 310 `en` or language-neutral IDs,
152 of them absent from the disc set (312 distinct IDs in the union). Six required update
filenames have the special `_unp1`/`_unp2` suffix and do not have a hex package ID in the
name; those are unpatchable bootstrap/UI packages. A sampled 01.23 package
(`ps3_investment_globals_012f_4.pkg`) retains the same header ID/value shape used by the
disc manifest generator. Whether the client registers these update-only IDs from the
content manifest or separately from installed patch metadata is **UNKNOWN**. On the first
run, preserve the sign-on manifest reply and package-registration lines before changing
the manifest; this count narrows a likely failure if the new content does not load.

**RUN LONG1 - the loop is visible now that sign-on works.** A 25-minute pressed run with
sign-on reached, the timeline is unambiguous from the log timestamps alone:

    13:42:46  stack up
    13:43:29  Cross #1        (booted past shader compilation)
    13:43:34  resolves the 20 STUN names
    13:43:58  Cross #2
    13:44:06  resolves www5.signon.deadorbit.net  -> sign-on
    13:44:07  resolves content.local.test        -> GET /manifest, 200
    13:44:00 onwards   STUN probes continue, every 15 s
    13:48:55  30 probes / 29 replies / 13 sign-on / BAP: still just its listening banner
    13:50:06  the STUN log is still being written

The connect targets over the whole run are `127.0.0.1:53` (23), `<router>:1900` (9),
`127.0.1.5:443` (1) and `127.0.0.1:80` (1) - **and never `127.1.1.127:12850`.** The client
never even attempts BAP.

**So the picture is now complete, and it is a single loop:** sign-on succeeds, the manifest
is served, and then the client goes back to STUN and stays there - 34 probes, 33 replies,
BAP never touched - while Cross is pressed throughout (22 presses, no effect, because it
is not waiting for input). This is `network_bap_nat_punch_queue` never draining:
Demonware will not open the BAP connection until the punch completes, and the punch is
exactly what the unaccepted STUN reply prevents.

That finally joins both halves of the session into one causal chain, and it is testable in
a way the earlier runs were not, because now sign-on and the manifest are out of the way:

> sign-on (solved) -> manifest (solved) -> **STUN reply not accepted** -> NAT punch never
> completes -> `waiting_for_bap_refresh` never returns -> BAP never dialled.

**The next experiment is narrow and well-posed:** with Cross pressed so the client gets
past the title screen, re-run the STUN reply variants and look for the first connection to
`127.1.1.127:12850` rather than for the field values. `bap_server.py`'s framing is
documented as taken from 01.00, so if a punch does complete, the shello handshake is the
next thing to verify.
**RUN PUNCH sweep - three variants, all identical, and the signal is now end-to-end.**
`Invoke-Final124PunchSweep.ps1` presses Cross until sign-on is clearly done (so the
title screen is out of the way) and then watches for the first TCP connection to the
BAP stand-in, which is the outcome that matters rather than any field value:

| variant | reply sent | presses | sign-on | probes | BAP log lines |
| --- | --- | ---: | ---: | ---: | ---: |
| `none` | never answers (`--reply-index -1`) | 2 | 13 | 33 | 1 |
| `mirror` | the request's own 29-byte record, `ff00` slots filled with the port | 3 | 13 | 32 | 1 |
| `typeversion` | 01.00-style `0f 02 00 7f 00 00 01 02 0c` | 3 | 13 | 32 | 1 |

Sign-on is 13 in all three, so the title screen is genuinely out of the way every time,
and the BAP log is one line - its listening banner - in all three. **The client never
connects to `127.1.1.127:12850` in any variant.**

That includes the `none` control, which is the important one: it behaves the same as
answering. So on the evidence available the STUN reply is not what decides whether BAP is
dialled - a silent stub and an answering stub are indistinguishable from BAP's point of
view. Something between the manifest and the dial is failing, and it is not the reply.

**What is now known, and what is left.** Solved on 01.24, with the existing stand-ins and
no changes to them: booting, the title screen, SSLv3 sign-on returning 200, the content
manifest, and the 1,034 update packages. Not solved: the BAP connection. The client
reaches `bootflow:bap_signin` and sits in `network_bap_nat_punch_queue` /
`waiting_for_bap_refresh`, with 30+ STUN retries and no dial.

The next session should therefore stop sweeping replies and start from the other end of
the chain:

1. hook the code that **consumes the NAT punch** (the `bap_nat_punch_queue` string is a
   concrete anchor) and find what it waits for;
2. in parallel, check whether `bap_server.py`'s framing - documented as taken from 01.00 -
   is still what 01.24 sends, since the client may be refusing to even open the socket;
3. the DLC entitlement gate at `0x36e92c` (noted in `final-build.md` as the 01.24 twin of
   the 01.00 check) is a plausible pre-dial gate worth ruling in or out early.
**RUN LOGIN1 - the full title -> setup -> character-select sequence, automated, and where
it stops.** `Invoke-Final124Login.ps1` is the 01.24 counterpart of `Invoke-TitanLogin.ps1`.
The important difference is the progress signal: TitanLogin watches the **BAP** log for
`sent sign-in rsp` and `select-character rsp`, which on 01.24 never appear because the
client never reaches BAP - it would time out on a client that has in fact signed on. So
this driver watches the SignOn listener instead (`replied 200 SignOn success`, `replied
200 content manifest`) and uses the `BETA BUILD` watermark screenshot test for menu
readiness, and it reports every stage whether or not the run completes.

Result, with the whole sequence driven unattended:

```
stage 1  title screen        : sign-on OK after 1 Cross press
stage 2  setup screens       : Cross, Cross, stick-down, Cross - manifest fetched
stage 3  menu readiness      : no BETA BUILD watermark within 120 s
stage 4  character select    : cursor moved, Cross x3 - not confirmed (no BAP line)
BAP log lines                 : 1 (its listening banner, as always)
```

The screenshots are the useful part, because they show the client is not stuck at a menu
it is failing to accept input on:

- `14:42:29` - the **Destiny loading spinner** (the glyph bottom right), screen otherwise
  black;
- `14:45:41` - the same spinner, ~3 minutes later, after six more Cross presses.

So on 01.24 the client gets: title -> Cross -> Bungie logo -> sign-on 200 -> manifest 200
-> **and then sits on a loading spinner indefinitely**, with the setup screens and
character select never appearing. The BETA BUILD watermark that TitanLogin waits for is
never drawn, so there is no menu to drive.

This is consistent with everything else and it localises the gate more sharply than the
STUN work did: the client is past sign-on and past the manifest, and it is waiting on
whatever comes between them and a character-select menu. On 01.00 that step is the BAP
connection (`network_bap_nat_punch_queue` -> `waiting_for_bap_refresh` -> dial), and BAP is
still the only thing with no traffic. So the spinner is the BAP wait, made visible: this
is a real, correct-looking loading screen that never completes, not a hang.

Practical consequence for automation: a 01.24 unattended driver cannot press its way past
this, because the client is not waiting for input. The correct next move is to satisfy the
BAP dial, not to press more keys - pressing is now provably useless (six presses, no
change). The DLC entitlement gate at `0x36e92c` and the punch consumer are the two
candidates to test first.
**Manifest and packages ruled out as the cause of the spinner (2026-09-30).** Worth
recording because it is a large class of suspicion eliminated cheaply, by inspecting what
the client actually got rather than by running more variations.

`manifest-final-smoke.bin` (15,964 bytes) is well formed: a msgpack stream whose entries
carry 20-byte SHA-1 digests and 22-byte `ps3_<name>_<id>_<n>` package names, 311 in total.
It includes every package the bootflow needs, not just the disc ones:

```
ps3_bootstrap_01a3_1                 ps3_investment_globals_012c_4 .. 0131_4
ps3_client_bootstrap_patchabl_0172_2  ps3_investment_assets_0135_4 .. 0142_3
ps3_client_startup_globals_018d_4     ps3_globals_0151_4, 0153_en_4, 0154_3 ...
ps3_client_development_fallba_0180_0 ps3_globals_cinematic_01e3_2
```

and all 1,034 update packages are installed at the right sizes in the RPCS3 copy, with
each of these specifically present:

```
ps3_investment_globals_012f_4        present
ps3_client_bootstrap_patchabl_0172_2 present
ps3_globals_0151_4                   present
ps3_investment_assets_0135_4         present
ps3_bootstrap_01a3_1                 present
```

So the content path is genuinely complete: the manifest is well formed, it names the
bootstrap/investment/globals packages, and every one of them is on disk. That removes
"the manifest is malformed", "the client is not offered the update packages" and "a
required package is missing" as explanations, and it is consistent with the client's
progress: it fetched the manifest with a 200 and moved on rather than erroring.

What is left between the manifest and a menu is the BAP dial, unchanged from the last
few runs. The DLC entitlement gate at `0x36e92c` calls `0x209340`, then `0x35cd7c` and
`0x1cb82c` before testing the result, and the client is silent about it in the RPCS3 log
- the `bootflow:check_media: NPDRM check failed.` / `bootflow:illegal_dlc_media` strings
are not appearing, which is weak evidence the gate is passing rather than failing.

The productive next step is therefore a hook on the BAP dial decision rather than more
manifest or input work: find what has to be true for the client to call
`start_connection` on `127.1.1.127:12850`, given the address is already delivered in the
SignOn reply.
**The BAP dial has a concrete precondition (01.24, `0x363138`, read from the disassembly).**
The string `Primary outbound BAP connection failed during start_connection` is referenced
from exactly one place, and that function shows the two values the client requires before
it will even attempt the outbound connection:

```
0x363148  ori   r31, r3, 0          ; the BAP connection object
0x36314c  lwz   r3, 0x30(r31)       ; state field
0x363150  cmpwi r3, 0x15            ; 21
0x363154  beq   0x363164            ; == 21 -> carry on, otherwise fail out
0x363164  lwz   r3, 0x34(r31)       ; a second state field
0x363168  cmpwi r3, 3               ; 3
0x36316c  beq   0x3632d4            ; == 3 -> carry on
```

So the dial requires `0x30 == 21` and `0x34 == 3` on the BAP connection object. Neither
is something a stand-in can set - the client sets them from its own state machine, which
is fed by the NAT punch (`network_bap_nat_punch_queue`, `waiting_for_bap_refresh`). That
is the same structural finding as the STUN gate, one level up and now with the exact
numbers attached, and it is a concrete hook target: watch the BAP connection object and
see which of the two fields is wrong.

It also explains the punch sweep's flat result without needing the reply shape to matter:
whatever the reply does, these two fields never reach 21/3 in our environment.
**RUN W21 - the BAP state machine walks to 19 and stops. This is the whole blocker, measured.**

Hooking the writer, both of its arms, the state setter it calls, and the gate:

```
0x366348  the writer (chooses 21 vs 22)  : 0 hits
0x366350  the 'send 21' arm               : 0 hits
0x36635c  the 'send 22' arm               : 0 hits
0x356494  the state setter                : 9 hits
0x363138  the dial gate                   : 200 hits
```

The writer never runs, but its callee does - from nine *other* call sites, and the state
argument at each one is the client's own BAP connection state:

```
lr=0x3664b0  r3 = 0x02
lr=0x3642e8  r3 = 0x03     <- the value the gate wants at 0x34!
lr=0x365d74  r3 = 0x04
lr=0x367708  r3 = 0x06
lr=0x3672d0  r3 = 0x07
lr=0x365b10  r3 = 0x08
lr=0x36274c  r3 = 0x0d
lr=0x364f0c  r3 = 0x12
lr=0x3670b8  r3 = 0x13     <- where it stops
```

So the client advances its BAP connection through **2 -> 3 -> 4 -> 6 -> 7 -> 8 -> 13 ->
18 -> 19** and then stops, with the gate (`0x363138`, 200 entries) checking a field that
never becomes 21. The `0x34 == 3` half of the gate *is* satisfied during that walk - state 3
is passed - but the run ends at 19, and the only code that ever writes 21 (`0x366350`) is
never reached.

**This is the most precise statement of the blocker so far and it is a client-side
sequence, not a stand-in problem.** The nine transitions are the BAP connection's own
lifecycle, and the walk stalls two states short of the one that permits the dial. Combined
with `waiting_for_bap_refresh` and `network_bap_nat_punch_queue` in the same binary, the
reading is that the sequence is waiting on the NAT punch completing, and the punch is what
the unaccepted STUN reply prevents - so the STUN work and the BAP gate are the same
problem, which is where this session ended up after starting on a different theory.

The next session has a single concrete target, and it is the best-defined one in the
whole project: **find what makes the client move from state 19 to state 21 at
`0x366350`, or find the transition 19 -> 21 that this hook list has not caught.** The
`lr` of each setter call above is a vtable slot, so the natural tool is a broader watch on
`0x356494` with more hits allowed (it hit its 9-hit cap behaviour early) plus the
callers of the nine sites, and the state-19 handler specifically. Nothing in the
stand-ins can move this forward; it is a client state machine, and the state numbers are
now known.
**The 19 -> 21 transition, read out (this closes the chain).** `0x367090` is 14
instructions and is the *only* place that produces the state 19 the walk stops on:

```
0x36709c  lwz   r4, 0x30(r3)      ; read the field the gate also reads
0x3670a0  cmpwi r4, 5              ; is it 5?
0x3670a4  beq   0x3670b0           ; yes -> park in 19
0x3670a8  bl    0x366b88           ; no  -> the long path (322 instructions)
0x3670b0  li    r3, 0x13           ; state 19
0x3670b4  bl    0x356494
0x3670c4  blr
```

and the long path, `0x366b88`, starts by re-reading the same field:

```
0x366ba0  lwz   r4, 0x30(r22)
0x366bb0  cmpwi r4, 0
0x366bb8  bne   0x366d64           ; non-zero -> jump into the middle
0x366bbc  bl    0x1269f8           ; zero: a first attempt of something
0x366bc0  cmpwi r3, 0
0x366bc4  bne   0x367088
```

So the picture is complete and it is a **single field, `0x30`, with two values that matter**:
`0x30 == 5` parks the client in state 19 (which is where every run ends), and
`0x30 == 0` starts the work that ends in the states already observed - 2, 3, 4, 6, 7, 8,
0xd, 0x12 - before returning to `0x367090` and parking again. The value 21, which the
dial gate wants, is written only by `0x366350`, and reaching it requires the long path to
complete instead of looping.

In our runs the field is 0, the long path runs, and the walk ends at 19 - so the long
path is not completing. That is the narrow question now, and it is a *client-side
sequence* like everything above it. What is worth trying next, in order:

1. hook `0x366b88` and the state setter with a high cap and read the **order** of the
   nine transitions across a longer run, to see whether it loops 2..19 or advances once
   and stalls. That distinguishes "retrying" from "waiting";
2. hook `0x1269f8` (the `0x30 == 0` first attempt) and `0x126648` (called first thing in
   the long path) - if the latter returns a failure the rest of the 322 instructions
   cannot succeed;
3. only after that, revisit whether `0x30` can be influenced at all. Everything so far
   says the stand-ins cannot, because `0x30` is set by client code and never by the
   network - but nothing has yet shown which client code, and `0x126648` is the first
   instruction of the long path, so it is the cheapest place to look.

**This is a good place to leave the project for a session.** The gate is located, its two
required values are known, the state sequence is enumerated, and the writer of the
required value is a single 14-instruction function. What is not known is why the 322-
instruction path does not complete, and that is one hook on `0x126648` away.
**RUN LOOP - it is looping, not waiting, and the sequence runs exactly once.** The
question from the previous section, answered:

```
0x367090  the parking spot (14 insns) : 200 hits   (capped)
0x366b88  the long path  (322 insns)   : 200 hits   (capped)
0x126648  first insn of the long path : 100 hits   (capped)
0x1269f8  the '0x30 == 0' attempt    : 100 hits   (capped)
0x366348  the writer of 21/22         :   0 hits
0x356494  the state setter            :   9 hits
0x363138  the dial gate               : 400 hits   (capped)
```

and the nine transitions, in order, once:

```
0x02 (from 0x3664b0)   0x03 (from 0x3642e8)   0x04 (from 0x365d74)
0x06 (from 0x367708)   0x07 (from 0x3672d0)   0x08 (from 0x365b10)
0x0d (from 0x36274c)   0x12 (from 0x364f0c)   0x13 (from 0x3670b8)
```

So the client runs the BAP startup sequence **once**, arrives at 19, and then spins:
the parking spot, the long path, its first two calls and the dial gate are all entered
hundreds of times while the state setter is not called again. It is not waiting for a
timeout and it is not retrying the sequence - it is re-entering a loop whose only exit,
`0x366348` writing 21, is never taken.

That reframes the remaining question usefully. The state machine completed its startup
and parked; the gate is a **consequence** of the parking, not a second failure. So
`0x30 == 5` at `0x36709c` is the thing to understand: the client parks in 19 when that
field reads 5, and in every run it reads 5. Whatever sets `0x30` to 5 is the last
untouched link, and it is not a stand-in concern - it is client code reacting to
something. `0x1d(r31)`, the byte that selects 21 versus 22 in the writer, is the natural
companion: 21 is "the sequence finished properly", 22 is "it did not", and 0x1d would say
which the client believes.

**Summary of the 01.24 gate, end to end.** Title -> Cross -> SSLv3 sign-on 200 ->
manifest 200 -> package registration (1,034 packages installed and named) -> BAP startup
sequence 2,3,4,6,7,8,13,18,19 -> park. The dial needs state 21, which is written only when
the sequence completes successfully, and the sequence's own condition (`0x30 == 5`) is
never satisfied. Nothing in the stand-ins is implicated by any of this: the network
exchange is complete and correct at every step that has been measured, and the gap is
entirely inside the client's own BAP connection lifecycle.
**RUN W5 - which writer sets 0x30 = 5, and the condition that chooses it.** Four
writers of the value 5 exist; two of them fire in our runs (both at their hit cap):

```
0x362b98  50 hits      0x365fdc   0 hits
0x366ea4  50 hits      0x366fc4   0 hits
```

`0x366ea4` is the one that matters, and the selector immediately before it is three
branches that all target it:

```
0x366df8  lwz   r4, 0x94(r3)     ; a field on the same object
0x366dfc  cmpwi r4, 1
0x366e00  beq   0x366e2c         ; == 1  -> carry on with the sequence
0x366e04  cmpwi r4, 0
0x366e08  bne   0x366ea4         ; != 0 (and not 1) -> park
0x366e10  ...                   ; == 0 falls through to its own test
0x366e18  beq   0x366ea4
0x366e28  bne   0x366ea4
...
0x366ea4  li    r3, 5
0x366ea8  stw   r3, 0x30(r22)    ; the value that makes 0x367090 park in state 19
0x366eac  b     0x367088
```

**So the whole 01.24 BAP gate reduces to one word.** The client completes its BAP
startup sequence and then checks `0x94(r3)`:

- `0x94 == 1` -> it carries on, and eventually the writer at `0x366348` emits 21, which
  the dial gate at `0x363138` accepts, and BAP is connected;
- anything else (0, or another value) -> `0x30` is set to 5, `0x367090` parks the client
  in state 19, and the gate is never satisfied. Which is what every run does.

`0x94` is a *result* field, not a state: it is the outcome of the NAT punch that the
STUN exchange feeds. The earlier work and this are the same problem seen from both ends -
the punch result is never 1 because the STUN reply is never accepted - and the punch
sweep's flat result is now explained rather than mysterious.

**The single highest-value next experiment** is to hook `0x366df8` and read `0x94(r3)` at
that point, to confirm it is neither 1 nor a value we could influence from the network.
If it is 0 (rather than an error code), that says the punch is simply incomplete; if it is
an error, the error value names the failure. Either way it is one hook, and it is the last
one between "STUN reply" and "BAP connected".
**RUN W94 - the deciding word measured: 0x94 == 0, and the object names a host we do
not sinkhole.** Hooking `0x366df8` (200 hits) and dumping 0x100 bytes of the object:

```
0x30 = 0        (the gate wants 21)
0x34 = 0        (the gate wants 3)
0x94 = 0        (the test wants 1)
strings in the object: 'dm-partnernet.upload.deadorbit.net' and '/ticket_drop'
```

Both string constants are real and present in **both** builds (`/ticket_drop` is at
`0x011bdc01` in 01.00 and `0x012d65c0` in 01.24, `dm-partnernet.upload.deadorbit.net`
at `0x012d6598` in 01.24), so this is long-standing Demonware plumbing, not something
01.24 introduced. It is an upload endpoint: the client holds a host and a path it
intends to POST to, and `0x94` is the success flag for that step. It reads 0, so the step
never succeeded, and that is the single word standing between the run and BAP.

**A real gap found on the way, though it is not the current blocker.**
`dm-partnernet.upload.deadorbit.net` is in **none** of the host tables
(`hosts.py` knows 48 names; 17 contain `deadorbit`, and this one is not among them). If
the client ever does resolve it, the sinkhole would not answer and the lookup would go to
the real internet - which the project's data policy forbids. Worth adding to
`ALL_OVERRIDES` as a precaution even though it is not why this run fails.

It is **not** why this run fails, and it is important not to overclaim here: the DNS log
for the run lists every lookup the client actually made - the 20 STUN names,
`www6.signon.deadorbit.net` and `content.local.test` - and `dm-partnernet` is **not** among
them. So the host is held in the object as configuration, not contacted. The client never
gets far enough to use it. Adding the override is correct hygiene; it is not a fix.

**State of the 01.24 gate, now fully mapped.** One word decides it:

```
STUN exchange -> NAT punch -> upload/ticket_drop step -> 0x94 == 1?
                                                            |
                                        yes: 0x366348 writes 21 -> 0x363138 dials BAP
                                        no:  0x366ea4 writes 5  -> 0x367090 parks in state 19
```

`0x94` is 0, so the client parks. Everything upstream is verified working: the title
screen, SSLv3 sign-on 200, the manifest, and the 1,034 packages. The next task is to find
what performs the `/ticket_drop` step and sets `0x94` - and because it is an upload, the
stand-in may well need to *serve* that endpoint, which would finally be a thing the
network side can do rather than a thing it can only observe. That is the first lead in
this project that suggests the fix is on our side of the wire.
**RUN W94B - none of the ten `0x94 = 1` writers ever execute.** The deciding test
itself fires 200 times as always, so the client keeps reaching the check and keeps
getting the same answer - but not one of the sites in the Demonware transport that
would set the flag is ever entered:

```
0x121168  0x1217d0  0x12197c  0x123f7c  0x123fbc
0x12a14c  0x12c5d8  0x12c610  0x12c64c  0x12d1b0      all 0 hits
0x366df8  200 hits                                (the deciding test)
```

That is a stronger result than "0x94 reads 0". It says the code that sets the flag is
never reached at all, so `0x94` is not being *written zero* by something - the whole
region of the transport that ends in `0x94 = 1` is skipped. The step that would upload
to `/ticket_drop` and record success never begins.

This also explains the flat punch-sweep result from the other direction. The sweeps
varied the STUN reply and nothing changed, and now we can see why: the sweep was
changing the input to a stage that this run is not entering. The upload is downstream of
the punch in the code, and the run is dying upstream of it.

**Where the evidence lives.** The hook lines from all three runs are kept, trimmed to the
watched addresses, in `notes/evidence/`:

```
RUN_LOOP-hooks.txt               the looping-vs-waiting run
RUN_W94-hooks.txt                the 0x94 measurement, with the 0x100 byte object dump
RUN_W94B-hooks.txt               the ten 0x94 writers, all silent
```

**Next, precisely.** The question is no longer "what sets 0x94" - it is "what calls the
code that sets it, and why is that never called". `0x127100`, the 2,556-instruction
function that holds both the upload host and `/ticket_drop`, has exactly two callers
(`0x28a744` and `0x28a850`). Those are the two addresses to hook next: if neither fires,
the transport is never asked to upload, and the reason will be upstream of them. If one
fires and the ten writers still do not, the failure is inside `0x127100` and the upload
reply handling is where to look. Either way it is two hooks and the question is closed
either way - which is why this is worth doing first next session.
**RUN CALL - correcting the previous section: the upload transport IS invoked.** This
changes the conclusion above and is worth being precise about.

```
0x127100  the 2,556-instruction upload function : 100 hits
0x28a744  its caller 1                          :   1 hit   (from lr=0x28a748)
0x28a850  its caller 2                          :   0 hits
all ten 0x94 = 1 writers                        :   0 hits
0x366df8  the deciding test                     : 200 hits
```

So the transport *is* entered, 100 times, and mostly from `0x28a854` (the return address
just past caller 2, so the `0x28a850` hook simply missed the window). It is not skipped.
I was wrong to say the upload never begins, and the W94B conclusion was overclaimed:
the ten writers are not silent because their region is skipped, but because they belong
to **different functions entirely**:

```
0x121128 .. 0x121a0c  569 insns   <- 0x121168, 0x1217d0, 0x12197c
0x12bd64 .. 0x12c6c4  600 insns   <- 0x12c5d8, 0x12c610, 0x12c64c
0x12d160 .. 0x12d1c4   25 insns   <- 0x12d1b0
```

None of them overlap `0x127100`. The function that gets the upload *requested* is not
the function that records the upload *succeeding*, so "the transport runs" and "the flag
is set" are separate facts, and only the first is true here. That is a genuine finding,
just not the one W94B claimed.

**The corrected picture.** The client asks for the upload repeatedly, the transport
function runs repeatedly, and the success writers never run. So the request goes out and
no success ever comes back. That moves the question to the **reply path** rather than the
request path, and it is the first time in this project that a stand-in could plausibly be
the cause rather than a bystander: an upload needs an HTTP response, and if nothing is
listening for it, the success writers are unreachable by construction.

**Next.** Two things, in order, and the second is the one that matters:

1. hook the three writer functions (`0x121128`, `0x12bd64`, `0x12d160`) and see whether
   they are entered at all with the flag not set, which distinguishes "reply handler never
   called" from "reply handler called and rejected the reply";
2. find what the upload expects back. `19bdUploadInterceptor` (`0x014020e8`) and
   `'Upload complete (%d total bytes, %.2f)'` (`0x01402508`) are the names to work
   backwards from, and the second one is a log line with a byte count - it tells us the
   upload has a body and a length, which is what a responder has to agree with.

Note the standing assumption this rests on: that the client is willing to talk to
`127.1.1.127` for anything. That has held for the STUN and sign-on traffic, and the
upload host resolves to the same loopback range, so it is reasonable - but it is an
assumption, and step 1 is cheap enough to confirm before building a responder on it.
**RUN REPLY - the reply handlers are never called, and one of my three "writers" was
an initializer, not a reply handler.** Hooking the two real writer functions, their
callers, and the vtable-dispatched one:

```
0x11cca0  caller of 0x121128       : 0 hits
0x121128  writer fn 1 (569 insns)  : 0 hits
0x12ca0c  caller of 0x12bd64       : 0 hits
0x12bd64  writer fn 2 (600 insns)  : 0 hits
0x12d160  writer fn 3 (25 insns)   : 1 hit
0x127100  the upload function      : 200 hits
0x366df8  the deciding test        : 200 hits
```

`0x12d160` is a false lead and should be struck from the notes. It is 25 instructions
that zero a set of fields:

```
0x12d174  li    r31, 0
0x12d1b0  stw   r31, 0x94(r3)     <- stores ZERO, not 1
0x12d1b4  stw   r30, 0xb0(r3)     <- r30 = -1
```

It is an object constructor, and it writes to **static** objects at `0x1a2xxxx`, not to
the BAP connection object. My scan had flagged `0x12d1b0` as an "0x94 = 1 writer" only
because the nearest preceding `li ... 1` was far above it and belonged to other code. One
hit at process start is exactly what a constructor does. **It is not a reply handler and
was never a candidate.**

**Also worth correcting: `0x94` is not a BAP-specific field.** Scanning the whole EBOOT
for stores to `0x94(r*)` gives **1,434 sites** across the image. It is a common struct
offset, so "find the code that writes 0x94" is not a tractable question on its own - the
hooks I chose were guesses at which struct was meant, and two of the three were wrong.
Any future work on this should identify the *struct* first (from the `0x366df8` object
dump, which shows the host string at `+0x04` and `/ticket_drop` at `+0x61`) and search
within it, not pattern-match the offset.

**What is actually established.** The upload function `0x127100` runs 200 times. Nothing
that could interpret a reply ever runs. And the client never even resolves
`dm-partnernet.upload.deadorbit.net` - the DNS log for this run has no lookup for it,
again. So the upload is *attempted* at the transport layer but never reaches a name
lookup, which means it cannot have produced a request on the wire, and so no responder
could help. That is the honest summary: this is not a missing-listener problem. The
failure is before the name is ever resolved.

**On the endpoints, since the policy question was raised.** Checked whether the real
services still answer:

```
dm-partnernet.upload.deadorbit.net        -> 205.209.23.65   (resolves)
www1.signon.deadorbit.net                 -> 172.97.59.1     (resolves)
panther-ps3-auth3.prod.demonware.net      -> NXDOMAIN
destiny-stun.us.demonware.net             -> 127.0.0.1       (our own override)
```

Two of them still resolve in DNS. Whether anything *listens* on them is a different
question and I did not probe it - sending traffic to Bungie's live infrastructure would
be of no use to a private server stack even if it answered, because the private stack has
to be self-hosted by definition, and an answered sign-on from production would not
produce a session our `bap_server.py` could speak to. So the relaxation does not change
the plan: the work stays on emulating the endpoints locally. Recording this so the option
is not re-litigated.

**Next.** The upload dies before DNS resolution, so the next thing to watch is inside
`0x127100` itself, not around it - specifically what makes it give up rather than
resolve. A cheap way to find that is to watch the point where it decides to proceed,
which is likely near where it loads the host string (`0x127d30`). Hooking `0x127d30` with
a dump would show whether the object it is building is even populated by then.
**RUN HOST - a tool bug was masking the real code, and the picture inverts.** Hooking
where the upload object is built:

```
0x127d30  where it loads the host string pointer : 0 hits
0x127e40  where it loads '/ticket_drop'         : 0 hits
0x127100  the function entry                    : 200 hits
```

`0x127100` runs 200 times and never reaches the code that builds the upload request. Two
things were wrong in how I had been reading this, and both had to be fixed before the
result meant anything.

**First, a real bug in `d1_eboot.py`.** `Eboot.read()` returns short at a page boundary,
so every scan of `0x127100` was silently truncated at 3,120 bytes - exactly at `0x127d30`.
The apparent "no early return in `0x127100`" and the truncation were the same fault: the
host-string load at `0x127d30` sat precisely at the end of the readable window, so it
looked like the function ended there. Re-reading a page-aligned window shows real
instructions continuing past it:

```
0x00127d2c  stw    r25, 0xc50(r6)
0x00127d30  addic  r10, r3, 0x6598   <- the host string pointer
0x00127d34  stw    r25, 0xc54(r6)
```

**Second, and the actual finding: nothing inside `0x127100` branches or calls into
`0x127d00-0x127d30`.** I scanned every branch in the whole function for a target in that
range and found none, yet `d1_eboot.py func 0x127d30` reports `0x127100 .. 0x1298f0` with
2,556 instructions - it believes they are one function. The linear-disassembly heuristic
is merging two functions that the control flow does not connect. **So `0x127100` is not
"bailing out early"; the upload-object code is simply not part of it, and `0x127100`
being hit 200 times says nothing about the upload.**

This retires the "the upload function runs 200 times" finding from the previous section
as well - it was measuring the wrong function. The honest current state is narrower:
`0x127d30` and `0x127e40`, which actually reference the upload host and path, are never
reached, and we do not yet know who calls them.

**Concrete next step, now that this is isolated.** Find the callers of `0x127d00`
properly, by scanning the whole image for branches to it rather than trusting the merged
function boundary. That gives the real entry point of the upload builder, and its caller
is what to hook next. The `d1_eboot.py` truncation is worth fixing at the same time -
it will silently mislead any other scan that crosses a page.
**Correction to the previous section, twice over. `d1_eboot.py` is not buggy and I was
wrong twice.** Having claimed a page-boundary truncation bug, I checked it directly:

```
read(0x127100, 0xc30) -> 3120 bytes     # 0xc30 IS 3120 - it returned exactly what I asked
read(0x127100, 0xc00) -> 3072
read(0x127100, 0xd00) -> 3328
```

`read()` is correct. My scan size ended *precisely* at `0x127d30`, so the address I was
looking for was the first word past the end of my own buffer. The tool was never at
fault; I was looking one instruction too far. **No fix to `d1_eboot.py` is needed and
none was made.**

**Second correction: `0x127100` is not an upload function, and the 200 hits mean
nothing.** What it actually does is visible in its first three calls:

```
0x00127108  bl 0xd98a98
0x0012710c  bl 0xd98ae4
0x00127130  bl 0x9a746c      <- memcpy/memset family
```

and then it is a long uninterrupted run of immediate loads with **no conditional branch
whatsoever** in the first 3,120 bytes, materialising a configuration table:

```
0x0012713c  li   r5, 0x7530     = 30000
0x00127158  li   r8, 0x7d0      = 2000
0x00127168  li   r10, 0x1388    = 5000
0x00127170  li   r11, 0x3a98    = 15000
0x00127180  li   r30, 0x2710    = 10000
0x00127188  li   r29, 6
0x00127190  li   r28, 5
0x00127160  li   r9, 0xa        = 10
```

Those are decimal-looking port and interval values, and the pattern
(`std r6, ...` zeroing a block, then filling constants) is a struct being initialised.
So `0x127100` is a **configuration/timing initialiser for the Demonware transport**, and
its 200 hits just mean that initialiser is called on every connection attempt. It holds
the upload host and `/ticket_drop` strings further along because the config record
includes them, not because it performs the upload.

**This retires the whole "the upload transport runs 200 times" line of enquiry.** It was
measuring a config initialiser. What remains true and useful:

- `0x127d30` / `0x127e40`, where the host and path are actually placed into a request
  object, are never reached - but they may be reached from a *different* entry point, and
  I have not yet established one;
- the client never resolves the upload host, so no responder could help;
- ports 2000/5000/10000/15000/30000 are configured. **None of them is 12850**, the BAP
  session port, which is worth noting: the ports in play are not the one the gate wants.

**Honest status.** The BAP gate is fully mapped - `0x94 == 1` permits the dial, anything
else parks the client in state 19 - but I have still not found the code that *sets*
`0x94`, and three consecutive attempts to find it by pattern-matching the offset have
now failed for concrete and different reasons each time (1,434 candidate writers;
`0x12d160` is a constructor; `0x127100` is a config initialiser). The offset itself is
the problem: it is not unique. **The right next move is to stop searching for `0x94` and
instead find who calls the deciding function `0x366df8`'s block from above**, i.e. walk
backwards from the gate to the state that must be true before it, rather than hunting
for a writer of a common offset.

Worth saying plainly: several hours of hooks have now produced a good map of the gate and
a set of corrections, but not yet a fix. The map is the durable result; it should be
trusted over the intermediate claims, which two of them turned out to be wrong about.
**RUN CLEAN - the hook file had 9 duplicated addresses, and it did not matter.** While
checking why `0x367090` shows 200 hits when no code anywhere branches to it, I found
the hook file itself had accumulated duplicates - 60 hook lines for 51 distinct
addresses, nine of them hooked twice or three times:

```
0x127100 x3    0x363138 x2   0x366348 x2   0x366df8 x2
0x121128 x2    0x28a744 x2   0x28a850 x2   0x366ea4 x2   0x367090 x2
```

Duplicated hook lines are a real hazard - they split hit budgets and can double-count -
so I deduplicated (60 lines -> 50, taking the larger hit limit) and re-ran the whole
sequence from scratch. **Every number came back identical:**

```
0x366df8  the deciding test        : 200    0x127100  config init : 200
0x127d30  host string load         :   0    0x127e40  path load  :   0
0x366ea4  writes 0x30 = 5          :  50    0x367090  parking    : 200
0x363138  the dial gate            : 400    0x366348  writer     :   0
0x356494  state setter             :   9    0xb36fd8  STUN reply : 200
```

So the earlier measurements stand. That matters, because the last three sessions have each
had to retract something, and this is the check that says the retractions were wrong
*conclusions*, not wrong *counts*.

**Still unexplained, and it is a real oddity worth flagging.** `0x367090` is hit 200
times, yet a whole-image scan finds **no branch, no data reference, and no code that
computes its address** - `bl`, `b`, all conditional forms, `lis`+`addi` pairs, and a
raw 4-byte search for the pointer all come back empty. The only branch target in the
whole `0x366000-0x368000` window that lands near it is `0x3670b0`, which is *inside* the
function (`0x367090..0x3670c4`), and the preceding function jumps over it entirely with
`0x367088  b 0xd98a4c`. So the function is real, the hook fires, and nothing in a linear
scan explains how it is entered. Most likely it is reached through a computed dispatch -
a table built at runtime, or an offset from a base I have not identified - which is
exactly the shape of thing a linear scan cannot see. I am not going to guess further
without evidence; it is recorded as open.

**Where this leaves the project.** The BAP gate is fully mapped and the map is stable:
`0x94 == 1` permits the dial, `0x30 = 5` parks the client in state 19, and the client
takes the second branch every time. The whole upstream chain works - title, Cross,
SSLv3 sign-on 200, manifest, 1,034 packages. Character selection and orbit remain
unreached because `0x94` is never set, and the writer of `0x94` remains unidentified
after four attempts, the last three of which I have now documented as wrong turns.
That is the honest state, and the durable output is the map plus the four corrections.
**SOLVED: the `0x367090` oddity is a vtable at `0x01855b40`, and it explains the loop.**
Searching the whole image for any word whose low half is `0x7090` and whose value lands
in the BAP region returns **exactly one hit**:

```
0x01855ba0  ->  0x00367090     the parking spot
```

Reading outward from it reveals a table of BAP handlers on an 8-byte stride (a function
pointer alternating with the constant `0x01891798`):

```
0x01855b40  slot  0 -> 0x003665a4      0x01855ba4  slot 10 -> 0x00366ab8
0x01855b48  slot  1 -> 0x003666f0      0x01855bac  slot 11 -> 0x0036295c
0x01855b50  slot  2 -> 0x003668a8      0x01855bb4  slot 12 -> 0x00366b24
0x01855b58  slot  3 -> 0x003669b4      0x01855bbc  slot 13 -> 0x00366b50
0x01855b60  slot  4 -> 0x00366a84      0x01855bc4  slot 14 -> 0x00367090  <==
0x01855b68  slot  5 -> 0x00220db8      0x01855bcc  slot 15 -> 0x003670c8
0x01855b70  slot  6 -> 0x00e4f2d8      0x01855bd4  slot 16 -> 0x00367114
0x01855b78  slot  7 -> 0x00e4f9c4      0x01855bdc  slot 17 -> 0x003671fc
0x01855b80  slot  8 -> 0x00366ab8      0x01855be4  slot 18 -> 0x003672d8
0x01855b88  slot  9 -> 0x0036295c      0x01855bec  slot 19 -> 0x00e4fa98
0x01855b90  slot 10 -> 0x00366b24      ...
0x01855b98  slot 11 -> 0x00366b50
0x01855ba0  slot 12 -> 0x00367090   <== the parking spot is slot 12
```

**This resolves several things at once.**

1. **Why a linear scan could not see it.** The parking spot is not branched to and not
   computed with a `lis`/`addi` pair; it is reached by indexing this table. Every earlier
   scan method was structurally incapable of finding it.
2. **Why the loop repeats.** This is a handler table indexed by connection state. The
   client is dispatching the same slot over and over, which is precisely the observed
   behaviour - `0x367090` at 200 hits, `0x366df8` at 200, the gate at 400, with the state
   setter untouched at 9. **The loop is the table being re-indexed on the same state**, so
   the client is not retrying the sequence, it is stuck re-entering one slot.
3. **It gives the missing index.** Slot 12 holds `0x367090`, so the state being
   dispatched is **12** - and `0x12` (18) is one of the nine states the client reached
   in the very first state-walker measurement. The sequence stopped at 19 (`0x13`), but
   the slot being dispatched is 12, so the state field and the dispatch index are not the
   same quantity, and that mismatch is now a concrete thing to test rather than a guess.

**Next, and it is now a two-hook job:** dump the table pointer and the index register at
a `0x367090` hit, to confirm the index really is 12, and hook the sibling slots - in
particular `0x3665a4`, `0x3666f0` and `0x366ab8`, which are in the same BAP block and may
be the ones that would set `0x94`. This is the first lead that is derived from the
binary's own structure rather than from pattern-matching an offset that turned out to
have 1,434 writers.
**The vtable slots write a small, consistent set of offsets - and `0x94` is not among
them.** Scanning what the sibling BAP slots actually store:

```
slot 0  0x3665a4  0x14 0x15 0x16 0x17 0x18 0x19 0x1a 0x1b 0x1c 0x1d 0x1e 0x1f 0x20 0x21
slot 1  0x3666f0  0x14 0x15 0x16 0x17 0x18 0x19 0x1a 0x1b 0x1c 0x1d 0x1e 0x1f 0x20 0x21
slot 2  0x3668a8  0x30 0x70 0x74 0x80 0x88 0x90 0x190 0x1a0 0x1bc 0x1c0 0x1c4 0x1c8 0x220 0x230
slot 8  0x366ab8  0x30 0x70 0x80 0x90 0xc0 0x1bc 0x1c0 0x1c4 0x1c8 0x310 0x330 0x340
slot10  0x366b24  0x30 0x70 0x80 0x90 0xa0 0xa4 0xa8 0xac 0xb0 0xc0 0x310 0x330 0x340
slot11  0x366b50  0x30 0x80 0x90 0xa0 0xa4 0xa8 0xac 0xb0 0xc0 0x310 0x330 0x340

most common:  0x80 x13   0x30 x9   0x90 x8   0x70 x6   0xa0 x6
              0x190 x4   0x1a0 x4  0x330 x4  0x340 x4  0xa8 x4
```

**None of these slots writes `0x94` at all.** So the flag the deciding test at `0x366df8`
reads is not set by any handler in its own dispatch table. That is a real structural
result, and it means the vtable is not the place to keep looking for the writer.

Two further readings worth keeping:

- **slots 0 and 1 are a different family.** They store a dense run `0x14` through `0x21`,
  which is byte-by-byte field filling rather than the sparse `0x30`/`0x80`/`0x90` pattern
  of the others. Something about the table is grouping two different record layouts
  together - worth remembering before assuming all 20 slots are the same class.
- **`0x30`, `0x80` and `0x90` are the working fields.** `0x30` is the one the gate reads,
  and it is written by nine of the slots. `0x90` is read by the deciding test's
  neighbourhood and written by eight. These are the live fields of this object; `0x94`
  may simply have been the wrong offset to fixate on.

**Next, revised in light of this.** Rather than hunting for a writer of `0x94`, read the
three fields that the dispatch table actually maintains (`0x30`, `0x80`, `0x90`) at the
moment of a `0x367090` hit, together with the index register. That single dump gives the
dispatched slot *and* the state of the object the handler is working on, which is enough
to tell whether the loop is re-entering the same slot with the same inputs forever - and
if so, which input is supposed to change. That is one hook and one run, and it is the
cheapest informative experiment left.
**RUN VT - the loop is three consecutive vtable slots, and slot 10 resets the gate
field. This is the tightest explanation of the stall so far.**

Hooking the table itself rather than individual guesses:

```
slot  0  0x3665a4 : 1        slot  8  0x366ab8 : 1
slot  1  0x3666f0 : 1        slot 10  0x366b24 : 20
slot  2  0x3668a8 : 0        slot 11  0x366b50 : 20
                           slot 12  0x367090 : 30
0x366df8 deciding test : 30   0x366ea4 write 5 : 20   0x363138 gate : 50
```

So slots 0, 1 and 8 fire **once each** - they are the startup, already finished - and
then slots **10, 11, 12 cycle indefinitely**. That is the loop, and it is a three-slot
cycle, not a single stuck slot as I said last section. The object dump at every slot-12
hit is byte-identical:

```
0x30=0  0x34=0  0x80=0  0x90=0  0x94=0        (30 of 30 hits, no variation)
r4=0x367090   r5=0x1c18c50
```

**And slot 10 is the one that resets the gate field:**

```
0x00366b24  stdu  r1, -0x70(r1)
0x00366b30  li    r4, 0
0x00366b34  stw   r4, 0x30(r3)     <- CLEARS 0x30, the field the gate reads
0x00366b38  li    r3, 0x10
0x00366b3c  bl    0x3595a0         <- state 16
```

slot 11 then reads that field and branches on it:

```
0x00366b60  lwz   r3, 0x30(r3)      <- reads back what slot 10 just cleared
0x00366b68  cmpwi r3, 3             <- == 3 ?
0x00366b6c  bne   0x366b74
0x00366b70  bl    0xe6778
```

**This closes the loop mechanically.** Slot 10 sets `0x30 = 0` and moves the state to
16; slot 11 reads `0x30`, sees 0 (not 3), and does nothing; slot 12 reads `0x30`, sees it
is not 5, and runs the long path, which writes 5 via `0x366ea4` and re-enters. The gate
at `0x363138` wants **`0x30 == 21` and `0x34 == 3`** - and the slot-10 handler is
*clearing the very field the gate reads, every single cycle*. Whatever sets `0x30` to 21
has to win a race against slot 10, and slot 10 always runs.

**So the useful question is no longer "what writes 0x30 = 21" - it is "what is supposed
to stop slot 10 from clearing it".** That is a much better-formed question, and it points
at the state machine's own exit condition rather than at a missing writer. `0x366348`
(the 21/22 writer) has still never executed, so the branch that would stop the cycle is
never taken.

**Next, concretely.** Slot 10's guard is whatever leads the dispatcher to choose slot 10
instead of a later slot - the index is the thing to watch, not the slot bodies. Dump the
dispatch index register at the `0x366b24` hit (it is already dumped as `r5 = 0x1c18c50`,
which is a pointer, so the real index is likely elsewhere) and hook the dispatch stub
itself. Finding *where* the index comes from tells us which value would move the client
off the 10 -> 11 -> 12 cycle, and that value is the whole game.
**Attempted to delimit the table; the marker is too common, so treat slot indices as
provisional.** `0x01891798` interleaves with the pointers, which suggested an
8-byte-stride table of (function, marker) pairs. Walking it in both directions gives a
32,849-slot span from `0x01849508` to `0x01889790`, which is not a dispatch table for
anything - it is a general constant region that happens to alternate with pointers
here. So:

- **do not rely on the "+12" slot number** from the previous section. It is an offset
  from an arbitrary choice of start, not a real index;
- what *is* solid, because it came from execution rather than from table-walking: slots
  `0x366b24`, `0x366b50` and `0x367090` fire 20/20/30 times in a repeating cycle, they
  are the only handlers firing repeatedly, and slot 10's body provably clears `0x30` -
  the field the dial gate reads. That part is measured, not inferred, and it stands.

No code anywhere loads the table base directly either (`lis 0x185` + `addi 0x5b40`
finds nothing), so the dispatch is indirect through an object vtable whose instance we
have not located. Finding that instance is the real next step, and the cheapest route to
it is the `r5 = 0x1c18c50` value already visible in the slot-12 dumps: that is a heap
pointer present at the parking spot, so it is very likely the object holding the vtable.
Dumping memory around `0x1c18c50` on the next run would show the table pointer itself and
close the last gap in this line of enquiry.

I want to be explicit that the "slot 12" framing was an over-reach on my part - it came
from a plausible-looking table walk that turned out to be noise, and I am withdrawing it
rather than leaving it to be trusted. The measured cycle underneath it is unaffected.
**FOUND THE DISPATCHER, from the `lr` in the hook line.** Every `0x367090` hit carries
`lr=0x356bcc`, and that address is the indirect call itself:

```
0x00356b90  lwz     r4, 0(r4)        <- load the handler pointer
0x00356b94  mtctr   r4
0x00356b98  bdnzctrl                 <- indirect call  == the return address
0x00356bb4  lwz     r3, 0x80(r3)     <- r3: an array of objects, indexed
0x00356bb8  lwz     r4, 0(r3)
0x00356bbc  lwz     r4, 0x1c(r4)     <- +0x1c: the handler slot on that object
0x00356bc0  lwz     r4, 0(r4)        <- +0x00: the function itself
0x00356bc4  mtctr   r4
0x00356bc8  bdnzctrl
```

So the chain is `r28 + (0x118(r28) << 2)` -> `+0x80` -> `+0x1c` -> `+0x00` -> call. That
matches the observed `r4=0x367090` exactly: `r4` already held the handler address before
the `lwz 0(r4)` load, and the slot lives at **`+0x1c` of a per-entry object**.

**And the loop has an explicit bound:**

```
0x00356bcc  addic  r26, r26, 1
0x00356bd0  cmplwi r26, 0x26      <- 38 iterations
0x00356bd4  bge    0x356bf8
```

`r26` counts 0..0x26 (**38**) and falls out to `0x356bf8` when exhausted. So this is a
**bounded 38-step dispatch loop, not an infinite spin.** That is an important correction to
"the client is stuck looping" - it is running a fixed-length schedule, and if it is
repeating we are seeing that 38-step loop restart rather than a hang.

The loop reads these fields off `r28`, and they are the real state of the connection:

```
0x118(r28)   the dispatch index, scaled by 4 (slwi r3,r3,2)
0x11c(r28)   compared against 0x12c(r28) as a loop bound
0x128(r28)   a second index / generation counter
0x12c(r28)   the upper bound
+0x80        object array base
```

**`0x118(r28)` is the dispatch index.** That is the value to watch - it is what selects
between the handlers, it is what the whole BAP progression depends on, and it has never
been read. One hook on the dispatcher with `r28` dumped gives the index, the bounds, and
the handler slot together, which is the last piece needed to say what the client *thinks*
its BAP state is at each step versus what it actually is.

**Next.** Hook `0x356ba4` (the index load) and `0x356bc4` (the final call) with `r28`
dumped, and compare `0x118`/`0x11c`/`0x128`/`0x12c` across hits. If `0x118` is pinned at
one value while the object stays all-zero, the client never advances its dispatch index
and the question becomes "what advances `0x118`" - which is a single word, in a single
structure, and the most tractable target found so far.
**RUN DISP - the dispatch index reaches 3 and stops. This is the whole stall in one
number.** Dumping `r28` (the connection state structure) at every dispatcher hit:

```
step  0:  idx=1  lower(0x11c)=0  gen(0x128)=1  upper(0x12c)=0
step  1:  idx=2  lower=0           gen=2           upper=0
step 32:  idx=3  lower=0           gen=3           upper=0     ... and stays

max index reached: 3        distinct indices seen: [1, 2, 3]
```

`0x128` tracks `0x118` exactly - both go 1, 2, 3 - so the generation counter and the
dispatch index advance together and then both stop. **`0x12c`, the upper bound, is 0**,
and `0x11c`, the lower bound, is 0. With both bounds zero the loop never gets past the
three states it does reach.

That is the first time the stall has been reduced to a **single monotonic quantity that
fails to advance**. Everything else observed is downstream of it:

- `0x30` stays 0 because slot 10 (`0x366b24`) clears it every cycle;
- `0x94` stays 0 because the deciding test reads it before the state that would set it;
- the client parks in state 19 because the long path sees `0x30 != 5` and the gate sees
  `0x30 != 21`;
- the 10 -> 11 -> 12 handler cycle repeats because the index never leaves the range that
  produces it.

**So the question is now precisely: what advances `0x118(r28)` from 3 to 4?** It is one
word, in one structure, written by code that has not been identified - but unlike `0x94`
(which had 1,434 candidate writers across unrelated structs) this is a *specific* field
of a *specific* object, and the search space is far smaller.

Two notes on scope, so this is not over-read. `r26` at `0x356bd0` had **zero** hits, so the
38-iteration bound from the previous section is real code but is not reached on this
path - the bound is not what stops us. And `0x12c == 0` may mean "unset" rather than
"zero-length range"; I have not distinguished those, and treating it as an upper bound of
0 is the reading that fits but is not proven. Either way the actionable fact is the same:
the index stops at 3.

**Next.** Writers of `0x118(r*)`, restricted to stores near the other known fields of this
structure so the search is not swamped the way `0x94` was - i.e. look for code touching
`0x118`, `0x11c`, `0x128` and `0x12c` together, since they are evidently maintained as a
set. That gives a small candidate list and, unlike the last four attempts, the candidates
will be structurally related to the field rather than merely sharing an offset.
**The advance site, located: `0x356ab0`, and it copies rather than computes.** Restricting
the search to `0x350000-0x370000` and grouping by 4KB block, exactly one block touches
three or more of the four fields - `0x356000`, the dispatcher''s own block. Within it the
store cluster is unambiguous:

```
0x00356aa4  lwz   r5, 0x128(r28)
0x00356aac  lwz   r6, 0x12c(r28)
0x00356ab0  stw   r5, 0x118(r28)     <- 0x118 = 0x128. A COPY, not an increment.
0x00356abc  stw   r6, 0x11c(r28)     <- 0x11c = 0x12c
```

**`0x118` is not computed at all - it is copied from `0x128`.** That is the missing piece
and it re-frames the last result. The dispatch index does not "fail to advance"; it is
faithfully mirroring `0x128` every cycle, and `0x128` itself is the thing that stops.
The measured `idx` and `gen` tracking each other exactly (1/1, 2/2, 3/3) is now explained
by construction rather than being a coincidence.

There is also a second, one-shot copy of the same four fields at `0x35692c` and a third at
`0x3569d8`, so there are three sites that fill this structure; `0x356ab0` is the one on the
dispatch path because it sits immediately before the indirect call.

**Two global flags gate the advance**, and this is new:

```
0x00356ab4  lbz   r7, -0x2990(r3)    <- r3 = 0x1a2, so global 0x1a1d670
0x00356ac0  cmpwi r7, 0
0x00356ac4  bne   0x356ad4           <- skip the initialiser if already set
0x00356ac8  bl    0x106618           <- otherwise do a one-time setup

0x00356b14  lbz   r3, -0x2c60(r3)    <- r3 = 0x1ac, so global 0x1acd3a0
0x00356b18  cmpwi r3, 0
0x00356b1c  bne   0x356b3c
0x00356b24  bl    0x116dbc
0x00356b34  stw   r3, -0x2c58(r4)    <- result cached at 0x1acd3a8
0x00356b38  stb   r6, -0x2c60(r5)    <- and the flag set to 1
```

`0x1acd3a8` is computed **once** by `0x116dbc` and cached, guarded by the flag at
`0x1acd3a0`. That is a lazily-initialised singleton, and it is consulted on every
iteration of the dispatch loop.

**So the next step is concrete and small: find the writers of `0x128(r28)`.** Everything
else has been pushed to its conclusion - the index is a copy, the copier is found, and the
one quantity that must move is `0x128`. Two globals (`0x1a1d670`, `0x1acd3a0`) are also
worth dumping in the same run, since a flag that is set when it should not be, or a
singleton initialised from a failed call, would explain a stall that never advances.

I will hook `0x356ab0`, `0x116dbc` and `0x106618` with `r28` and the two globals dumped.
That is three hooks and answers both remaining questions at once.
**RUN W128 - the writer of `0x128` is `0x356120`, guarded by three conditions.** Of the
seven candidate sites across the whole BAP region, exactly one recurs:

```
0x3561a4  read 0x118(r30)              :  0
0x356120  STORE 0x128(r30)             : 10   <== the only recurring writer
0x35692c  fill 0x118(r3)               :  1
0x356938  STORE 0x128(r3)              :  1
0x3569d8  fill 0x118(r31)              :  0
0x3569e0  STORE 0x128(r31)             :  0
0x356ab0  THE COPY 0x118 <- 0x128      : 11
0x356ab4  reads global 0x1a1d670        : 11
0x356b14  reads global 0x1acd3a0        : 11
0x106618  the one-time setup           : 20
0x116dbc  the lazy singleton           : 20
```

The two fill sites at `0x35692c`/`0x356938` fire exactly once - they are the constructor,
setting up the structure before the loop - and `0x356120` is the only thing that ever
writes `0x128` during the run. So the chain is closed end to end:

```
0x356120  writes 0x128 = r26        (10 times)
0x356ab0  copies 0x128 -> 0x118     (11 times)
0x356ba4  indexes the handler by 0x118
```

**And it is gated by three tests before it stores anything:**

```
0x00356108  cmpw  r5, r28
0x0035610c  beq   0x356488      <- bail if r5 == r28
0x00356110  cmpwi r29, 0
0x00356114  beq   0x356488      <- bail if r29 == 0
0x00356118  cmpwi r30, 0
0x0035611c  beq   0x356488      <- bail if r30 == 0
0x00356120  stw   r26, 0x128(r30)
```

All three bail to the same `0x356488`. So the advance happens only when `r5 != r28`,
`r29 != 0` and `r30 != 0` simultaneously. **Any one of those three failing stops the
index from moving, and all three lead to the same place.** Given the index reaches 3 and
stops, the useful next measurement is which of `r5`, `r29`, `r30` is the one that goes
bad - that is a register-level question answerable in a single hook, with no further
static guessing.

Also worth recording: `0x116dbc` and `0x106618` each hit 20 times against 11 loop
iterations, so they are being re-entered more often than the copy site - the "once" in
my earlier description of the lazy singleton is not literally once in practice. The flag
guard is read 11 times, consistent with the loop, but the initialiser itself runs more
than the guard suggests it should. That may be normal (a different call site sharing the
function) or may be a second bug; I have not distinguished those and am not going to
guess from a hit count.

**Next.** Hook `0x356108`/`0x356110`/`0x356118` with `r5`, `r28`, `r29`, `r30`, `r26`
dumped. That single run says which of the three guards is rejecting the advance, and
`r26` at the same moment gives the value the index *would* have taken - so we learn both
why it stalls and what it is trying to reach. That is the last structural unknown between
here and the gate.
**RUN GUARD - the guards pass, so `0x356120` does execute; the bail is not what stops
us.** Results:

```
0x356108  guard 1 (r5 == r28)   :  0    <- not reached at all
0x356110  guard 2 (r29 == 0)    : 10
0x356118  guard 3 (r30 == 0)    : 10
0x356120  stw r26, 0x128(r30)   : 10    <- THE STORE RUNS
0x356488  the bail target       : 10
0x356ab0  the copy              : 11
0x356bc4  the indirect call     : 30
0x366b24 / 0x366b50 / 0x367090  : 20 / 20 / 20
0x366df8 / 0x366ea4 / 0x363138  : 20 / 20 / 40
```

Guard 1 is never reached, which means control always arrives from a path that skips it,
so the `r5 == r28` test is not what we are up against. Guards 2 and 3 are reached, and
the store at `0x356120` follows both of them 10 times - **so the guards are passing and
`0x128` really is being written.** That contradicts the reading in the previous section
that the advance might be blocked by a failing guard, and I am correcting it: the store
runs.

`0x356488` also firing 10 times needs care. It is reached from the *earlier* branches in
the same block (`0x356104`, `0x35610c`, `0x356114`, `0x35611c` all target it), so a
bail and a legitimate arrival can both land there, and the equal counts do not prove the
guards failed. I am not going to claim which without evidence.

**A tooling limitation worth recording, because it changes what is measurable.** The
register dumps at all four addresses are byte-identical (`r5=0x30650150` everywhere,
`r28`/`r29`/`r30`/`r26` absent). The instrumented RPCS3 build is not honouring the
per-hook register list in this file - it emits the same fixed set regardless. Earlier
dumps did vary (`r4=0x367090 r5=0x1c18c50` at the parking spot, and `m28=` memory at the
dispatcher), so the mechanism works for *some* registers and not others. **Any conclusion
in this project that depends on a specific register being dumped is currently
unverifiable**, and the ones I have drawn so far rest on `r28`/`r3`/`r6` which did work.

**Revised understanding.** `0x128` is written 10 times and copied to `0x118` 11 times,
yet `0x118` only ever reads 1, 2, 3. So the store is running with the *same* value each
time - `r26` is not advancing. The thing to find is what computes `r26`, which is a
cleaner target than anything attempted so far: one register, set upstream of `0x356120`,
inside the same `0x356000` block whose other entries are already mapped.

**Next.** Fix the dump specification first (try `m30:`-style memory dumps on `r30` rather
than register dumps, since memory dumps have been reliable), then read `r30` at
`0x356120` and follow where its value at `+0x128` came from. Given the tooling issue, I
would rather change the measurement than draw another conclusion I cannot back.
## CORRECTION - the dispatch index does NOT stop at 3. It reaches 19.

Everything above about "the index advances 1 -> 2 -> 3, then stops" is **wrong**, and the
error was mine: I was reading the `m28=` memory dump but had the field offsets wrong.
Re-reading the archived dumps at the correct offsets gives the whole sequence.

`m28=` is 320 bytes of hex. Reading it properly:

```
dump   0x118   0x11c   0x128   0x12c     <- the four state words, big-endian
  0   0xffffffff 0xffffffff    1       0
  1        1         0         2       0
  2        2         0         3       0
  3        3         0         4       0
  4        4         0         5       0
  5        5         0         6       0
  6        6         0         7       0
  7        7         0         8       0
  8        8         0        13       0
  9       13         0        18       0
 10       18         0        19       0
```

**Both fields walk the full sequence `1 2 3 4 5 6 7 8 13 18 19`**, offset by one because
`0x118` is the previous value of `0x128` (the copy at `0x356ab0` happens one step behind
the store). Corroborated independently by `r5` at `0x356ab0`, which *is* the value being
written to `0x118` (`stw r5, 0x118(r28)`), and it reads the same eleven values in both
RUN_W128 and RUN_GUARD:

```
r5 at 0x356ab0: [1, 2, 3, 4, 5, 6, 7, 8, 13, 18, 19]
```

So the state machine is **not** stalling at 3. It runs to completion and terminates at
**19**, which is exactly the state the client parks in (`+0x94 == 0` -> state 19 via
`0x366ea4`). The earlier "stops at 3" came from the truncated tail of the dump being
misread, and the "1,2,3 then stops" pattern was an artefact of only seeing the first few
entries. **The index reaches its terminal state; it never was the blocker.**

### What this rules out

The whole `0x128`-writer investigation was chasing a non-problem. `0x356120` writing
`0x128` ten times was never "the same value each time" - it was writing the whole
sequence, and `r5` proves the receiving end advanced in lockstep. The guards pass, the
store runs, the copy runs, and the index reaches 19. That chain is healthy.

### The gate, read properly

```
0x00363148  ori   r31, r3, 0
0x0036314c  lwz   r3, 0x30(r31)
0x00363150  cmpwi r3, 0x15        <- 21 decimal. Requires +0x30 == 21.
0x00363154  beq   0x363164        <- taken path continues
0x00363158  ori   r3, r31, 0
0x0036315c  bl    0x3629ec        <- failure path
0x00363164  lwz   r3, 0x34(r31)
0x00363168  cmpwi r3, 3           <- requires +0x34 == 3
0x0036316c  beq   0x3632d4        <- the success continuation
```

`0x15` is 21 - matching the earlier note. **The gate is a two-stage check and both
stages must pass.** `+0x30` is set to 5 by the failure path at `0x366ea4`, never 21.

Note `0x363138` itself is just the function prologue (`mflr r0`) - it is being hit 40-50
times per run as a call-rate counter, not as a gate outcome. The real decision points are
`0x363150` and `0x363168`. I had been hooking the entry and reading `r4=0x363138`, which
is only the return address, and treating the call count as meaningful. It is not.

### Corrected picture

The state machine is fine and reaches 19. The blocker is the `+0x30 == 21` check at
`0x363150`, whose value is set by whatever drives the BAP connection attempt - and the
only writer we have seen put 5 there. Nothing about the dispatcher needs fixing.

### Next

Hook the two decision points with `r31` and the loaded value, so we read the actual
`+0x30`/`+0x34` at the moment of comparison rather than inferring it:

```
0x36314c  r31:0x40
0x363150  r31:0x40
0x363164  r31:0x40
0x363168  r31:0x40
```

Per the hook-format spec in `Invoke-ObjectExperiment.ps1` the dump syntax is
`rN[+OFF]:LEN` - **there is no `mN:` form**, which is why RUN R26 armed zero hooks
(`coverage 0`, zero `D1P` lines) while W128 and GUARD worked. `coverage 0` is printed by
every run regardless and is not a health signal.
## RUN GATE2 - the gate fields, measured directly. `+0x30 = 0`, `+0x34 = 0`, 60/60.

The gate function at `0x363138` **is** reached every run: it fired 60 times, always with
`lr=0x356bcc`, which is the return address of the dispatcher's indirect call at
`0x356ba4-0x356bc8`. So the dispatcher really does route into the gate - the connection
attempt is being made, not skipped. Reading the object (`r3` at entry becomes `r31` at
`0x363148`) gives, across all 60 calls:

```
+0x30 = 0    +0x34 = 0    x60
```

No variation whatsoever. The gate compares `+0x30` against 21 (`0x15`) and `+0x34`
against 3, sees 0 and 0, and takes the failure path at `0x3629ec` - which fired 40 times
alongside it. **The `+0x30 == 21` writer has never run in any run we have captured.**

### A second correction: the mid-function hooks do not fire

RUN GATE armed all seven of `0x363148/0x36314c/0x363150/0x363154/0x363164/0x363168/
0x36316c` with valid `rN:LEN` syntax. **None of them fired**, while `0x363138` (the
function entry, 16 bytes earlier), `0x3629ec`, `0x367090`, `0x366df8` and `0x356ab0` all
fired normally in the same run. The instrumented build only takes watches at **function
entry points**, not arbitrary addresses.

That invalidates a lot of earlier measurement. Any watch placed mid-function was never
live, so its "zero hits" meant nothing. This specifically means:

- `0x356108` never firing in RUN GUARD was **not** evidence that guard 1 was skipped.
  It was simply not an instrumented address.
- `0x366ea4`/`0x366df8`/`0x366b24`/`0x366b50` firing shows those *are* entries.
- The "success writer `0x366348` never executed" claim is **unproven** - it may simply
  never have been hooked correctly.

Hit counts in earlier notes need re-reading with this in mind: a zero can mean
"not an entry point" rather than "not reached".

### Hook syntax, settled by evidence

The label in the log (`m3=`, `m28=`, `m30=`) comes from the spec `r3:`, `r28:`, `r30:` -
the `m` prefix is just how the dump is printed. RUN R26 wrote `m30:0x140` in the spec
position, which is not the documented form, and armed nothing. RUN GATE2 used `r3:0x40`
and dumped correctly. `coverage 0` is printed on every reload regardless and is not a
health signal; the reliable check is that `D1P` hit lines exist for the addresses of
interest.

### Where this leaves the blocker

Three things are now established and mutually consistent:

1. The state machine runs `1 2 3 4 5 6 7 8 13 18 19` and terminates at 19 - healthy.
2. The gate function is entered 60 times per run from the dispatcher - healthy.
3. `+0x30` is 0 at every one of those entries and `+0x34` is 0 - **this is the blocker**.

The client wants `+0x30 == 21` before it will dial, and nothing in these runs ever sets
it. That is a *value* that some writer must produce, and the write is not happening at
all - as opposed to being written and then overwritten, which is what the earlier
"+0x30 gets set to 5" reading implied.

### Next

Stop hooking mid-function. The productive step is to find **function entries** that write
21 into `+0x30`, by scanning for the store-and-compare pattern rather than guessing
offsets: `li rX, 0x15` / `stw rX, 0x30(rY)` sequences, and the callers of the gate
(`0x363138` has `lr=0x356bcc`, i.e. it is reached through the dispatch table, so the
value must be set by whichever handler fills the object before the dispatch).

Concretely: locate the function that owns the dispatch table at `0x356ba4`, dump the
object it dispatches on at the **entry** of `0x356ba4`'s function, and walk its `+0x30`
across the whole run. If it is 0 from the first call, the writer is never invoked at all
and the search moves upstream to whatever constructs the object.
## Gate-chain correction (2026-09-30)

The state-zero diagnosis in commit `505e9c5` is withdrawn. The saved 01.24 ELF
has `crorc cr0eq, cr1eq, cr0eq` (word `4c461342`) at `0x362a0c`. The operation
is BA OR NOT BB, as defined by [IBM's instruction reference](https://www.ibm.com/docs/en/aix/7.2.0?topic=set-crorc-condition-register-complement-instruction).
For the preceding comparisons, this is `(state == 1) OR (state != 0)`,
which reduces to `state != 0`. Therefore zero falls through to the store
of one at `0x362a1c`; nonzero states skip that initialization. State zero
is not a terminal no-op, and a separate initializer is not the next target.

The Boolean wrapper at `0x332d24` was inverted too: `cntlzw` followed by
`rlwinm ...,27,31,31` returns one for a zero result, zero for nonzero.
`0x126a40` also indexes fields by `4*r4`; samples from other caller LRs
and indexes cannot establish the relevant return value.

The `RUN_GATE2`, `RUN_S21`, and `RUN_CHAIN` hook files remain valid records
of what the logger emitted, but do not prove that the guest body executed
on every hit. The hook was emitted before RPCS3's status check; scheduler
retries could spend the entire budget on an unchanged pre-execution state.
For example, the 60 gate records in `RUN_GATE2` span only 12,625 microseconds.
The diagnostic patch now emits after status admission and uses PPU cache
prefix `d1p3`. A rebuilt executable is required; the transferred emulator
still uses the earlier hook. The full patch passes an application check
against the imported RPCS3 baseline `b5a174e93`, but has not been compiled
on this PC, which lacks the Windows C++ build toolchain.

The [audit checkpoint](checkpoint-2026-09-30-final124-audit.md) contains the
current live observations and next experiment. Direct bootflow/resource
samples are preferable until the corrected emulator is built. The older
claims in the preceding sections are retained as an experiment history,
not as established causes of the stall.
