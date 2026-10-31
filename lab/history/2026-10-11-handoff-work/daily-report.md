# Earlier game content recovered; the exact executable remains unavailable

Reporting date: 11 October 2026, Europe/London. One consolidated private engineering
handoff. The metadata records the preparation cutoff in UTC. All results below
occurred on 11 October; previous PS3 packages are background references only.

Candidate public update: Work continued on preparing the next version of the
preservation project. Earlier game content has been recovered and checked, and
the local services have passed an isolated test. The game is not running on this
new setup yet, and no new gameplay footage is available today.

## Private engineering summary

The active target changed to the user-selected PS4 US CUSA00219 patch 01.29,
using the supplied preservation project and the existing research. The PS3
01.24 implementation remains a retained checkpoint. No PS3 mission or graphics
repair is claimed today. Both source/data repositories remain private; owned game
inputs were opened read-only and are not part of this production handoff.

The owned base package exposes a later executable identifying itself as build
56578.17.04.12.1251.evil. Its clear SELF flags do not solve the version mismatch.
The correct owned update chunks were joined and read-back verified, but their
retail PFS remains encrypted. Repack unwrap, zero-passcode and base-image reuse
checks did not obtain its image key. The target remains build
52553.16.09.28.1551.evil: decrypted ELF 29,148,264 bytes, SHA-256
7271fcb926401df8defb126cb8eb2b247134138b81e401bbacb2ab60791b7954.
No later executable was substituted. No console or third-party download was used.
The full parsed inode review found no unlinked regular-file record to recover an
earlier executable. This was a metadata review, not unallocated-sector carving.

The repack retained earlier Bungie package generations. Recovery selected builds
through 52553 and now supplies 1,647 packages, 49,917,028,352 bytes. Two defects
were corrected: modern .NET decompression could return a short read, and the
existing package owner lookup selected a same-ID/generation twin for local blocks.
The extraction loop now fills the sector; local blocks keep their exact file
identity, while inherited lookup and hash verification remain intact.

Full corrected content acceptance at 2026-10-11T13:38:06.4200226Z passes every
package layout/table and all 701,164 stored-block SHA-1 checks. The 52,797-byte
English manifest matches SHA-256
0c5bba3c63e9b68021b536513639cfc8896ec736950323e4ab06c5d7ed0770be.
At 13:39:16.6615499Z the documented default content paths were installed, with
the prior empty directory preserved. Rejected extraction/validation output remains
private and separate. This is content-only acceptance, not executable recovery.

The existing canonical run-boot.sh -> run-signon-experiment.sh now has Plan and
ServerCheck modes. Plan records actual exports after creator overrides.
ServerCheck exercises the existing Linux servers and sealed namespace without a
game, emulator, activity host or controller. Native preflight retains exact
executable/manifest pins. New preparation output and unrelated source revisions
are refused; server cleanup targets owned namespace children.

At approximately 14:22 UTC the actual server run passed 19/19 checks. Its
subsequent child-cleanup receipt at 14:22:34.7585225Z confirms both server children
are gone. The new narrow bootstrap collector also passes synthetic FTP controls
on Windows and Ubuntu. It is for a later, already isolated owned console; it
neither proves console isolation nor installs files. Metadata version remains
unverified, and no real console transfer has occurred.

## Tests and reproduction scope

Environment: Windows PowerShell with separately built Windows Rust tools and
Ubuntu 24.04 WSL tools. These are offline/content or synthetic server tests; no
PS4 native client was launched. The supplied project's GPL implementation stays
separate. Original helpers/minimal patches are tracked in the private source
repository. Final source checkpoint: 12467f31286da4e4a34dcf0610c9fad48f9b318d,
branch master, pushed and independently remote-verified; worktree clean at the
checkpoint and checked again during report preparation. The imported PS4 tree
has no Git metadata. Its supplied sources plus separately installed patches and
toolchains are required; the commit alone is not the full PS4 runtime.

- PASS: d1-pkg controls --json, 28 controls / 0 skips on installed defaults,
  also confirmed through the canonical Ubuntu entrypoint.
- PASS: d1-pkg verify --blocks, 1,647 packages / 0 failed and 701,164 stored-block
  checks / 0 failed. Full validation log digest and acceptance receipt retained
  locally. No full estate rescan is claimed at report time.
- PASS: 19 package library tests and 2 header-audit bin tests against installed
  content. Regression fails with the old owner lookup and passes with the fix;
  altered payload/digest negatives remain rejected. Exact crate patch replay and
  source-drift refusal pass.
- PASS: cargo test -p d1-pkg --bin d1-platform, 2 tests on Windows and 2 on Ubuntu.
  d1-platform controls: 10 controls on each platform, real curl against bounded
  synthetic loopback fixtures. Includes public PASV address ignoring, partial
  transfer retention, wrong digest, overwrite and path/name guards. A missing
  transfer client returns exit 2. Real console retrieval: NOT RUN.
- PASS: Prepare-D1PS4Launch.ps1 -Mode Plan produces the canonical contract;
  creator mode withholds the profile and pushes family 4, ordinary mode retains
  4,3. Preparation has no input. The supplied 20,318-byte profile fixture exists;
  its presence is not native acceptance or newly generated game data.
