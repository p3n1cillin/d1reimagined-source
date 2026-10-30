# 01.24 continuation and diagnostic audit — 2026-09-30

The active target is BLES01857 **01.24**, not the historical 01.00 mission
milestone. Source baseline: `505e9c5` on `master`, matching `origin/master` after
fetch. The older imported checkout has additional uncommitted readers and notes;
it is preserved. Runtime: `<D1_ROOT>\main-pc-import\d1xenia\local`; data checkout:
`<D1_ROOT>\d1xenia-data`. The source checkout remains `<D1_ROOT>\d1xenia`.

## What survived independent review

The saved 01.24 bootflow samples from runs 019/022/023 identify stage 19,
`bootflow:bap_signin`, with stage-object substate 12 and resource slot 3 pending
at status 1. Run 020 reached substate 16 with that resource at status 2 under a
different, mostly disc manifest. These observations already existed locally,
but the 2026-09-29 checkpoint and its reader had not been committed.
Run 023's preserved sample is `FINAL124_SMOKE023-bootflow.json`; its previously
"pending" result is state 19, substate 12, bootstrap status 1, no BAP connection.

The current final-build preflight passes: exact installed 01.24 EBOOT and patch
identity, all 247 disc filenames, 1,034 required update packages at expected
sizes, 5,041 exported types, 94 service mappings, and both mission schemas.
Passing this check verifies setup and encodability, not live client acceptance.

## Corrections to the latest gate diagnosis

