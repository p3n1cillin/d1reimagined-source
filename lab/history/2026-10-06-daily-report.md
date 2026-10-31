# Graphics stability testing advances; character and mission acceptance remain open

Reporting date: 2026-10-06, Europe/London. Evidence cutoff for this handoff is
2026-10-06T17:52:00Z. This is one private source handoff, with later results retained
under their actual dates and the next eligible daily edition. No release is approved.

Candidate public update: Testing has cleared two graphics errors. Character
rendering and mission playability still need work, and further testing is underway.

## Engineering changes and maturity

The active title is PS3 BLES01857 01.24. Earlier GPU checkpoint work now retains
both graphics/compute bindings, command counts and DMA readback bounds. Native
returned markers narrow a command region but do not identify a faulting shader.
Maturity: Experimental diagnostics, with no gameplay or pixel-fidelity acceptance.

Core Vulkan validation then identifies concurrent host use of the same queue by
presentation/submission and unmatched query scopes. Queue access now uses one
exclusive guard with RAII release; failed submission results propagate before a
fence is marked flushed. Queries use the existing matching outside-pass path on
every driver. These two isolated repairs are Partially Working in the bounded
fixture. Overall rendering remains Known Broken because descriptor-pool resets
and depth-feedback/draw-layout errors persist.

GC prefix selection and removal now occur under one exclusive map lock. CPU writer
interleavings prove the previous reader-upgrade path could discard pending scopes.
The native comparison still reports descriptor resets, so this Experimental repair
has not resolved their observed cause. No performance improvement is claimed.

Status-file lock contention previously terminated the automatic capture worker.
Updates now retain pending fields, preserve the old timestamp on failed status I/O
and retry without stopping the observer. Its error handler also survives an
unavailable status sink. This is verified telemetry resilience. A later native
recorder exit is separate and remains unresolved; uninterrupted long-run capture
is not established.

The next renderer increment preserves pending events after unsuccessful fence
waits, requires confirmed completion before buffer/frame/DMA reuse, and retains a
DMA fence if waiting throws. Its actual-code CPU checks and full Release build pass.
Native JOIN154 is currently testing it. Maturity: Experimental; native graphics
acceptance is NOT RUN/pending at this report cutoff.

## Tests and reproducible native scenario

Private renderer base: b5a174e933a083389e2f851005e474ccb3e676d9, with earlier project
patches and unrelated private changes retained. Scoped exported patches reproduce
the exact LF-normalized changed sources against saved pre-change snapshots. Public
checkpoint f285449843af996e87885faf06b87cc4b9baa257 is pushed on master. The subsequent
three-file fence increment is uncommitted at this cutoff; the public commit alone
does not reproduce the tested private binaries.

| Check | Actual result and limit |
| --- | --- |
| Queue actual-C++ CPU audit | 48 PASS; exact baseline 15 FAIL. Mock API/ownership checks, not a GPU test. |
| Query actual-C++ CPU audit | 29 PASS; exact baseline 7 FAIL. Four driver policies and query rearming checked. |
| GC actual-C++ CPU audit | 14 PASS; exact baseline 3 FAIL. Controlled documented writer interleavings. |
| Fence completion actual-C++ CPU audit | 71 PASS; exact baseline 51 FAIL. Timeout/error retention and cleanup callers checked. |
| Native builds | Full MSVC x64 Release builds for all four increments PASS. Existing conversion/post-build warnings remain. |
| Capture suite | `python -m unittest discover -s tools/content_capture -v`: 73 PASS; contention, concurrent pending fields and failed status I/O included. |
| Live capture status locks | Six-second held locks both idle and recording: exact worker/recorder survive and fresh status resumes. This does not prove all encoder output. |
| Scoped patch reproduction | Queue, query, GC and fence increments apply to their exact baselines and match changed sources after LF normalization: PASS. |

Native fixture: Titan starter 01.24, highway world 113 baseline, original PPU watches,
mission/heartbeat proxy, already-watched opening cinematic skipped, and 500 seconds
after match. Only the title debug-output switch and process-local Khronos core layer
are enabled. Runtime 1.4.341.1 is confirmed loaded in the exactly owned clients;
GPU-assisted shader instrumentation is not requested. Callback duplicate suppression
means reported counts are not total occurrence counts.