- PASS: Prepare-D1PS4Launch.ps1 -Mode ServerCheck, 19 actual checks: different
  namespace, both servers in it, only loopback/dummy interfaces, no IPv4/IPv6
  default route, immediate off-box TCP failure, failed upstream DNS, four service
  redirects, shadNet/BAP TCP listeners, exact HTTP/WebAPI/TLS manifest, untrusted
  TLS rejection, and unsupported-route 404. Both child processes exit cleanly.
  Dummy address 10.77.0.2/24 is inside that namespace. This does not validate
  Windows guest transport, UDP/STUN behaviour or a native BAP client session.
- PASS: wrong manifest exit 2, missing executable exit 1, same-size wrong
  executable exit 2, reused preparation output exit 2; none started the delegate.
  Canonical patch replay/idempotence, refusal and preservation of unrelated source
  edits, Bash syntax checks and PowerShell parsing/name guards pass.
- Expected NOT READY: Test-D1PS4Readiness.ps1 at 14:24:43.9949433Z, native exit 2,
  12 PASS / 2 FAIL. Missing exact executable and unaccepted native Windows/WSL
  runtime remain explicit. Firewall, redirects, local configuration, 37 existing
  firmware module hashes and installed canonical source pins pass. Firmware
  compatibility and stock-emulator private probe support are not established.
- Existing controls: responder 22 PASS; shadNet 32 PASS / 1 SKIP (requires emulator
  source); d1 controls 12 PASS / 1 FAIL (requires the actual eboot for crash
  symbolization). Do not convert skipped/blocked controls to passing results.

Early failed Windows FTP fixture logs are retained. Accepted sockets inherited
nonblocking state, and the deliberate truncated transfer needed to finish after
its failure reply; fixing those fixtures permits the controls above. The first
content copy and first full validation were rejected and retained. After the
two content fixes, the complete accepted scan passed. These are tool repairs,
not before/after gameplay or an emulator graphics repair.

## Maturity and evidence index

- Fully Working within tested content scope: selected estate recovery and stored
  byte/table verification, exact manifest generation and installed content paths.
  This does not establish loading or decoding all assets in the actual game.
- Fully Working within tested synthetic scope: platform transfer controls,
  canonical preparation, contained server checks and owned server shutdown.
- Experimental / Stubbed or Mocked: synthetic signon/service surfaces and the
  Windows runtime port. No playable gameplay or complete backend is established.
- Known Blocked: exact executable acquisition, dump-dependent native preparation
  and first contained Windows game run. Broader mission, graphics, progression,
  inventory and multiplayer completeness remain open.

This package contains daily-report.md, events.json and generated metadata.json
only, plus their hash manifest. No new video, screenshot or audio exists today.
Media decode and visual inspection: NOT APPLICABLE, not PASS. No new timecodes,
comparison frames or clean screenshot candidates are supplied. Confidence is high
for the recorded scoped test results; native gameplay remains untested.

Private local provenance references (not included payloads): corrected recovery
and acceptance/installation receipts; ownership regression and patch controls;
platform-preparation build/control/integration receipts; canonical plan, live
server-check and child-cleanup receipts; the readiness report. All raw paths,
logs, binaries, content, TLS materials and production instructions stay local.

Already delivered October 10 report:
<lab-pc>-20261010T181054-bbc9e7edfce14abfb991cee8485b098c.
Earlier PS3 JOIN252/253 clip packages:
<lab-pc>-20261010T180802-6a22199448334c9d9b7c118ae9ddbfb6 and
<lab-pc>-20261010T180808-5edaf0838d814fb9b5895bd5a98f1515.
These retain their actual recording dates and are not new PS4 evidence. Referenced
only; none of their media or ready files is copied, modified or resent.

## Remaining work and next acceptance

Retain the exact target while the console inputs are unavailable. Next: obtain
the owned matching decrypted executable/metadata through the prepared collector
when the isolated console is available; verify the immutable executable pin and
metadata version before installation. Then follow the supplied bootstrap/schema/
atlas generators and dump-dependent controls, preserve supplied fixtures, and
resolve the native Windows-emulator/WSL-server transport/probe conditions.

Acceptance requires the exact game executable, matching required modules and
metadata, passing dump-dependent checks, effective containment of the Windows
process and a bounded real game sign-in run with recorded evidence. A Linux
namespace cannot contain a Windows executable; mirrored loopback connectivity
and a static firewall check are not guest transport acceptance. Do not weaken
the identity gate or substitute the later build. First-mission PS4 behaviour,
Ghost/lighting/flicker fidelity, completion and multiplayer are NOT RUN.

## Privacy, audio, rights and handoff state

All files are private production inputs requiring downstream publication review.
Only the candidate public paragraph omits technical details. No microphone,
music or original-game reference footage is included, and no rights clearance is
implied for future game footage. No source game assets, extracted binaries,
credentials, private instruction files, raw logs or TLS keys are included.
No video editing, narration, final rendering, uploads or publication occurred.

The existing content-capture report command will queue this text-only report in
the durable local outbox, validate all payload sizes/hashes, and use the existing
verified NAS transfer and atomic uploading-to-ready transition. Actual transfer
identity, times, destination verification, free space and pending state are
recorded only after delivery in a separate local receipt. Ready packages remain
immutable; a failed transfer keeps the outbox for retry. No critical native test
is active. This report does not complete the engineering goal or trigger release.