- At `0x362a0c`, `crorc` computes BA OR NOT BB. The zero-state path writes one
  at `0x362a1c`; the assertion that zero is a terminal no-op is inverted.
  [IBM's reference](https://www.ibm.com/docs/en/aix/7.2.0?topic=set-crorc-condition-register-complement-instruction)
  and RPCS3's local interpreter/translator agree on the operation.
- `0x332d24` applies `cntlzw`/`rlwinm` without a following inversion: it returns
  one when `0x126a40` returns zero. That callee indexes its fields by `4*r4`;
  entry samples from different callers/indexes do not establish this result.
- The existing hook logs before the compiled partition's scheduler status check.
  When the check defers execution, retries can consume the watch budget without
  executing guest instructions. In `RUN_GATE2`, 60 identical gate dumps span
  12,625 microseconds; 40 identical advance dumps span 7,682 microseconds. These
  are bursts, not independent observations throughout a session.
- A hook address identifies an RPCS3 compiled partition, which can start inside
  a logical guest function. A zero hit count at an arbitrary instruction does
  not establish that the instruction was never executed.

`patches/rpcs3-destiny-main-memory-blit.patch` now places the hook after status
admission and advances its PPU object-cache prefix from `d1p2` to `d1p3`.
The complete patch passes `git apply --check --cached` against source baseline
`b5a174e93`. The transferred executable remains the old build. No MSBuild/MSVC
toolchain is installed on this PC, so this patch has not been compiled or
deployed here; direct guest-memory samples are used for the current experiment.

## Tool and launcher repairs

`d1_offset_scan.py` now decodes displacement forms from primary opcodes, including
integer, floating, and DS forms. Indexed operations have no immediate struct
displacement and are excluded. DS subtype bits are masked separately, so `ldu`
and `lwa` references are no longer missed. `d1_eboot.py` uses supported PPC64
disassembly and handles those subtype bits in TOC references too. The prior
claim that Capstone silently rejects PPC64 mode was not reproducible.

Manifest selection can now use `--inventory-file` to exclude stale/unlisted
updates. Numeric suffix alone is not release chronology: the inventory marks
`ps3_investment_globals_012f_5.pkg` stale while final `_4` remains valid. The
smoke launcher supplies the verified final inventory automatically. Explicit
replacements remain experimental overrides. A missing update directory fails
instead of silently producing a disc-only manifest.

The run wrapper resolves paths from `Paths.ps1` and explicit parameters, uses a
fresh run name, saves process identities and protocol counts, and reports
launcher/preflight/service failures. It sends input only to its owned emulator
and cleans up only recorded processes with matching identity. Shared RPCN is
retained. Occupied debugger ports are refused instead of attaching to another
emulator. `-ManifestFile` permits controlled content comparisons.

The imported test copy's GPU adapter was changed from the transferred RTX 5070
name to this PC's RTX 2060 SUPER. Both config files were backed up locally before
editing. The old watch file was preserved and logging disabled for direct memory
sampling; no service reply or guest behavior was patched to force progress.

## Direct observation and next target

`scripts/rpcs3_final124_bootflow.py` promotes the earlier private reader. It checks
memory reply lengths and reports stage stability, stage substates, the eight
resource records, pending count, and optionally the bootstrap request/parent
chain and resource scheduler (`--request`). The auxiliary mode, readiness masks,
and bounded active descriptor summaries are always included. Samples are sequential,
not atomic, and belong under private `local/`. Each daemon memory request briefly
pauses/resumes the emulator; the stage guard runs after the optional reads.

The saved ELF identifies the state-12 wait precisely:

1. Resource base `0x1ada1b0`, slot 3 at `0x1ada1d4` (three u32 words: status,
   request handle, asset tag), pending count at base `+0x60`.
2. `0x531fe0(..., 3)` submits `client_bootstrap_patchable`; `0x53280c` maps
   resource statuses 1/3 to the pending mask. State 12 waits for its bit 3 to clear.
3. Poller `0x532718`, called from `0x35c050`, skips work when pending count is
   nonpositive. Otherwise it resolves handles and calls `0x14e864`.
4. That query reads signed request byte `+7`, following the parent handle at
   `+0xc` when kind byte `+6` is 2 and status is 1. A result <=1 remains pending;
   a result >1 invokes completion `0x531e78`. Result 3 is an error.

These are resource-load prerequisites inside the BAP boot step. Stage 19 alone
does not prove a STUN/NAT dependency, and unanswered UDP probes alone do not
identify its current blocker. Correlate the request and pending count before
making another reply-shape change. The reproduction and cleanup commands are
at the top of `next-session-playbook.md`.

## New run results

| Run | Inputs / change | Observed result |
| --- | --- | --- |
| `FINAL124_AUDIT01` | Initial repaired wrapper, default 311-entry manifest | Launch failed before RPCS3 started. The ownership record exposed an immediate-process metadata race; HTTPS upload aliases also attempted duplicate socket binds. Both defects were repaired. Three identified children with null executable identities required explicit cleanup; the new Python-child lifecycle tests cover that race. |
| `FINAL124_AUDIT02` | Inventory-filtered manifest (byte-identical to baseline), deduplicated listeners, disabled pre-status hooks | 240-second observation: one SignOn success, one manifest reply, 21 STUN probes/replies, zero BAP connections/frames. Direct samples agree on stage 19, substate 12, bootstrap status 1, pending count 1. Request kind 2/status 1 waits on parent kind 3/status 1. |
| `FINAL124_AUDIT03` | 160-entry disc manifest with only `0151` replaced by update `_4`, reproducing the earlier run 020 profile | 240-second observation: one SignOn success, one manifest reply, 21 STUN probes/replies, zero BAP connections/frames. Retained-session samples agree on stage 19, substate 16, bootstrap status 2, invalidated bootstrap request handle, and pending count 0. The separate asset-readiness wait is still active. |

AUDIT02's two independent samples are preserved as
`RESEARCH/ps3_updates/FINAL124_AUDIT02-bootflow-{1,2}.json`. The client-side
01.24 PPU hash was verified in the preserved emulator log. Its 311-entry
manifest SHA-256 is `d39d356b5dbf165f9ca9f24ce3df6e0588a8ebf315a5d6392584e8c990f7c3e7`;
inventory filtering changes no bytes on this already clean installed package set.
The final run index preserves 18 log fingerprints, 325 package-open events,
and two screenshots of the black spinner. The owned stack was closed after
sampling and the final emulator log was copied before another launch.

The paired parent/child handles are expected, not an infinite query recursion:
`0x1523c4` transfers kind-2 request lists to a new kind-3 request and stores the
reciprocal handles at both `+0xc` fields. Query `0x14e864` follows only kind 2.
The request snapshot includes the complete `0x44`-byte record (six list
descriptors and counters at `+0x40/+0x42`) to inspect the scheduler dependency.

SignOn listeners now deduplicate aliases by address/port; the HTTP Host and SNI
paths are unchanged. Run tooling records explicit executable paths at launch,
checks PID/start time/path before cleanup, and saves emulator logs with Windows
read/write/delete sharing. Capture also runs after failed runs, and retained
emulators can report a deferred capture without losing their session.

AUDIT03's comparison manifest is 7,925 bytes with SHA-256
`6903c8d4cb6420818a8356507ecfe944badfc0a904685ba4c9e0f7903233d037`.
It changes many package choices relative to AUDIT02; the comparison establishes
that resource completion is possible under a different profile, not that `0172`
alone causes the first wait. No guest state was written to bypass either wait.

The state-16 poll `0x3592d8` reads the auxiliary singleton at `0x01abc8b8`,
then its signed mode at `+0x80`, and calls `0x856d70` / `0x85619c`. The
initialized readiness table is at `0x01b558f0` (byte `+0xe8`). The poll waits
while `required[mode] & (0x7ff & ~completed)` is nonzero, with u16 mode masks
starting at the table and the u16 completed mask at `+0xba`. AUDIT03 observes
mode 2, required mask `0x0002`, completed mask zero, missing mask `0x0002`,
and initialized flag 1. This identifies a concrete asset prerequisite before
BAP; it is not evidence that changing a STUN reply can resolve it.

The resource scheduler reader also records manager queue/retry/active-handle
fields and the bounded active token record. In AUDIT03 it observes an empty
queue, zero retries, active handle `0x02fa0002`, token zero, and batch status
word 8. The word remains numeric until its phase and dependency are verified.

The label table at `0x018b7608` names bit index 1 **`ui_core`** (pointer
`0x01345280`). The active descriptor has content handle `0x80814000`,
loaded pointer `0x01b5ef00`, and allocation pointer `0x01b55b10`. The loaded
record's byte `+0xf` is zero; completion updater `0x8564e4` waits for it to
become one before publishing readiness bits at `0x856670`. The active kind-3
request `0x02fa0002` remains status 1 with counters `+0x40/+0x42 = 0/1`.
This narrows the next investigation to the pending `ui_core` load and its scheduler
phase. Intrusive list links alone do not identify the cause of the pending I/O.

AUDIT03 was retained for three consistent bootflow samples and then closed using
its ownership manifest. Its final index preserves 19 log fingerprints, 179
package-open events, two black-spinner screenshots, and no fatal events. By
cleanup there were 62 STUN probes/replies and still no additional SignOn or BAP
progress. The final closed record confirms no owned processes remain; shared
RPCN was left running.

## Validation and resume boundary

The server suite passes 82 tests and the scripts suite passes 59 (141 total).
`scripts/Test-Final124Run.ps1` passes its isolated Windows fixtures, including
process metadata timing, unrelated-process survival, interruption during
registration, hung-launcher child cleanup, shared log snapshots, capture after
failure, and deferred capture for an exclusively locked retained log. Independent
review found no additional actionable launcher issue. `git diff --check` passes.

The unresolved live boundary is before BAP, with content loading prerequisites
identified separately for substate 12 and substate 16. Reproduce the final
manifest and record the scheduler token/active request before making a focused
content or emulator change. The comparison profile is diagnostic and should not
replace the final manifest. The corrected RPCS3 trace patch still requires a
Windows C++ build environment before trace-hit counts can be trusted again.

All raw logs, requests, guest memory, screenshots, and generated manifests remain
under the ignored runtime. Only code, tests, and research summaries are committed.

## 2026-10-01: second PC, d1p3 build (Claude session)

**Toolchain.** The second PC now builds RPCS3 (see the README's "Moving to another PC"). The
d1p3 binary built here (`6d0217aa...`) and the main PC's staged one (`6a79e3e7...`) both
contain `kusa-d1p3`. The staged one is the one in use: swapped in as `rpcs3.exe` in
`RPCS3_FINAL124_SMOKE001` and `RPCS3_TEST048_SETUP`, with the old exe kept as `rpcs3.d1p2.exe`.

**FINAL124_D1P3_WARMUP / WARMUP2.** First boot on d1p3. WARMUP aborted at its first Cross
(45 s): `Send-RpcsKey` found no game window during the PPU recompile, so warm-up boots must
use `-PressSeconds 0`. WARMUP2 (no presses): the PPU recompile took about 3 minutes, not the
expected 20-40. At emulator time 3:12 the game was drawing (D1MEM surface 3M, Bungie logo).
The run was then closed through its ownership manifest. The wrapper's `.stop` file only
interrupts the launcher, not the observation loop.

**FINAL124_NEXT.** Died at 57 s: `dns_sinkhole.py` raised `ConnectionResetError [WinError
10054]` from `recvfrom` (Windows reports ICMP port-unreachable for an earlier reply that way),
and the wrapper aborted because an owned process had exited. Fixed in `1dc3f28`: the DNS, STUN
and session-listener receive loops ignore it.

**FINAL124_NEXT2** (final manifest `manifest-final-smoke.bin`, d1p3, 300 s, Cross for 150 s,
`-KeepRpcs3`). SignOn 1/1, manifest reply 1, 24 STUN probes (15 answered), 0 BAP. The
`--request` snapshot (`RESEARCH/ps3_updates/FINAL124_NEXT2-bootflow.json`) reproduces stage 19
`bap_signin`, substate **12**, slot 3 `client_bootstrap_patchable` state 1, handle
`0x01878002`, extra `0x80ae4000`, pending 1. The kind-2 request has status 1 and parent
`0x01fa0002`; that kind-3 parent is status 1, with the child handle at +0xc and
`0003 0003 00000001` at +0x10. The scheduler's active handle is `0x01fa0002`, and its retry
slot is `0x06fa0001`. **Expected = actual**: the wait reproduces on the corrected build.

**What slot 3 loads (static, 01.24 EBOOT).** `0x531fe0(mgr, slot)` reads the slot name from
`0x01315588[slot]` and resolves it with `0xe659c` -> `0xecc6c`. That function is a named-tag
search over the loaded packages' named-tag tables (68-byte entries: tag, class, char[60]
name), keeping the match from the package with the highest u64 at record +0x10. If the
lookup returns -1, the slot gets status 5. Otherwise it creates a request with
`0x15216c(..., 7, 2, 0)` and stores the handle at +4 and the tag at +8, which is the
snapshot's "extra". So **`client_bootstrap_patchable` = tag `0x80ae4000` = package
`0x0172`, entry 0** (by `d1_pkg.tag_hash`).
- The request handles `0x01878002` / `0x01fa0002` / `0x06fa0001` are **request-pool
  handles**, not tag handles: group = (h >> 13) & 0x3ff (0x3c, 0x3d0), item = h & 0x1fff, pool
  stride 0x48, descriptor table `*(*0x01896d50)`. The handoff's "handle >> 13 = package index"
  applies to tags, not to these handles. The records hold no tag. The tag lives only in the
  slot's +8.

**Package 0172 in FINAL124_NEXT2's RPCS3.log.** Both `_1` (01.13, 12.5 MB) and `_2` (01.20,
38 KB overlay) are `stat`ed at 1:56; `_1` is **never opened**. `_2` is opened at 2:39 on
async_job (644 bytes read, closed), then reopened at 2:47 on `fios mediathread 2` (fd 3) and
`fstat`ed, with no close before the 4:54 snapshot. Right after it the same thread opens the
**disc** `ps3_shared_manifest_0430_0.pkg`. This RPCS3 build aggregates `sys_fs_read` (`⁂
sys_fs_read [944]`), so the log cannot show whether `_2` was read after 2:47. The 09-29
checkpoint's run 022 (explicit `_0`+`_1`+`_2` chain, all three opened) also stopped at
substate 12, so chain registration alone is not the answer.