| Run and exact executable SHA-256 | Native evidence |
| --- | --- |
| JOIN150, 090cf2ff657ed6a745e734b0a07bbd29262533626dc2f5f655f3f08e380d7808 | Query error 2:59.183349, queue concurrency 3:01.091090, SPU device loss 3:09.410591: FAIL. No useful accepted finalized video. |
| JOIN151, d415a3455871a84642047b852479372271b2b6ae6a5dd48f339b506bb01e862f | Final 11:55.154100; queue callbacks/device-loss entries 0. Query errors, four descriptor resets and layout errors remain. Overall conformance FAIL. |
| JOIN152, e74967ad48b6fc852f58c9fc48cfeda82c3666784c94281c01c57d4c4c2ded50 | Final 14:52.575180; queue/query/query-pass-closure/device-loss entries 0. Three descriptor resets and layout errors remain. Overall conformance FAIL. |
| JOIN153, 4fa0276a3703ceebd53029ccaec746ebf4c10ac3ed9dc56648f7b384e83fe047 | Same fixture completes, then extended unattended time; final 2:41:22.713699 with queue/query/device-loss entries 0. Two descriptor resets and layout errors remain. Overall conformance FAIL; elapsed lifetime is not active-play evidence. |
| JOIN154, 051e8eb5d7e53ab322918111c51c707f00a0a05e5ed09354372edc59b44026c8 | Live at cutoff; acceptance pending. Private launcher automatically snapshots and restores its exactly owned stack on completion. |

JOIN150–153 cleanup PASS: exactly owned clients/services/proxies/launchers closed,
original watch/title hashes match, XInput restored, diagnostic controls inactive and
shared RPCN retained. JOIN154 is active and intentionally not yet cleaned.

## Evidence index: reference existing packages

All three packages below already exist in today's machine/date folder on the
configured 500 GB share. They are private inputs. Media is referenced rather than
resent; no new footage is fabricated for unchanged capability or a clip quota.

| Package ID | Useful timecodes and what it establishes |
| --- | --- |
| <lab-pc>-20261006T065220-7641e8704d0841978e0c575aad433c87 | 00:00–01:00.42; native video/audio context around an owned validation-layer host crash. It does not prove GPU device loss or a graphics repair. Post-event coverage is truncated. |
| <lab-pc>-20261006T082554-05a38c92bf8d4457a36e9cf2b57d4324 | 00:00–01:24.25; three distinct segments including verified complete fragments from an interrupted recording. Original interruption/recovery limitations remain explicit. This corrects an earlier superseded integration package. |
| <lab-pc>-20261006T132502-7aa19388507d4333a577cfb19994ea20 | 00:00–01:00.42; preparing/message UI around returned diagnostic markers. It does not visibly prove repaired body rendering or a playable mission. Footage ends 3.45 seconds before event detection. |

The latter package's seven files and six payload hashes/sizes were independently
verified. All three package metadata records specify full decode PASS and native
audio present. Existing metadata carries actual timestamps, gaps and sensitive
review flags. No source material or manifest in a ready package is changed here.

Inspected native comparison screenshots: JOIN151 shows an urgent-message dialog
over the world; JOIN153's final view is black/loading. Character-selection imagery
contains the local test profile and native screenshot overlay. These are private
diagnostic candidates with no clean public screenshot selected or approved. No
before/after visual repair pairing is established by these images. Original-D1
reference provenance: NOT RUN; no reference footage was acquired.

## Capture limitations and rights

The status worker exits around 14:41 UTC during JOIN152 and restarts at 14:48 UTC;
that coverage gap is missing. JOIN153 then has 63 automatically decoded and
hash-verified segments from 14:52:24 to 15:24:24 UTC, with one initial unusable segment.
Its native recorder later exits separately. Later coverage is unproved, and the
remaining long original stays local for diagnosis. Native trailer-error messages
also occur for segments that pass full decoding; those messages alone do not prove
unusable media. The corrected observer remains alive and resumes recording for
JOIN154 without user intervention.

Existing clips are native landscape game/audio, approximately 1280×720 at 30 fps;
no upscaling or interpolation is claimed. Rotation gaps are explicit. Microphone
recording is not claimed. Music/rights assessment: NOT RUN. Technical imagery,
local test-profile text, raw runtime logs and provenance are private, not cleared
for publication. Report and media require downstream privacy/rights review.
No final editing, narration, subtitles, social uploads or website publishing occurs
on this machine. Instruction files, extracted game assets and native binaries are
excluded from this report package.

## Remaining work and next acceptance

Full-body/equipment rendering, flicker/distortion, stable outdoor/indoor views,
mission handoff and combat remain unresolved. No mission completion, enemies/AI,
multiplayer or complete-game claim follows from these isolated checks.

Next: finish the same native fence comparison and correlate any descriptor reset
with its actual command buffer and completed event. Diagnose depth-feedback layout
transitions and the recorder's source-identification exit separately. Accept a
rendering repair only after consistent native validation and visible body,
equipment, outdoor and mission evidence. Keep mission publication/transport
return-path investigation separate; wrapper acceptance does not prove wire delivery.

## Handoff state

Eleven immutable ready packages are present before this consolidated report, three
dated today; local outbox/review counts are zero and the 500 GB share is connected.
This report uses the existing durable outbox and hash-verified `.uploading` to
`.ready` transfer. Actual creation/transfer verification and package ID are recorded
in the private handoff receipt after submission. No transmission result is marked
PASS before it runs. Continued engineering and later evidence are separate from
this immutable daily handoff; production decides whether the evidence supports an
edition and performs all release work.