**Next, smallest first.** (1) Read the `0x80ae4000` load state live, meaning the tag's
resolution in the package registry for 0172, to see whether the overlay `_2` resolves entry 0
into `_1`'s data and whether FIOS has a pending read on fd 3. (2) Compare against run 020
(substate 16, slot 3 done): which 0172 package that manifest served, and whether it used the
disc 0430/0431 shared manifests.

### 2026-10-01 (cont.): package registry live, three manifests on d1p3

**Registry (static analysis from the main PC, checked live).** Pool `*0x1896510` (records at +0x20,
stride 0x1a8; 314 records in the final manifest), u16 id -> index map at `0x19c2c60`, head/count
`0x19bac30`/`0x19bac34`, so there is one record per package id. Live record fields: +0x04 id,
**+0x10 = the package's build time** (equal to the .pkg header's build time; 0xecc6c prefers the
newest), **+0xb4 = entry count** (not a block size), +0xec named-tag count, +0x140 the
`packages/<name>.pkg` path, +0x178 tag-table offset, +0x17c resident header block.
`rpcs3_final124_bootflow.py --package HEXID` now dumps it (`--request` adds 0172), as of `acc4d94`.

**Package 0172 offline.** `_2` (01.20) is an overlay. Of its 118 blocks, 68 are stored in patch 0
(the disc `_0`), 49 in patch 1 (01.13 `_1`) and 1 in itself. Entry 0, the root tag `0x80ae4000`
(48 bytes, pointing at entries 2 and 1), sits in a patch-0 block. `d1_pkg.py blocks --verify` on
`_2` with `_0`/`_1` alongside: 118/118 SHA-1 match, so the shipped chain is internally
consistent. Entry 0 is byte-identical in `_1` and `_2`. A word scan of all entries for
cross-package tags is too noisy to isolate a missing dependency (about 230 plausible packages in
both `_0` and `_2`); not pursued.

| Run (d1p3) | Manifest | 0172 record (+0x140, build time, entries) | 0172 files opened | Stage 19 | Slot 3 | Active request |
| --- | --- | --- | --- | --- | --- | --- |
| FINAL124_NEXT2 | `manifest-final-smoke.bin` (`_2` only, disc shared) | `_2`, `0x56fcaa7c`, 949 | `_2` only (async + FIOS); `_0`/`_1` never | substate 12 | 1, `0x01878002` | `0x01fa0002` |
| FINAL124_CHAIN_D1P3B | `manifest-final-bootstrap-chain.bin` (`_0`+`_1`+`_2`, shared `_4`) | `_2`, `0x56fcaa7c`, 949 | all three, by async and by FIOS (`_2`, `_1`, disc `_0`) | substate 12 | 1, `0x01878002` | `0x01fa0002` |
| FINAL124_DISC0172_D1P3 | `manifest_0151_override.bin` (run 020; `_0` only) | `_0`, `0x53b251f2`, 943 | disc `_0` (async + FIOS) | substate **16** | **2**, pending 0 | `0x02fa0002` |

**Conclusion.** The substate-12 wait follows the 01.20 version of package 0172 (`_2`). Making its
base files reachable (the explicit chain) changes which files FIOS opens, but not the outcome: the
same kind-2 request, its kind-3 parent `0x01fa0002` and the retry slot `0x06fa0001` remain. With
the disc `_0` the slot completes (run 020 reproduced), but the final manifest with `0172_0`
crashed in run 021. The next question is what the kind-3 request `0x01fa0002` waits on while
`_2` is registered. Next test: hook-trace the request poller `0x14e864` and the status writer, or
diff first-call coverage between FINAL124_NEXT2 and FINAL124_DISC0172_D1P3 around the load.
`01a3` also differs (`_1` vs `_0`) between the two manifests.

### 2026-10-01 (cont.): first-call coverage, stuck vs passing (d1p3)

FINAL124_COV_FINAL (final manifest; substate 12) against FINAL124_COV_DISC0172 (run 020
manifest; substate 16), each 420 s with `coverage` in `RPCS3_FINAL124_SMOKE001\d1\d1ppu.txt`
(restored afterwards). About 82k first calls each, 81,838 shared. `scripts/d1_coverage_diff.py STUCK
PASS --after 0:01:20` lists 3 blocks only in the stuck run and 746 only in the passing run.

- **Stuck run only:** `0x103b68/0x103b90/0x103ba4` (job #1, 114.8 s), the retry arm of a
  lock-free CAS loop (`lwarx`/`stwcx.`, up to 10,000 tries) in `0x103828`. Contention: a symptom,
  not a cause.
- **Passing run only, from 112.29 s:** partitions inside `0xb7134c` called from `0xed458`, a
  20-byte digest check (`sha1(data, len) == expected`, called by the header loaders `0xeaf6c`
  and `0xebd68`). Its padding branch depends on data length, and `_0`'s tables differ in size
  from `_2`'s, so this difference is probably not causal.
- **Passing run only, from 119.46 s:** the completion path, starting at `0xf13d8` inside
  **`0xf12a8`** (called from `0xe6d28` and `0xe7f40`), which both runs call. Then the stage-19
  tick advances (`0x362e74`...). The partition at `0xf13d8` is reached only when the u16 at +0x30
  of the entry `0x19c6520 + idx*0x44` (idx = `0xf12a8`'s 8th argument, r8/r20) is **not**
  `0xffff`. Lead: in the stuck run that entry stays empty. Next, read the `0x19c6520` table live
  in a stuck run and identify what fills +0x30, and which caller (`0xe6d28` or `0xe7f40`) passes
  the index.

**Correction (FINAL124_SLOTS_STUCK, live, same session):** `0x19c6520` is not a stream table. Its
three real 0x44-byte entries are **memory arenas**: base `0xc0000000` (RSX local memory, size
0x0d91f000, used 0x1f7900), `0x30aa1d80` (0x6ff000, used 0) and `0x311a1e00` (0x4ff000, used
0x44100). +0x20 is the granularity, +0x30 the u16 head of an allocation list (`0xffff` in all
three while stuck). Past entry 2 is unrelated data. `0xef20c` returns size minus used, so
**`0xf12a8` is an arena allocator** (fallback arena via its 9th argument; errors -0x46, -0x4e,
-0x51). The passing-only `0xf13d8` path walks an existing allocation list, which differs because
the loaded data differs. **Like the SHA-1 padding branch, this is data-driven, not shown to be
causal.** Nothing in the stuck run is out of memory. First-call coverage cannot isolate the
blocker, because the stuck run has no unique error path. Next approach: trace the lifecycle of the
kind-3 request `0x01fa0002`, i.e. which code writes request status byte +7 and what the scheduler
does with the active request each tick, using d1p3 hooks with register dumps on the status
writers.

### 2026-10-01 (cont.): request lifecycle trace, the loader goes idle after shared manifest 0430

**Status writers (static, 01.24).** A capstone sweep for `stb rX, 7(rY)` in functions that
use the request-pool table finds the lifecycle. **`0x1523c4(manager, handle)` submits** a
request (status 1). **`0x14eca0(record)` completes** it: status 3 if the error byte +9 is set,
status 4 if a flag in +8 or a pending kind-3 parent says so, otherwise 2. Its callers are the
workers `0x14f690`, `0x14ffb4`, `0x150470`, `0x150584` and `0x150d9c`.

**FINAL124_REQTRACE** (final manifest, d1p3 hooks on those 7 functions with `r3+0:16`, watch
file restored afterwards). Every load request follows the same cycle: submit `0x1523c4` (the
kind-2 child from `0xd1ab10`/`0x532160`, then its kind-3 parent from `0x15464c`), then start
`0x14f690`, then two async callbacks `0x150584(r4=0xb)` and `0x150584(r4=0x17)` 30 ms to 1.5 s
apart (chain `0x15392c` -> `0x1507a8`), then complete `0x14eca0` with status 2. Slot 3
(`client_bootstrap_patchable`) goes through this **many times**: handles `0x00878001` up to
`0x06878001` from 0:32 to 1:36, then generation-2 `0x00878002/0x01878002`. Its records
`0x6087be48`/`0x6087be90` complete each time. The final submission is at 1:37.933 (stage tick
`0x362dbc` -> `0x532160`: child `0x01878002`, kind-3 parent `0x01fa0002` = record
`0x6087be90`), and `0x14f690` starts the parent at 1:37.965. **After that, neither
`0x150584` callback ever arrives and `0x14eca0` is never called for it.**

**The FIOS media thread at that moment.** At 1:37.966 it opens `0172_2.pkg` (fd 3) and
`fstat`s it, then opens and `fstat`s disc `ps3_shared_manifest_0430_0.pkg` (fd 4), and **logs
nothing more for the rest of the run**. Two thread snapshots 100 s apart
(`FINAL124_REQTRACE-threads{1,2}.json`) show both FIOS media threads (`0x1000003`,
`0x1000004`) at the same PC `0x01ca5ae0` with the same stack, and the job threads parked at
`0x10498c`. **The loader is idle, not blocked in I/O**: the last operation's completion was
never delivered, or it failed without notifying the scheduler. The same tail appears in every
stuck run: FINAL124_NEXT2 (disc 0430_0, 2:47) and FINAL124_CHAIN_D1P3B (update 0430_4). In the
passing run (FINAL124_DISC0172_D1P3) FIOS reads `0430_0` at 1:37.45, then `0431_0` at 1:37.88,
then carries on (`ps3_globals_audio_0161_0`...).

**Shared-manifest experiments.** The registry has disc `0430_0` (8,192 entries) and `0431_0`
(3,967), and no `0x0432`: shared manifest 0432 shipped only in updates (01.13+) and is in no
manifest. `0430_4` and `0432_4` share registration value `0x3c1c37cc3517fdbc`. Variants built
from an exact rebuild of `manifest-final-smoke.bin` (generator kept in the session scratchpad;
outputs in `local/RESEARCH/ps3_updates`):

| Run | Change vs final manifest | Registry 0430/0431/0432 | Result |
| --- | --- | --- | --- |
| FINAL124_SHARED_A0432 | + `0432_4` | `_0` / `_0` / `_4` | substate 12; FIOS stops after opening `0430_0` |
| FINAL124_SHARED_B4 | `0430_4`, `0431_4`, `0432_4` | `_4` / `_4` (8,192) / `_4` (3,617) | substate 12; FIOS stops after opening `0430_4` |

**Conclusion.** Registering shared manifest 0432, or the 01.23 shared-manifest set, does not
change the stall. With 01.20's `0172_2` registered, the FIOS operation that reads shared manifest
0430 never completes; with disc `0172_0` it does. Next: see what that read is. This RPCS3 log
collapses `sys_fs_read` lines (`⁂ sys_fs_read [N]`), so raise sys_fs logging, or hook the
game's FIOS read submission, to capture the fd, offset, size and result of the reads on fd 4
after 1:37.97.

### 2026-10-01 (cont.): ROOT CAUSE of the substate-12 wait, fixed by registering patch chains

**What the stalled read is.** With `sys_fs: Trace` added to the per-game config's `Log:`
section (the per-game file overrides the global one; restored afterwards), FINAL124_FSTRACE2
shows the last FIOS work on disc `0430_0`: tables (`lseek 0x1000`, read 0x20000 + 0x180), then
`lseek 0x22570`, read 0x10c, then nothing. That is **entry 14 = tag `0x8106000e`**, the `ref`
of 0172's root entry `0x80ae4000` (the same in `_0`, `_1` and `_2`). Its 268 bytes (class
0x80800341, naming tag `0x80ae4000` and package `0x0172`) end in a 30-word bitmask. The two
versions differ in one word only:

| Copy of tag `0x8106000e` | Last mask word | Bits = entries |
| --- | --- | --- |
| disc `0430_0` (2014) | `0x00007fff` | 29*32 + 15 = **943** = `0172_0`'s entry count |
| updates `0430_1`..`_4` (01.13+; data in a **patch-1 block**, i.e. in the `0430_1` file) | `0x001fffff` | 29*32 + 21 = **949** = `0172_2`'s entry count |

`0172_2`'s entries 943-948 were added in 01.13 (patch-1 blocks). Entry 948 (862 KB, type 8) is
referenced by entry 188, so the 01.20 root needs it. With the old 943-entry mask the load can
never complete. That is the substate-12 wait: no error, the loader just goes idle.

**Why every earlier manifest failed.** The content manifest must register each package's
**patch chain** (all patch files its blocks live in), not just the newest file:
- final / `_2` only: neither `0172_0`/`_1` nor the updated 0430 mask is reachable;
- run 022 (0172 chain + shared `_4`): `0430_4` keeps entry 14 in patch 1 (`0430_1`, not
  registered), so it is unreachable;
- FINAL124_SHARED_CHAIN (shared chains only): FIOS opens `0430_4`..`_0` and reads entry 14 **from
  `0430_1`** (fd 10, offset 0x21ea0, 268 bytes: the 949 mask), then needs `0172_0`/`_1` blocks
  that are not registered;
- run 020 / disc `0172_0`: the old 943 mask matches the old package, so slot 3 completes.

**The fix, verified live: FINAL124_BOTH_CHAINS** (`manifest-final-Dboth-chains.bin`, 325
entries: the final manifest, plus the `0172` chain `_0` (disc), `_1`, `_2`, plus the shared-manifest
chains `0430_0`..`_4`, `0431_0`..`_4`, `0432_1`..`_4`, chain entries listed before the newest):
**stage 19, substate 16, slot 3 `client_bootstrap_patchable` state 2, pending 0**, with
01.24's final content. FIOS goes on to the globals packages (`0156_4`, `0157_4`, `0161_3`,
`0169_2`, `072b_3`, `0735_1`...). Still 0 BAP connections: the next wait is substate 16's
asset-readiness descriptor (mode 2, content handle `0x80814000`), as in run 020.

**Next.** 284 of the final manifest's 311 packages are overlays. Registering every chain from
the block tables would add 514 entries (825 in total, no files missing). The 800-entry cap in the
main PC's `--chain` variant has no recorded source; test it live. Add an automatic block-table
`--chains` mode to `build_manifest_probe.py` (patch 0 from the disc, others from the inventory's
non-stale update files; marker-0 shared manifests included). Then re-check substate 16.

### 2026-10-01 (cont.): FIRST 01.24 BAP CONNECTION, with every patch chain registered

`build_manifest_probe.py --chains` (`b240eba`): the final manifest plus every package's
block-table patch chain plus the shared manifests, **837 entries** (`manifest-final-chains.bin`).
The client accepts them all (registry count 840; the "800 maximum" does not apply).

**FINAL124_ALL_CHAINS: stage 19 reaches substate 21**, with slots 3 (`client_bootstrap_patchable`)
and 7 (`investment_globals`) at state 2 and pending 0. Asset readiness: mode 3, mask 26, three
descriptors (content handles `0x80814000`, `0x808a0000`, `0x80822000`). **The client opens its
first BAP connection** (wrapper: `REACHED BAP (protocol frame received)`, 1 connection, 1
frame) at 09:33:02, 1:46 after launch. The stage-19 blocker was package registration,
not STUN: patch chains are mandatory for 01.24 content.

First 01.24 BAP frame (`FINAL124_ALL_CHAINS-bap.log`): outer kind 2 (plaintext), **type
`0x1e`**, id 0, **128-byte payload starting with the ASCII magic `DESTINY` and byte 0x01**,
then 120 bytes that look random (key material or a nonce?). 01.00 opened with type `0x19`
"shello req" (protobuf with the SignOn token). The stand-in has no handler for `0x1e`, sends
nothing, and the client closes the connection after 10 s ("closed with 0/6 bytes"). **Next:
identify the 01.24 BAP handshake** (message-name table and the `0x1e` codec in the 01.24
EBOOT), then answer it in `bap_server.py`.

### 2026-10-01 (cont.): 01.24 BAP session: start req answered, sign-in through family push, then "pigeon"

**01.24 BAP message types** (jump table `0x134f3d0` in the name function `0x8c7f48`, 0x130
entries): the same numbers as 01.00 for shello (0x19/0x1a), c->ws (0x0a/0x0b), sub (0x0c/0x0d),
client config (0x12/0x13), qz_reg (0x79/0x7a) and rrc (0x12e/0x12f). New in use: **0x1e "c->b
start req" / 0x1f "c->b start rsp"**, plus 0xab "c->x not" and 0xfa/0xfb echo.

**start req / start rsp.** Builder `0x8c74e4`: memcpy of `"DESTINY\x01"` (data at `0x134f270`),
then 60 u16 values from an LCG (x*0x19660d + 0x3c6ef35f, seed from `0xbf1d58`): 128 bytes, sent
plaintext. Reply dispatcher `0x8c7c94`: pending request kind 1 -> expects 0x1f, kind 2 -> 0x1a,
3 -> 0xfb, 4 -> 0x7a, 5 -> 0x12f. Status must be 200, then the codec decodes the body (vtable
slot 3 of `*(0x189a548 + 4*type)`, live). The **start rsp decoder `0x478764` accepts exactly
0x80 bytes** and the dispatcher discards them; qz_reg/rrc rsp decode with `0xa6220` (ignored).
FINAL124_START1 with an empty body: the client closed after 10 s. **FINAL124_START2 echoing the
128-byte request** (`bap_server.py` default now): accepted.

**FINAL124_START2 session** (`manifest-final-chains.bin`, d1p3), on the existing 01.00 stand-in
logic with the 01.24 schemas: start req -> start rsp (echo) -> **shello req -> shello rsp** ->
qz_reg -> rrc -> **c->ws 503 sign-in** (account `0xa3d4a2c8fbea6b9`) -> frame 6 type 0xab
"c->x not" (86 bytes, protobuf-like, no reply expected?) -> c->ws `0x00ce` family fetch -> sub req
-> pushed family type 4 (1,972 bytes, root object) -> **the client shuts the socket down 50 ms
later** (`sys_net_bnet_shutdown(s=80)` at emu 1:46.05). Screen: **ERROR "Your connection to
Destiny character servers has been lost. ... error code: pigeon"**. Bootflow state **25
"cleanup"**: stage 19 `bap_signin` was passed. Next: find why the client drops the session after
the family push (01.24 family/character object layout vs the 01.00-derived push, the 0xab
notification, or a missing push or reply), and compare with the 01.00 session order (client
config request, acct id trans...).

Run automation: `Start-FinalBuildSmoke.ps1` now builds its default manifest with `--chains`
(`manifest-final-chains.bin`).

### 2026-10-01 (cont.): "pigeon" fixed (01.24 qz object type hashes); the client reaches profile_setup

- **What closes the session.** d1p3 hooks on the BAP client (FINAL124_PIGEON): at 1:45.788
  `0x476550` -> `0x8c6bc8` sets BAP error `0x20` and state 9. That is a deliberate teardown
  (`0x476550` closes any active session); the chain runs through the bootflow stage tick
  (`0x36ab3c` -> `0xe4fc38`) after the bootflow had already switched to cleanup. The BAP error
  field was still 0, so the decision comes from stage 20 `investment_signin`. Bootflow stage
  names: table at `0x012f7f00` (19 bap_signin, 20 investment_signin, 21 profile_setup, 22
  prepare_for_orbit, 23 character:signin, 25 cleanup, ...).
- **Not the stored character.** 01.24 runs now use their own store `characters-0124.json` (the
  shared store holds a 01.00 creation request). FINAL124_NEWSTORE2 (no character) failed the
  same way.
- **The family push is the trigger.** FINAL124_NOPUSH (`--no-sub-push`, passed with the new
  `Invoke-Final124Run.ps1 -BapExtra`): the session stays up, the bootflow waits in
  `investment_signin` substate 7, and the client re-subscribes. With the push it quits 150 ms
  later.
- **Cause: the qz object type hashes.** Each qz object header carries a type hash that is a
  layout fingerprint. 01.24 changed the hash of every type that grew. 01.24 values from the type
  descriptors `{..., 0xf4, hash, 0, wrapper handle, size}`, found by wrapper handle + new size;
  the method returns the unchanged hashes of the two types that did not grow:
  self root `0x80801803` `0x88190FE1` -> `0xB5DF2907`; select character `0x80801954`
  `0xF65ABD52` -> `0x7D473C17`; self character `0x80801743` `0x242B2F61` -> `0xEB4B8439`; peer
  character `0x808017FD` `0xA0A1BC80` -> `0x30D274D2`; unchanged `0xA2AC2C74` (`0x80801951`)
  and `0x1D7BB8F6` (`0x80801921`). `bap_server.object_hash()` picks the hash by the schema's type
  size (01.00 behaviour unchanged).
- **FINAL124_HASH1:** push accepted, session kept, **bootflow stage 21 `profile_setup`**,
  screen **"SET SCREEN BOUNDS"** (the first-run profile setup: press X). Next: press through
  profile setup (the wrapper's Cross presses stop at the first BAP connection; add a phase that
  keeps pressing), then prepare_for_orbit -> character:signin -> character creation on 01.24.
