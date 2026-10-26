# PS4 retarget to the owned build 56578 — 12 October 2026 (evening)

On 12 October the user dropped the exact-01.29 executable pin, superseding the
11 October "keep exact 01.29" decision. The PS4 line now targets the executable
the owned repack actually contains. 01.29 stays registered as the **address
reference**: every fixed address in the PS4 tree's `design/`, probe-site
catalogues and BAP tables was measured on it. No native gameplay is claimed.

## The build we have

| | |
| --- | --- |
| Executable label | `56578.17.04.12.1251.evil` (April 2017) |
| SELF (launched) | 29,276,248 bytes, `ebee5df27aca00057f223b96edab461917e1a7e837b4017b7d6d84b0ae7a2a02` |
| ELF (static tools) | 29,249,016 bytes, `672f03411c0503dfbcda804cbb9fb27cdd9a8d73a8bcb71bf0c4f688073b0833` — the new pin |
| Package metadata | `param.sfo` VERSION **01.33** (APP_VER 01.00 is the repack's own), every `changeinfo` `app_ver="01.33"` |

The package metadata says 01.33, the end-of-life patch. That agrees with
aknumbers' 11 October video, which runs an Xbox One build "56578" against a server
set to "PS4 1.33". The executable has not yet reported its own version, so the
registry keeps `app_version` empty and records `package_version` separately.

aknumbers' GitLab still has only the 11 August snapshot and no tags. On 9 October
he wrote that the 1.33 build can be used once a public release ships. Nothing was
downloaded.

## Full extraction

`Export-D1RepackFull.ps1` extracted the repack's whole game directory (`/uroot`)
through the corrected LGPL reader, with the original opened read-only:
**2,119 files, 52,738,749,800 bytes, 2,105 Bungie packages**, finished
21:49:37 UTC. Every file went `.partial` → length check → SHA-256 → rename →
ledger; `SHA256SUMS` and `manifest.json` were written only at the end. The
`eboot.bin` matches the pinned SELF. The earlier inode review counted 2,120
regular files; the extra one is `/flat_path_table`, outside the game directory.
The brief mentions 2,158 files; that count was not reproduced here.

`param.sfo`, icons, `changeinfo` and the trophy file live in the package's entry
table, outside the PFS image. `Export-D1RepackSceSys.ps1` copied the 17
unencrypted entries into `sce_sys/` (`SHA256SUMS.sce_sys`); encrypted entries were
not read. Its `param.sfo` is byte-identical to the independent extractor's copy.
The partial Codex extraction and the 01.29 junction were left untouched.

## English manifest

The existing default policy (all generations, neutral plus English) selects
**1,392 packages, 71,003 bytes**,
`50f2b13866773d7143f65218c3205179147406ac3d32d50c536028d9de515bb0`. That is
above the **1,325-descriptor cap measured on the 01.29 client**; the 56578 cap is
not yet measured. The newest-generation policy selects 408 packages. With
`D1_BUILD=01.29` the same command still reproduces the pinned 1,033-package,
52,797-byte reference manifest exactly.

Full content validation (`Test-D1BuildContent.ps1`, 13 October 07:09–07:27 UTC):
all 2,119 extracted files re-hash to their recorded SHA-256 values, and
`d1-pkg verify --blocks` passes **2,105 packages and 964,151 stored-block SHA-1
hashes with 0 failures**. The regenerated manifest is byte-identical to the
recorded pin. An earlier attempt was stopped to give a boot a quiet machine, and
another was interrupted overnight during block verification. Both are kept as
incomplete evidence and neither is counted.

## Build parameter

`capture-rig/builds.json` in the PS4 tree, read by a new `d1-build` crate and
`capture-rig/d1build.py`. `D1_BUILD` selects a build; the default is
`later-56578`. Changes are in `tools/ps4_setup/build-registry.patch`, the same
edit as the public `ps4/` tree:

- `d1-eboot`, `d1-bap`, `d1-oracle`, `d1` (crash symbolizer), `d1-pkg`,
  `d1-harness`: default image, packages, manifest and launch paths come from the
  selected build. The 01.29 constants remain as the reference.
- `run-boot.sh`, `d1-platform`, readiness, artifact preparation: executable,
  manifest and patch gates read the registry. A build without its own client
  patch gets no patch; the 01.29 lab patch is never applied to another build.
- Address-bearing data keeps its derivation hash. Probe-site catalogues,
  `launch_knowledge.json`, the surface registry and the diagnostic catalogue
  stay pinned to 01.29 and fail their existing build gate on 56578. The
  diagnostic catalogue writes non-reference output to a per-build folder.
- The Python Oodle harness loads the 01.29 ELF by fixed path and calls fixed
  01.29 decoder addresses, so it fails closed until re-derived.

Tests: `d1-build` 3/3 (Windows and Linux), `d1-harness` 13/13 (after expanding
the registry variable in its run-boot comparison), `d1-platform` 2/2 on Linux.
The canonical patch installer recognises the new `run-boot.sh` state.

## First static look at 56578

`d1-eboot controls` on the new ELF: **21 PASS / 40 FAIL, exit 2**, as intended
for a build without re-derived addresses. Everything address-independent passes:
build registered, 2,633 RTTI types all named, 115 investment BAP message classes,
NID oracle, FNV-1 name hashes, family 4/4 instance checksums, and the
**character, roster and 503-response schemas**. The **account and profile schemas
fail**, so serialisation for those two may have changed. The world-controller
ladder is re-found from RTTI (35/35 handlers), with moved addresses, e.g. rung 28
update `0x7A6D60` → `0x7a8710`. `d1-bap controls` refuses at the 01.29 identity
gate (8 failures), as designed. Receipts:
`cfw-staging/2026-10-12/retarget-static`.

## First contained boot

Containment was checked first: all three firewall profiles on; the enabled
outbound block for the exact `shadPS4.exe`; no emulator running. The 50 original
redirects resolve to loopback. The new executable adds four service literals
(`auth.api.sonyentertainmentnetwork.com`, `s.ac.playstation.net`,
`ps4.scedev.net`, `graph.facebook.com`). They are added to `d1-hosts.txt` but
**not yet applied**: re-run `contain-d1.cmd` as administrator. The boots ran
with `-AllowUnredirected`, recorded in each receipt; the firewall still blocks
any non-loopback connection.

Servers ran in WSL, bound to 127.0.0.1 only (`wsl-servers-loopback.sh`).
A mechanics check confirmed Windows reaches 80, 443, 31313 and 31315 on
loopback, the LAN address is refused, the WebAPI serves the manifest bytes, and
both servers stop on request. shadPS4 0.19.0 started with `-i` and no client patch.

Result, two runs (the second on a quiet machine):

- Content gate passes: `CUSA00219 · Destiny · App Version 01.00` (repack
  APP_VER). shadPS4's own NP login to the local shadNet succeeds
  (`D1Guardian`, accountId 1).
- The game's code runs: modules load (eboot at `0x51c0000`), job threads start,
  graphics initialise on the RTX 2060 SUPER, the splash screen is hidden, the
  first shaders compile, and the game writes `reserved_crash_file`/`test.dat`.
- Without the guard, about 30–50 s in, the game's **Graphics Heartbeat** thread faults:
  `0xc0000005` at executable address **`0xFAAF4`**, deterministic across both
  runs. No title screen, no `/SignOn` and no request to the responder. No guest
  crash report was written.

Cause, from the new ELF. The heartbeat thread (entry `0x7E6420`, created through
`0xF7C90`) loops in `0x7DD060` until a frame counter (`[0x26E4840+0x20]`) reaches
its target. While behind, it calls `0xFB860` → `0xFAAC0` to run one pending job
itself, and sleeps 300 µs (import call, `edi=0x12C`) when there is none.
`0xFAAC0` reads the thread's job-worker pointer from TLS `+0x58`, which only
worker threads set (the `0xFAD30` worker loop). The TLS segment has no
initialised data, so a non-worker's pointer is null and `mov rsi,[rax+8]` at
`0xFAAF4` faults. The PS4 project recorded the same address on 01.29 under
Linux as a load-sensitive race; here it is deterministic. Enabling shadPS4's
pipeline cache did not help (the fault came sooner, about 10 s in), so the
setting was restored exactly.

**Synthetic guard.** `make-job-helper-guard.py` writes
`patches/CUSA00219-56578-job-helper-guard.xml` (synthetic client modification;
everything reached through it is `synthetic_ancestor`). It verifies the ELF hash
and the original bytes. A 13-byte hook at `0xFAAE7` jumps to unreferenced
padding at `0x290992` (46 bytes between a `ret` and the next prologue; no xrefs
or call-graph targets). There it calls the TLS getter. If the worker is non-null,
it runs the original instructions and returns to `0xFAAF4`. If null, it sets
`ebx=2` and jumps to the function's own exit at `0xFACF6`, which returns "no job
run" through the normal canary check and register restores. The heartbeat then
takes its existing 300 µs sleep path. A patched scratch copy disassembles as
intended. shadPS4 0.19 silently ignores a patch entry without
`AppElf="eboot.bin"`, and `-i` also drops an explicit `-p`, so with a client
patch the orchestrator omits `-i` and instead requires the automatic patch
folder to be empty. Three unapplied attempts (`guard`, `-b`, `-c`) are retained.

**Guarded run `20261013-073442-later-56578-guard-d`** (both patch lines logged as
applied): no fault for the full 8 minutes; the orchestrator stopped it.

- The game prints its own build, `build:string: '56578.17.04.12.1251.evil'`,
  and a networking session id.
- Window frames show the DESTINY logo animation, then **Connecting** after the
  Cross taps, then the game's own **"Destiny servers are not available"**
  dialog (about 105 s in).
- The responder received the game's first requests: two
  `GET /v1/users/D1Guardian/friendList` on 31315 from the game's `libhttp`,
  both answered 200.
- **Stop point:** the new build signs in to PSN at
  `POST https://auth.api.sonyentertainmentnetwork.com/2.0/oauth/token`, not the
  01.29 `auth.api.np.ac.playstation.net`. That name is one of the four not yet
  redirected, so it resolved upstream and the connection then failed (the
  firewall block held). No `/SignOn` was sent. The responder already serves
  `/2.0/oauth/token` regardless of host, so applying the redirect is the next
  step.

**After the redirects (13 October, run `20261013-074718-later-56578-guard-redirected`).**
The user applied `contain-d1.cmd`, so all 54 names resolve to loopback. Readiness
now fails only the permanent native-runtime flag. The boot ran with the guard
and without `-AllowUnredirected`; no fault occurred in 8 minutes. Responder log:

- `POST auth.api.sonyentertainmentnetwork.com/2.0/oauth/token` → 200 with
  `id_token`.
- **`POST www4.signon.deadorbit.net/SignOn`** (795 B, client build
  `56578.17.04.12.1251.evil`) → 200 placeholder success (BAP `127.0.0.1:3075`,
  manifest URL `http://cdn.deadorbit.net/manifest.bin`).
- `GET cdn.deadorbit.net/manifest.bin` → our 71,003-byte unsigned manifest.
- One full retry of SignOn and the manifest fetch, then the game's own dialog:
  **"Failed to download configuration files from Bungie servers", error code
  `turtle`**. No guest crash report.

So the server stack still answers the new build through OAuth and `/SignOn`. The
next gate is the CDN manifest's RSA signature envelope, as predicted. It cannot
be forged.

**Lab equivalent re-derived, not yet run.** The one RSA-2048 DER key is at
`0x15A0440` (01.29: `0x1591800`). The `cdn_content_manifest` handler is
`0x2B0980` (01.29: `0x2B0030`). Its only flag read is `0x2B09F0`:
`cmp byte [0x18DC974],0; je 0x2B0AF2`, which otherwise requires a body over
`0x107` bytes and verifies a signature over `body+0x108`, matching the 01.29
description. The flag ships as `0x01`, at 01.29's `0x18C4974` plus the
second-segment shift of `0x18000`.
`make-job-helper-guard.py --unsigned-manifest` writes
`patches/CUSA00219-56578-lab-manifest.xml` (guard plus that byte set to `00`,
after verifying the shipped value). The run using it was blocked by the session's
permission policy as a security-check bypass, so it awaits the user's decision.
The 1,392-entry manifest also exceeds the 01.29-measured descriptor cap, which
may be the next gate after that.

**Lab manifest run (user-approved, run `20261013-080220-later-56578-lab-manifest`).**
All three lines of `CUSA00219-56578-lab-manifest.xml` were logged as applied. No
fault occurred in 8 minutes. OAuth, `/SignOn` (`www8.signon.gravityshavings.net`)
and the 71,003-byte manifest were served twice as before, but the game's error
moved from `turtle` to **`marionberry`** ("Could not connect to the Destiny
servers"). So the unsigned 1,392-entry manifest was accepted past the
envelope, at least as far as this gate; the descriptor cap did not stop it here.
`marionberry` is `error.transport_method_invalid`. The PS4 project traced it on
01.29 to a loopback-only transport: the client rejects `127.0.0.1` STUN/BAP
addresses and wants a routable-looking local interface. It solved this on Linux
with an isolated dummy interface at `10.77.0.2`. Here every redirect and listener
is `127.0.0.1`, and shadPS4 logs `sceNetCtlGetInfo: local ip: failed to retrieve`
four times. This is the open Windows guest-transport gate. Everything past the
lab byte is `synthetic_ancestor`.

**Lab network (13 October).** The user ran `d1-lab-net.cmd`. "D1 Lab" (KM-TEST
loopback adapter) now has `10.77.0.2/24` with no gateway, the redirects point
there, and the shadPS4 block covers everything except `10.77.0.0/24`.

- With WSL servers (`20261013-083045-later-56578-labnet`), the game's OAuth timed
  out. Under mirrored networking Windows owns `10.77.0.2` itself, so Windows
  clients never reach a WSL listener on it.
- The orchestrator's `-LabIp` mode therefore runs the same `d1-responder.exe`
  and `shadnet-server.exe` natively on Windows: guest-facing responder on the
  lab address, a loopback responder for the emulator's WebAPI, shadNet on
  `127.0.0.1:31313`. Every listener is checked to be loopback or lab, and only
  its own processes are stopped (verified).
- Run `20261013-084057-later-56578-labnet-win`: OAuth, `/SignOn` and the
  manifest were served at `10.77.0.2`, the game resolved all STUN names there,
  but it still ended in `marionberry`. No BAP connection was made.

**Root cause: shadPS4's local-IP probe.** `sceNetCtlGetInfo(IP_ADDRESS)` calls
`NetUtilInternal::RetrieveIp()`, which `connect()`s a **TCP** socket to
`1.1.1.1:80` and reads `getsockname()`. Our firewall block correctly prevents
that real off-box connection, so it fails and shadPS4 reports the placeholder
`127.0.0.1` with success. The game's Demonware layer (`0x2E9FC0` on 01.29) uses
that as its own address and rejects the transport. The PS4 project fixed this
in its private fork (`shadPS4-d1trace`; `status/shadps4.md`, "local-IP fix",
with `capture-rig/retrieve-ip-control.py`): the probe becomes `SOCK_DGRAM`
(route lookup only, no packet) with a `getifaddrs()` fallback, and total
failure returns `NOT_AVAIL` instead of a fabricated loopback. Current upstream
still uses the TCP probe. Allowing the TCP probe would make a real internet
handshake and is not acceptable. The next step needs a shadPS4 build carrying
that fix; see the questions in the report.

**Patched shadPS4 (user-approved downloads, 13 October).** The shadPS4 source
at tag `v.0.19.0` (`c7e065d1`, the commit embedded in the binary in use) was
cloned with 45 submodules to `F:\d1	ools\shadPS4-src`. The official LLVM
23.1.3 Windows archive was downloaded; its SHA-256 `9ee6a219…` matches the
release's GitHub build attestation. It is unpacked to
`F:\d1	ools\llvm-23.1.3`. `Build-D1ShadPS4.cmd` builds Release with
`clang-cl`, Discord RPC and the updater off, and `HAVE_ENDIAN_H=OFF` (LLVM 23's
`endian.h` lacks the Windows byte-order helpers, which broke the vendored
LibreSSL). Result: 2,449 steps, `shadPS4.exe` `1297c91c…`. It was swapped in at
the firewall-blocked path. The stock binary (`91eb69a5…`) is kept as
`F:\d1	ools\shadPS4\stock-0.19.0-shadPS4.exe.bak`, which is deliberately not
launchable because the block is path-based; `SWAP-RECEIPT.txt` records both
hashes and how to restore.

Run `20261013-091447-later-56578-labnet-guestip` (guard + lab manifest, lab
network, Windows servers, `D1_GUEST_IP=10.77.0.2`): shadPS4 now reports
`local ip: 10.77.0.2` four times, with no IP warnings. No fault occurred in 8
minutes. The game resolves all 16 STUN names three times and opens 13 UDP
sockets. OAuth → `/SignOn` (the reply advertises BAP `10.77.0.2:3075`, client
IP `10.77.0.2`) → manifest → one retry, but still **`marionberry`** and still no
BAP connection. The local address was necessary but not sufficient.

Leading hypothesis (not yet tested): the 56578 client parses the transport
fields of our 01.29-shaped SignOn reply differently. The static check already
showed account/profile schema changes, and the client re-signs-on after the
manifest instead of dialling BAP. `error.transport_method_invalid` is at
`0x1654414`; its only reference is the error-table function `0x7855C0`, not the
decision site. Next: locate the 56578 SignOn response parser and compare its
field layout with `d1-responder`'s reply (f2.6/f2.7 BAP, f14 client IP, the
STUN/transport method), using RTTI, strings and the 01.29 design notes.

**From marionberry to investment sign-in (13 October, later).**

1. *SignOn schema is unchanged.* `dump-codec-descriptor.py` reads the codec
   records and resolves their pointers from `DT_SCE_RELA`. 56578's response
   descriptor `0x1910CC0` (parser `process_response` `0x2AA420`, same codec
   `0xA2020`) has exactly the 01.29 fields, including f2.6/f2.7 BAP and f14.
2. *Guest diagnostics re-derived (output only).* `retail_log_emit` is `0xE4AA0`
   (same 1,124 x `0x150` ring at `+0x19860`, `0x140` copy), the check is
   `0xE4D70` (1,636 sites), `printf` is `0x12AAB90` and `"%s
"` is `0x16136A0`.
   `make-job-helper-guard.py --diagnostics` adds the sink and T=1, verified by
   disassembly of a patched scratch copy, in `CUSA00219-56578-lab-diag.xml`.
   The game's own log then shows STUN falling back to `10.77.0.2`,
   `platform_signin`, `account_signin` (SignOn success; the server reports
   content build `56578.17.04.12.1251.evil`), `content_check` passed (manifest
   accepted), and then `bootflow:package_registration`: *Prerequisite
   'ENUM(15)' failed for reason 'error.transport_method_invalid'*, preceded by
   *STUN address discovery failed and not configured to use external address
   from signon.*
3. *The setting.* `0x2E0560` reads engine-config bools `+0x598` (on STUN
   failure use the SignOn f14 address) and `+0x599` (prefer SignOn over STUN),
   via `0x34E3A0` = block `+0x534` (block pointer `0x2693DF8`). The 56578
   config-delta setter switch is `0x1033F80` (decoder `0x1DB190`, magic
   `0x123456`; key 894 still maps to `+0x9EE`). `+0x598` and `+0x599` are
   **new sparse keys `0xF1048` and `0x4409EC`**, absent in 01.29, where an
   unknown key aborts the SignOn parse. `d1-responder` now appends key
   `0xF1048 = 1` to the SignOn f4.3 delta **only when the request's build is
   `56578.*`** (`d1-responder-56578-signon-fallback.patch`). The test covers
   both directions; 13/13 tests and all responder controls pass.
4. *Result* (run `20261013-094238-later-56578-signon-extaddr`):
   `package_registration` → **`bootflow:bap_signin`** → **`investment_signin`**.
   The BAP channel opened (start 0x1E/0x1F, secure hello 0x19/0x1A), then the
   client sent queuez register 0x79. The emulator then aborted in shadPS4's
   shader recompiler: `V_CMP_CLASS_F32: Unsupported float class mask: 0x2`
   (QuietNan alone). The D1 shadPS4 build now maps a single-kind NaN class
   test to `IsNan` (SPIR-V cannot distinguish them; GPU arithmetic yields only
   quiet NaNs). Rebuilt binary `03909598…` swapped in at the blocked path
   (`SWAP-RECEIPT.txt`); `shadps4-0.19.0-local-ip.patch` now covers all three
   files.
5. *Current stop* (run `20261013-094527-later-56578-signon-extaddr-b`, no
   emulator fault): investment sign-in exchanges 503 (account) and 206
   family 3 (roster `0xA2AC2C74` + character `0x3431F1BA`, both still valid in
   56578), plus queuez pushes for families 4 and 3. The client then fails with
   **`error.investment_server_object_instance_type_from_checksum_not_found`**.
   From the 56578 descriptor table
   (`retarget-static/descriptor-checksums-later.json`, 4,566 descriptors):

   | object (tag) | 01.29 checksum / size | 56578 checksum / size |
   | --- | --- | --- |
   | account `0x80801803` | `0x64AE75BF` / `0x7338` | `0x6B30D2B4` / `0x75C8` |
   | profile `0x80801743` | `0x8127F15A` / `0x7078` | `0x003BC6E5` / `0x7378` |
   | roster, character, family-4/4 wrapper | unchanged | unchanged |

   Both objects grew (+0x290 / +0x300 bytes), so the server must serialise them
   with the 56578 schemas. Changing only the checksum is not enough. This is the
   investment-schema re-derivation (plan step 5).

**Investment sign-in passed (13 October, ~10:07 UTC).** The minimal account
and profile encodings are unchanged in 56578: the inner account `0x808018C7`
still has 25 top-level fields with the same leading four, and the inner profile
`0x80801714` still has 18, ID first. So only the instance checksums need to
follow the client build. `d1-bap` gains `ACCOUNT_INSTANCE_TYPE_56578`
`0x6B30D2B4`, `PROFILE_INSTANCE_TYPE_56578` `0x003BC6E5`, `set_client_build()`
and accessors. `d1-responder` records the build from the SignOn `build=` query,
and the builders stamp checksums through the accessors (01.29 defaults
unchanged; d1-bap 161 tests, responder 13 tests and all controls pass). The
01.29 profile fixture is withheld from other builds, which get the sparse
profile (ID + orbit destination). Patch: `d1-bap-56578-instance-types.patch`.

Run `20261013-100634-later-56578-acct-profile-b`: OAuth → SignOn → content
check → package registration → BAP sign-in → **investment sign-in → world
`ambient_bootstrap` → `bootflow:profile_setup`**. The 503, 206 family-3
(roster + character), subscription and echo exchanges were all answered, and
family 4 (account `0x6B30D2B4` + profile `0x003BC6E5`) and family 3 were pushed.
About two minutes later the game halted itself (`RELEASE_VHALT`, *hitch
detected: mainloop … bootflow:profile_setup … job name: 'network_send' …
stalled*) and uploaded its crash report to our `ticket_drop`.

The game's own crash report gives its version as `evil tiger_final release
orbis 56578.17.04.12.1251.evil`, `insecure_ip 10.77.0.2`, BAP
`10.77.0.2:3075`, silo `28729`.

**Caveat for those halts:** the window frames show the Windows lock screen.
The session locked at about 10:05 UTC (the orchestrator's screenshot call
failed then, and it is now soft-failing). A locked session stops the emulator
presenting frames, which stalls the main loop that the game's watchdog
measures. The pipeline cache, enabled in the emulator config (recorded in
`SWAP-RECEIPT.txt`) to rule out shader-compile stalls, warmed with 223 entries
and did not change the result. The halt must be re-tested with the session
unlocked before it is treated as a protocol or emulator problem. One
interrupted run (`…-100517-…`) left the emulator and servers running; they were
stopped after its logs were saved.

`profile_setup` is the operator menu step on 01.29 (move down, confirm,
dismiss the brightness prompt). It needs the user's controller (`-OperatorInput`).

**Static re-derivation while the PC was locked (13 October).**

*Oodle (content decoding).* `DecodeSome` is the only function referencing the
assertion name `"OodleLZDecoder_DecodeSome"` (`0x16C3BB1`): **`0x126CA00`**
(01.29 `0x12672E0`). The scratch query `0x126C840` and create `0x126C8C0` sit
at the same offsets before it, and the frame `memset` returns to `0x12761A5`
(the library moved `+0x5720`). The harness's other hooks are confirmed by NID
(computed `SHA-1(name + suffix)`): memcpy `0x12AA7C0` (`Q3VBxCXhUHs`), memmove
`0x12AAAB0` (`+P6FRGH4LfA`), memset `0x12AA980` (`8zTFvBIAIN8`),
`__stack_chk_fail` `0x12AA7F0` (`Ou3iL1abvng`). The 01.29 harness had
memcpy/memmove labels swapped, harmlessly, since both are copy stubs. The stack
guard slot is `0x1A1F100`, and Oodle's plugin-routed aligned alloc/free are
`0x1278720`/`0x12787B0`. The 01.29 `IMP_NOP` (`0xF8ED00`) has no 56578
equivalent identified yet, and the decode did not need one. `d1-oracle` picks
these per image hash and refuses unknown images. **The client's own decoder
extracted tag `0x80A5E37A` for 56578:** `ps4_investment_globals_012f_6.pkg`
entry 890, 6,528 bytes, self-length header match, SHA-256
`84ab907ea3e794eb12023303dc3a3b84866cdc85cb7a2bb89675f95b073aee27`
(private, `retarget-static/`).

*BAP tables.* `POLICY_PTRS` is the only run of 33 relocated slots pointing at the
33 objects whose vptr is a `c_investment_bap_message_policy_*` vtable (RTTI):
`0x193BD50`. The registry `0x193A1B0`, names `0x193BB40`, codecs `0x193B540`
and push table `0x193DB30` follow at the same `+0x180A0` shift and match their
structure. With the 56578 definitions file, **`d1-bap controls`: 18/18 PASS**,
including drift checks (derived == server constants) for the 58 message
classes, 33 policies, 13-entry push set and 8 header lengths, and messages
206/503 keep their schema tags. The server's BAP tables are therefore verified
for 56578, not merely tolerated.

*Schema survey.* Of the 19 schema tags whose 01.29 checksum is documented in
`design/`, 16 are unchanged in 56578. The changed three are account and profile
(handled) and `0x80801941` (`0xB8C0245A` → `0x2859B3E2`, the world-server root
embedded in 503/205). Its 56578 layout is unchanged where it matters: 944 bytes,
8 fields, unlock flags at field [3] as a 6-bit count plus 50 × {u16, u2}, so the
503 encoding is unaffected.

*Fix found on the way.* `Image::open_default` still opened the 01.29 path; that
edit had failed in the first registry change. It now follows the registry, and
`Image::open_reference` / `Harness::open_reference` / `Content::open_reference`
serve tests and controls that assert 01.29 facts. Those skip unless
`D1_BUILD=01.29` and the 01.29 estate are present; `d1-oracle`'s Oodle control
command always uses the reference. `d1-oracle` unit tests that need the absent
01.29 executable fail as they did before this change (public CI does not run
them). Patch: `d1-56578-oodle-bap-anchors.patch`. Windows: d1-build 3, d1-eboot
all suites, d1-bap 161 tests, responder controls pass. Linux (public tree):
d1-build, d1-eboot, d1-bap, d1-responder and d1-harness suites pass.

**Codec, Activity-Host and profile re-derivation (13 October, still static).**

The 01.29 ELF is not on disk, so each 56578 address was found from the 56578 image plus the
01.29 notes' structural descriptions. None was found by offsetting a number.

*Bitstream primitives* are at the same addresses as 01.29 (ctor `0x72240`, begin-write/-read
`0x723B0`/`0x72400`, flush `0x72560`, end `0x72650`, spill `0x733A0`, read-bit `0x73430`), and each
body matches its documented behaviour. Write-one-bit is the gatekeeper vtable slot 5 (RTTI) at
`0xAF1650`, byte-identical to the 01.29 listing.

*Object codec* `0x2C9A80`/`0x2C9A90`, replicated `0x2C9AE0`: the module moved `+0xA90`. The three
documented bitset memset sites moved by exactly that. The allowance window is
`0x2C6A90..0x2C9E20`, deliberately not a blind `+0xA90`, because `0x2C9E20` memsets an unrelated
stack buffer.

*Message codec* `0x261E90`/`0x262260`, manager `0x25CBD0`, id→index `0x2800E0`. *Registry global*
`0x1A238A0`: the decoder has three rip-references to it (01.29 had three), and the walk is the
oracle's construction contract instruction for instruction.

*Activity Host:* decode/encode wrappers `0x2A4AC0`/`0x2A4B70` (capacity still `0x1DF00`),
`activity_client_send_message` `0x299D10`. A register tracker read all 30 call-site rows: udt,
descriptor global and size at the 28 send sites, 12 decode sites and 2 replicated sites. Every
global moved `+0x18090` and every udt keeps its role. Every stated size is unchanged except
**udt 37 `send_client_heartbeat`, `0xB1E0` → `0xB750`**. The RX dispatch table is at `0x1910480`
with the same 17 types, and its relocated handlers confirm the udt 0/1/4/11 rows independently.

*Results on 56578 (Windows, the owned image):*
* `d1-oracle control`: object and message PASS.
* `vectors`: all OK, including account and profile.
* `activity-host controls`: **34/34**. This includes the client's own decoder reproducing the
  captured live join_request's logged fields, a byte-identical re-encode, and a 7,297-byte
  replicate_membership through the replicated wrapper.
* sensor 11/11, roster 18/18, character 20/20, inventory 9/9, profile 12/12.
* `d1-activity-host controls`: **112/112**.
* item-index 4/5. The 5th needs `catalogue/d1-catalogue.json`, which is not on this machine.

`capture-rig/item-content-index.tsv` was generated from the 56578 packages with the client's own
decoder. It has 7,544 rows, the entry SHA-256 is `71bfdc0a…`, and the file is git-ignored and local.

*The profile changed in 56578.* The struct is `0x7378` = 29,560 B, up from 28,792. Body field
[10] `0x80801701` grew `0x100`. Field [11], the dense completion flags, went from `0x80804C21`
(2,048 elements) to **`0x8080165B` (2,560 elements)**. That is 960 B on the wire instead of 768,
and `0x80804C21` no longer exists in 56578. Everything from field [12] onward moved `+0x300`:
the stat block is now at `+0x6FD0`, the overrides at `+0x6FE0`, and the all-content override at
`+0x714C`. Fields [0]–[9] are unchanged. The negatives caught it: the old `+0x6CE0` positive
passed by accident because it now lands in the dense u32 array, and the paired negative failed.
The server's own sends are unaffected:
* The minimal profile (SOID plus 17 absent fields) still decodes through 56578's codec.
* The instance checksums are right: account and profile were already build-aware, and roster
  `0xA2AC2C74`, character `0x3431F1BA` and unlocks `0xC4060975` are unchanged in 56578.
* Grant tooling that writes completion flags must use 56578's 2,560-element array.

*Harness hardening:* stubbed memcpy/memset/alloc now refuse lengths over 256 MB as a guest fault.
On 56578, one Activity-Host negative control drives the encoder into a bitset memset of length
`-0xA4`, returning to `0x2C8F89`, a measured codec site. Before this change the whole process
aborted (`capacity overflow`). Now it is the refusal that control requires.

*Linux, public tree:* the d1-oracle and d1-activity-host failure sets are identical to the public
HEAD's: 13 and 9, every one needing game images. The two new 56578 tests pass by skipping. One
regression was caught and fixed first: `ProfileSpec::to_object_for` lets the image-free unit tests
pin 01.29 offsets.

Public commit `dba749c` (local, not pushed); patch `d1-56578-codec-ah-profile.patch`.

*Environment note:* `d1-env.ps1` sets `LIBCLANG_PATH=F:\d1\tools\LLVM\bin`, which no longer holds
`libclang.dll`, so unicorn's bindgen fails. Building d1-oracle needs
`LIBCLANG_PATH=F:\d1\tools\llvm-23.1.3\bin`.

*Still open:* the dynamic-flags fault RIP for 56578 (unobserved; the refusal text names the grow
`0x2C7630` only), field [10]'s internal change (no 01.29 sub-layout on record), and the 56578
`IMP_NOP` equivalent.

**Build 56578 reaches orbit (13 October, lab runs, user away; runs authorised).**

Run `20261013-130110-later-56578-lab-orbit-c` went title → … → `investment_signin` →
`prepare_for_orbit` → `character:signin` → `character:prologue` → `cleanup` → `setup:orbit_loading`
→ **`setup:orbit`**. It stayed in orbit, with no guest halt and no host fault, until the 14-minute
limit. A frame shows the orbit screen with the D1Guardian nameplate, Set Destination and a "Crucible
Unlocked" prompt. Everything is reached through synthetic client patches
(`patches/CUSA00219-56578-lab-orbit.xml`), so it is `synthetic_ancestor`.

*The "profile_setup stall" was not the wizard.* Three earlier runs halted with
`hitch detected … job name: 'network_send' … stalled`. The first one was put down to a locked
session. Wizard taps did not help: the posted keys never reached the unfocused window, and focused
taps lost focus. With the wizard skipped (c1: `0x57F5E0` → `mov al,1; ret`; it is the 56578 form of
`0x57E490`, called only at `0x79DAE0`), the same hitch moved to `prepare_for_orbit`. Lib.Net debug
logging showed the client spinning on `sys_sendmsg(BAP socket)`. **shadPS4's Windows
`PosixSocket::SendMessage` cast `OrbisNetMsghdr` to `WSAMSG`, whose `WSABUF` is `{len, buf}` where
an iovec is `{base, len}`, and called `WSASendMsg`, which Windows does not support on TCP.** A
multi-buffer stream send returned 0 bytes forever. Under the fix, stream sendmsg/recvmsg
gather/scatter through one `send`/`recv`, and the client's `0x0015` purchased-offers request
arrived and the conversation continued. That fix is in `shadps4-0.19.0-local-ip.patch`, in the
binary with sha `e96f7308…` (receipt in `F:\d1\tools\shadPS4\SWAP-RECEIPT.txt`).

*Voice.* shadPS4 stubs libSceVoice. In `cleanup` the client faulted three times in its voice-port
layer:
* a NULL port object in the SetMuteFlag thunk `0xB47E50`;
* the same in the GetPortInfo thunk `0xB47CA8`;
* `#DE` at `0xB493D9`, a divide by a GetPortInfo field the stub never fills.

The root is `0xB483EC` `mov byte [r14],1`. It marks voice "started" unconditionally after
Init/GetResourceInfo/Start, and every voice entry point opens with `cmp byte [rdi],0; je`. The lab
patch writes 0 there, the console-without-voice path, and keeps `xor eax,eax; ret` guards on the
six dereferencing thunks as a backstop.

*Next gate.* In orbit the client sends `0x000C` subscription requests about every 10 s, roughly
100 in the run. Each is answered with `0x000D`, yet it logs
`Failed peer-subscription validation [error error_timeout][account_soid 0xD100000000000001]` 86
times. It is waiting for something the server does not send after subscribe.

*Peer-subscription gate closed (same day).* Classes 0 and 2 subscribe and, with only bare `0x000D`
ACKs, time out as they did on 01.29: the client marks them LOST, a permanent state that refuses
later updates. The responder's existing `D1_ANSWER_FAMILIES=0,2` sends each one empty baseline
family answer after its ACK, the answer-once shape proven on 01.29. In run
`20261013-132144-later-56578-answer-fam02` both classes then subscribed exactly once and there were
**0** `Failed peer-subscription validation` lines (86 without it). The client stayed in
`setup:orbit` for the rest of the 12-minute run with no faults and only routine log lines. The
orchestrator passes such knobs as `-ExtraServerEnv 'D1_ANSWER_FAMILIES=0,2'`, and they are
recorded in `server-environment.txt`.

*Driving orbit by hand (same day; the user was away and asked me to do the controls).* For a
session with `-OperatorInput`, two helpers in `tools/ps4_setup` stand in for the pad:
`d1-pad.ps1` focuses the emulator and sends real key input (cross `NumPad2`, circle `NumPad6`, left
stick `W/A/S/D` with hold times), and `d1-shot.ps1` captures the window. The first attempt stalled
behind a Windows Firewall prompt for the new `d1-activity-host.exe` listener. I left that decision to
the user, who allowed it. From the title I reached orbit and dismissed its notices (Crucible
Unlocked, Armsday, Age of Triumph, a Destiny 2 advert dismissed with circle, not cross, so as not
to press Pre-order). I then opened the Director through Set Destination. **The Director is empty:**
Earth has no destination nodes, and the Crucible emblem shows no tooltip and does not respond.

*Unlock flags are build-specific.* The 01.29 ten-flag set is ordinals in the client's master flag
table. That table is content: tag `0x80A5FFCD`, class `0x8080179C`, element class `0x808018B8`, rows
`{u32 hash, u16 kind, u16 sub}`. It has 6,944 rows in 01.29's `012f_3` and **7,731 in 56578**, the
same count as the 2020 manifest's hash universe. Read with the client's own decoder and joined by
hash: 98, 308, 419, 440 and 834 are unchanged; 2667→2668, 3389→3391, 5015→5021, 5987→5998 and
6018→6029. Controls: 01.29[98] = `3638915056` and 01.29 ord(`1681748763`) = 2538, both documented;
56578 puts the latter at 2539, the 2020 manifest value. The responder's built-in default would have
granted the wrong five.

The 56578 503/205 pair was encoded by the 56578 client's codec after the pinned-body control. The
shared tail agrees at `503[23:]==205[3:]` with ids, `[21:]/[1:]` without. It is now the default when
SignOn reports 56578 (public `704c8d3`, patch `d1-56578-unlock-ordinals.patch`). It was also served
by override in run `…-operator-grants`: delivered (503 once, 205 by poll), and the responder
confirmed agreement. **The Director stayed empty.** The ten flags are the rung-33 DLC/permission
gates, not Director content.

*How 01.29 left orbit.* Not through the Director: `design/GUARDIAN-IN-WORLD.md` §3 used a live
memory poke (`session-params.py --launch-poke --activity-id 407`), then a cinematic-suppress byte, a
done-mask force, a join counter, two code hooks and a spawn poke. The biped existed but could not be
controlled. Reproducing that on 56578 means re-deriving those addresses and having a live poke
transport for this emulator, which is a separate piece of work.

*Build 56578 reaches `activity:in_world` on `pvp_moonbase` (13 October, run
`20261013-…-later-56578-launch-poke-b`).* The route mirrors 01.29's, rung for rung, re-derived:

1. **Leave orbit: synthetic launch poke** through `tools/ps4_setup/d1-winguest.py` (Windows
   ReadProcessMemory/WriteProcessMemory on the shadPS4 process; guest pointers are host addresses,
   and eboot-relative ones take the logged load base). The session accessor `0x775770` is
   instruction-identical to 01.29 `0x773FC0`; its manager global is `0x26CA4E0`. Writes: desired
   activity `params+0x9B8+0xEA` = 407, goal `params+0xE4` = 0x24, orbit handler (vtable
   `0x19474B0`, the single hit in the image's data range) `+0x38` debounce = 0. Every value read
   first was the predicted orbit state.
2. **Rungs 28–33 on served data:** session creation, matchmaking, `activity_host_setup` (our
   Activity Host answered the join), prologue, outro, then `activity_world_transition`:
   **"successfully changed world to: pvp_moonbase" / "World-transition finished".** The rung-33
   DLC/permission gates passed on the re-indexed 56578 unlock flags (`704c8d3`), with no poke.
3. **Emulator fix:** loading `pvp_moonbase` hit shadPS4's texture-cache `UNREACHABLE` ("unresolvable
   image overlap with equal memory address": same format, tile mode and resources, different image
   type, 16× the size). Fork-local change: treat a TYPE change at an equal address like the
   existing tile-mode reuse (free the stale image when safe, use the merged id). Binary sha
   `8cfc65fa…`, receipt in `SWAP-RECEIPT.txt`, patch regenerated.
4. **Rung 34** (`0x7969A0`: request 0x23 when done `+0x1A8` == requested `+0x198`): masks
   requested `0x7FF`, done `0x17F`. **Task 7 suppress, synthetic:** byte 0 of the
   activity-selection record (pointer at `0x26BC068`; `0x766750` = record[0]==1 || `0x78C710()`) = 1.
   Task 7 completed and **task 9 started**, as on 01.29. Tasks 9/10 (world content) stalled.
   **Done mask forced to `0x7FF`, synthetic, as 01.29.**
5. **Rungs 35 → 36 by themselves:** `activity:physics_join` → **`activity:in_world`**. 01.29's
   join-counter poke was not needed. In world, the client's own diagnostics show both connections
   established: primary BAP (109 sent / 112 received) and the Activity Host at 10.77.0.2:4110
   (169 / 64).

Synthetic in this run: the lab-orbit client patches, the launch poke, the task-7 byte and the
forced done mask. Everything else is served. The screen locked at 15:09 local (Windows lock
screen), so there is no in-world frame. I did not try to get past the lock. The client later left
`in_world` with the game's own `error.inactivity_timed_out` (the idle kick; no input reached it
while the screen was locked) and returned through `cleanup` to `setup:orbit` cleanly. The run ended
at its 45-minute limit with no fault, no crash ticket, and the servers stopped.

*Watched rerun (screen unlocked, run `…-in-world-watch-b`).* The first attempt failed with
**BUFFALO**: shadPS4's startup probe of the local shadNet server got no ServerInfo within its window,
so it disabled shadNet for the run and PSN sign-in failed. The previous probe answered in 0.15 s; it
looked transient, and a restart probed OK. The rerun repeated the same route. Task 7 clears with the
suppress byte, tasks 9/10 do not, and after the forced done mask the client reaches `in_world`. The
slice-set manager reports `initial_slice_set_instantion → fully_enabled` on region `PRV00.00`
(`pvp_moonbase`, `0x65BDC6F8`), the private match advertises, and session updates succeed. **The
screen is black:** no player character is constructed, so there is no camera, exactly 01.29's
"no handle = no camera". Next frontier: the spawn. Served route: the Activity Host's udt-5
`sensor_auth_update` (`D1_AH_SENSOR_PHASE`, `design/activity-lifetime-state.md`), which needs the
per-load sensor-name hash and generation (`world+0x1D`) read from 56578 memory, and was `[U]` on
01.29 too. Synthetic route: 01.29's two code hooks (lifetime-state getter, per-player reject `je`)
plus the spawn poke, re-derived for 56578.

*Served spawn route, measured live (runs `…-sensor-phase3`, `…-sensor-measure`).* The Activity Host
sent udt-5 `sensor_auth_update` phase 3 every 10 s: 25 frames, ~18 of them while in world.
`d1-winguest.py` now reads the decoder's preconditions live, each re-derived for 56578:
* **Lifetime record:** `lifetime` replicates `0x10C500` (same address as 01.29): `0x1072C0(0x10,0)`
  (count word `0x25D6FF8`, table `0x1AD5000`), then the registry walk (`0x1A238A0`), then
  `+0x1AC`. The phase byte is at **`0x38026207C`, the same address 01.29 recorded**.
* **World pool:** `worlds` reads the pool via `0x2A00A0` (`[0x26556D8] + i*[0x26556E0]`, stride
  `0xDB0` as on 01.29).
* **Group table:** read from the globals container (`0x1071D0`: groups `{base, hash}` at `+0x10`,
  stride `0x14`).

Results in world:
* `world[0]+0x1C = 1` (enabled) and **`+0x1D = 0`**, equal to the frame's `gen` (check at
  `0x2A1E88`).
* Group 0 hash is **`0xC2DACAF7`** with base 0, so name-ref sub 3 = record index 3, the lifetime
  record. In orbit the single group is `0x4786C0E0`, as on 01.29.
* The record stays at phase **−1**, `+0x1B4 = 0x811C9DC5`, and `+0x674/+0x678 = −1/0x811C9DC5`.

So every precondition this side of the walker holds, and the 01.29 defaults are correct for 56578,
yet nothing is stored. That isolates the one step 01.29 left `[U]`: whether the post-pass
(`FUN_002CE2A0` dispatch via `record+0xA8`) fires for a freshly delivered element. The next probe
needs a runtime hit counter or breakpoint in the client (the decoder `0x2A1C40`, the walker
`0x2D0CD0`, the setter `0x11DE30`), which is a new capability.

*Hit counters: where the served phase is lost.* `tools/ps4_setup/d1-wincount.py` adds entry hit
counters to the live client. It allocates a page within ±2 GB of the image, relocates each target's
first whole instructions into a stub (only push/mov/xor with no rip-relative operand or branch),
and has the stub do `lock inc` on a counter and jump back. The entry gets a 5-byte `jmp`, the
original bytes are saved to JSON, and `remove` restores them exactly; all six were verified
afterwards. `d1-relaunch.ps1` re-runs launch → rung 34 → in_world from orbit in the same session.
The 56578 setter is **`0x11DE50`** (01.29 `0x11DE30`; `memcpy(…, 0x504)` at `0x11DF10`).
Counts in world with the sensor repeating every 10 s:

| function | hits | what it is |
|---|---:|---|
| `0x29DDF0` udt-5 RX handler | 9 | one per frame |
| `0xBB1D00` sim-queue resolve world (`0x75D6B0(ah_id)`) | 9 | world found |
| `0x2A1C40` frame decoder | 9 | gen / enabled checks pass |
| `0x2D0CD0` named-globals walker | 8 | runs |
| `0x2D1600` walker terminator (`has_entry = 0`) | = walker (51/51 over a run) | frame parses to its end |
| `0x2D2F20` → `0x2D2F30` post-pass | **0** | never |
| `0x2CED30` dispatch, `0x11DE50` setter | **0** | never |

The post-pass runs inside a sweep (`0x2D1400` loop). It enumerates the world's resident object
set, resolves each entry's registry object `rbx`, and calls the post-pass when `rbx+0x60` equals
the current partition id (`0x345620`) or is ≥ 0x80. The lifetime record's `+0x60` is `0x8080039D`
(≥ 0x80), and its dispatch guard fields are as 01.29 documented (`+0xA8 = 0x8080039D`, ordinal 2,
`+0xB0 = 0x27FA2003`). So it **would** be swept if it were in the set. It never is. **This settles
01.29's `[U]`: the post-pass fires only for content resident in the world, and a freshly delivered
element outside that set is parsed and dropped.** The resident set is what tasks 9/10 (world
content) load, which is exactly what the forced done mask skips. The served sensor therefore cannot
land on this route until the world content is served or its residency is established another way.
The remaining in-world options are serving the task-9/10 content, or 01.29's synthetic spawn (the
lifetime-state getter hook, the per-player reject `je`, and the spawn word).

*Where the sweep's range comes from (static, 56578).* The decoder calls the walker with
`world+0x28` (`add r13,0x28` at `0x2A1E99`). At `0x2D134B..0x2D13DC` the walker sets its sweep
container to `[rbp-0xAF30]+0x10`, i.e. `world+0x38`. That container holds a total count at `+8`, a
group count at `+0x5E8` and group start indices at `+0x5EC`. The `0x2D1400` loop iterates those
entries, resolves each through `0x10304F0` and the registry, and post-passes the matching objects.
**The lifetime record is a globals record (`0x1072C0(0x10)`), not a member of the world container,
so the sweep cannot reach it however correct the frame is.** For the served phase to land, the
activity world must hold an entry for that record. That entry is presumably established by the
content tasks 9/10 load, which is the circularity both 01.29 and this build hit. Task 9 itself
(01.29 `FUN_00794D30`, 56578 `0x7965A0`) gates on two lookups: leg A, `0x1192A0(entity)` with
`[+8] != 0`, keyed by the controller→player map; and leg B, this phase. Next measurement: read
`world[0]+0x38` live (counts, groups and entry ids) in world, to see which records the world
container does hold and whether any lifetime-class record ever appears.

*Measured (run `…-worldset`, in world, sensor repeating):* `worldset` (`d1-winguest.py`) decodes
`world[0]+0x38` as `0x10304F0` does. It holds **97 entries in 3 groups (starts 0/32/64)**, all from
one pool: tags `0x..F420xx`, pool type `0xF42`, objects 0x70 apart, `+0x60` = −1 or 0. The lifetime
record (`0x27FA2003`, pool type `0xFA2`) is **not** in it, which confirms the static reading. Open
point: those 97 entries would themselves pass the post-pass condition (`+0x60` −1 is ≥ 0x80, or
0), yet the bridge `0x2D2F20` counted 0. So the walker's outer loop over containers
(`0x2D114A`/`0x2D134B`: `cmp [rbp-0xAED8], [rbp-0xAEC8]`, pointer stepped by 0x10 per container) is
gated by more than the container contents. That needs either a counter mid-walker (requires the
tool to relocate a non-prologue site) or a static trace of `[rbp-0xAEC8]`.

*⭐ Root cause of the dropped served phase, measured (run `…-replicas`).* The walker calls its
terminator `0x2D1600` (`call` at `0x2D10BA`, then `je 0x2D15BC` exits on false) **before** any
post-pass. `0x2D1600` walks the world's replica pool `[world+0x28]` and **returns false if any
pending replica (`+0x15 != 0`) has owner `+0x60` equal to the local partition id or greater than
`0x7F`**. The local id is bits 3..8 of `[[0x2693B60+8]+0x2114]` (`0x345620` → `0x12D700`).
`d1-winguest.py replicas`, read in world:
* local partition id **0** (the client);
* the replica pool (750 slots, the named-globals replicas, tags `0x..F420xx`): almost every slot is
  **pending with owner `0xFFFFFFFF` (unowned)**, so the terminator rejects every frame;
* slot 3 (`0x33F42003`, group 0 base 0 + sub 3, exactly our name-ref) is the only one **not**
  pending. The walker applied our entry there; the frame as a whole is refused.

So the served phase fails because **nobody owns the world's replicated globals**. In a real session
the activity host is their authority (owner = its non-local peer index, 0..0x7F), so its updates
pass. Our Activity Host never establishes that ownership, and its membership names member 0 (the
client) as peer-session host. The served fix is in the AH's session/authority protocol (entity-slot
and authority-mask messages: udt 0 `entity_slots_allocated`, 30/31
`query_entity_slot_authority_mask…`, membership host identity), not in the sensor frame. Next:
find the client writer of replica `+0x60`.

*The owner writers (static).* The only stores of a computed owner to replica `+0x60` are
`0x2C16A0` and `0x2C2000`: `owner = 0x2C1040(id)`, also mirrored to `+0x64`, with negative meaning
unowned. Both `0xFFFFFFFF` stores are resets (`0x2C16F0`, `0x2C1F80`). `0x2C1040` searches an
identity table at `[0x2671BC8]` with `memcmp` (`0x12AA800`) and returns the matching index, so
ownership is assigned by peer identity. Callers are `0x2BC910` (16 callers), `0x2BCA10`, and
caller-less `0x2C2710`/`0x2C2890` (likely vtable slots). One chain climbs `0x2BC910 ← 0x2B5EE0 ←
0x2B7060 ← 0xB6AB80 ← 0xB641D0 ← 0xAE3A40` (network-channel layer). Working hypothesis: replica
authority is granted when a remote peer's identity is registered with the simulation, and our
Activity Host never registers as that peer (membership names the client as host). Next: identify
the identity table's entries live (who is registered) and the message that adds a peer to it.

*Correction, and the served fix (13 October, run `later-56578-sensorclients2`).* The ownership
hypothesis above is wrong. `0x2C1040` is a refcounted WebApi user-context cache keyed by an
NP online ID: four 0x1C-byte slots (20-byte ID, handle `+0x14`, refcount `+0x18`). On a miss it calls a
runtime-bound function with `(libCtxId, onlineId)` and releases through its pair. Live, slot 0 is
`D1Guardian` with handle `0x10008`. `0x2C16A0`/`0x2C2000` store that handle into WebApi request
objects that happen to use `+0x60`. They are unrelated to replicas.

The replicas are pool "sensor entity clients 0" (tag `0xF42`, 750 slots of 0x70 bytes, created
at `0x7595B4`). Each holds:
- `+0`: name `{u32 hash, i8 @+4, i16 sub @+6}`
- `+8` / `+0xC`: B and A types
- `+0x10`: checksum
- `+0x15`: pending
- `+0x60`: owner

The walker `0x2D0CD0` clears a client's pending byte (`0x2D106D`) only when the frame carries an
entry for it. The terminator's owner test means "the local machine must receive this one":
owner == local id, or unassigned (> 0x7F). A client owned by another machine is skipped. So the
fix is in the frame, not in authority: deliver every such client. Entry grammar from the walker:
1. `has_entry`, then the name ref (codec `0x808004B1`)
2. `u32 body_bit_length`, used only to skip unresolved names
3. if A is set: `fresh` plus the replicated value (a presence bit; the object follows only when it is set)
4. if B is set: `has_b`
5. then a `u32` into `+0x10` when B is present

`fresh=1, presence=0, has_b=0` default-initialises a client and clears it.

Live, pvp_moonbase had 97 clients in three groups: `0xC2DACAF7` ×19, `0x39774EE1` ×60, `0xD472B0EB` ×18.
Only the audited lifetime entry (sub 3) was clear. `d1-winguest.py sensor-clients` writes the list;
`D1_AH_SENSOR_CLIENTS` serves it, which made a 1,147-byte frame with 96 extra entries
(`d1-56578-sensor-clients.patch`).

Result, all served, with no getter hook, spawn hook or unfade:
- 0 clients pending.
- Lifetime phase byte `0x38026207C` = **3**.
- Player 0's spawn word = **1**.

The spawner and gate then ran every tick: 609 hits in 20 s, no reject message formatted, and the
lifetime `+0x674/+0x678` reader `0x10CB60` was never reached. The biped handle stayed −1 and the
screen stayed at full fade. About 6 minutes after in_world the client left with
`error.inactivity_timed_out` (no input in world) and returned to orbit.

*Where the served spawn stops: the gate's leg A (two more loads from orbit, same result each time:
in world, 0 pending, phase 3, spawn word 1).* Block-start counters on the gate `0x415700` for 10 s:
- entry: 310 hits
- spawner reject `0x41314D`: 310 hits
- spawner accept `0x412DDC`: 0 hits
- first interior block `0x415776`, and every later block: 0 hits

So the gate refuses at its first test: `0x1192B0(tag)` returns null. The spawner loops over the
**"player array"** (`0x2689580`, 20 × 0x1C0, `c_player_manager`; not the players pool) and passes
each entry's tag. `0x1192B0` reads that entry's `+0x50` tag. It then resolves the object, takes the
object's `+0x1F8` tag, and adds the object's `+0x200` offset. The gate needs `[result+9] != 0`.

Live, entry 0 is populated:
- `+0x08`: machine id `E0D55E272F90`
- `+0x40`: 3
- `+0x48`: own tag `0x1BFF2000`
- `+0x4C`: players index `0xEC700000`

But `+0x50..+0x5B` are all FF.

01.29's blocker, the controller→player map (`[0x273EB80]+0xC0` on 56578), is already set
(`0xEC700000`), and the players pool count is 1. So this is exactly 01.29's open "which other leg
refused" (design/activity-lifetime-state.md §5): leg A. The same record's `[+8]` is task 9's test,
which is why rung 34 still needs the done-mask force.

No direct writer of `entry+0x50` was found among the 400+ functions that reference the table
(dword/qword/vector stores at `+0x40..+0x5F` checked). The writer reaches the entry through a
pointer. Next options:
- a hardware write-watch on `entry+0x50`, attached as a debugger so first-chance exceptions pass through;
- the AH entity-slot messages (udt 0 `entity_slots_allocated`, 30/31 authority mask), on the guess
  that `+0x50` names the player's simulation entity.

*Leg A traced to the per-player client bundles (13 October, hardware write-watch).*
`d1-watchwrite.py` attaches as a debugger and sets data-write breakpoints on every thread,
passing every other exception back. It was armed at orbit on player-array entry 0 (`+0x48`,
`+0x50`) across a world load and caught three writers:
- the pool memset;
- the entry constructor `0x301470`, which sets the tag at `+0x48` and `+0x50 = -1` at `0x30150F`;
- the constructor's caller chain `0x3026BF ← 0x90EF35 ← 0xBA9E94 ← 0xF70404` (add-player).

The emulator stayed up and the watch detached cleanly.

The only non-reset writer of `+0x50` is the setter `0x301C60` (`mov [rdi+0x50],esi`). It is called
only from the linker `0x1195F0(key)`, which add-player calls at `0x3026DF` with
`key = entry+0x18` (live `0xD100000000000201`). The linker walks the world content's **type-12**
objects (the iterator `0x100CB80` over the table at `content+0x520`, 0x38-byte rows; content from
`[0x1AD5000]`, index `[0x25D6FF8]`). For each object:
1. resolve `obj+0x208` (tag) plus `obj+0x210` (offset) to a record;
2. link the object's tag into `entry+0x50` when the record's first 8 bytes equal the key.

The gate then reads `obj+0x1F8`/`+0x200` → record `[+9]`, and task 9 reads `[+8]`.

Live in world, pvp_moonbase has 12 type-12 objects. They are "client bundle instances" slots 7–18,
the same pool as the lifetime record (slot 3). Each object's tag points back at itself:
- the gate record is the object's own `+0x218`, reading `ff ff ff ff c5 9d 1c 81 …`;
- the identity record is its own `+0x258`, reading **all zero**.

So no bundle carries our player's key, and the link never forms. These 12 objects are sensor
clients `0xC2DACAF7` subs **7–18**, i8 `0x0C` (= type 12): A type `0x808022A5`, B type `0x80802C30`.
We currently send them fresh and value-less.

Next: serve one bundle (sub 7) a real value, with identity = the player key and the gate byte set.
That needs the A/B type setters (the post-pass dispatch `0x2CED30` per type) and the schema layout
of `0x808022A5`/`0x80802C30`.

*Bundle test (SYNTHETIC, `d1-bundle-poke.py`, 13 October 18:54 UTC).* In world, the first
type-12 bundle (tag `0x4EFA2007`) was given:
- the player key at `+0x258`;
- `1` at `+0x220`/`+0x221`.

Within 10 s both were back to defaults (`+0x218 = ffffffffc59d1c81 0000…`, `+0x258 = 0`), and
`entry+0x50`, the spawn word (1) and the biped (−1) did not move. The AH re-sends the udt-5 frame
every 10 s, and its fresh, value-less entries for subs 7–18 default-initialise exactly these bytes.
So the sensor value for those clients *is* the bundle record, starting at `+0x218`. The identity sits
at value `+0x40`, the gate byte at `+9`, and task 9's byte at `+8`.

The post-pass `0x2CED30` hands each value to a bound BlamScript function (`0x353240` /
`0x353420` on the record's `+0xA8` binding), not a fixed setter.

Blocker for serving it: `d1-oracle object --tag 0x808022A5` (and `0x80802C30`) faults in the encoder.
The field bitmask size is computed from the descriptor's field count (`[desc+desc[0x48]+0x5C]`,
`0x2C8F3B`), and the harness gets a negative memset length at `0x2C8F89`. The reported struct size
is 0x78 for both. Fix that in the harness, then encode sub 7's A value:
- default image;
- `+0x40` = the player key (`entry+0x18`, live `0xD100000000000201` = member id `0x201`, high
  byte `0xD1`);
- `+8`/`+9` = 1.

Then serve it through `D1_AH_SENSOR_CLIENTS` with a value.

*🎉 Served spawn on build 56578 (13 October, run `later-56578-served-spawn`).* A Guardian spawned
in pvp_moonbase with **no spawn hooks, no gate patch, no spawn poke and no unfade**. The fade lifts
by itself, the Ghost-raise intro plays, and the view moves.

What is synthetic is unchanged from the drive to `in_world`:
- the launch poke;
- the task-7 suppress byte;
- the rung-34 done-mask force.

The chain, all measured live:
1. *Oracle.* Kind 0x23 is encoded by `0x102A450` as type `[[0x197E160]]` = `0x80801932`.
   - The selector `0x25CC10` is `[[0x26527E0]+0xC] != -1`; it is true live. Otherwise the type is
     `[[0x191A240]]` = `0x808004AC`.
   - The box encoders read further start-up cells: `0x1996050`, `0x1974C00`, `0x1974BE0`.
   - All of these are zero in the file. Seeding them (`CodecAddrs::type_cells`) makes `0x80800646`
     and the bundle types `0x808022A5`/`0x80802C30` encode and round-trip.
   - Kind-0x23 field (the identity) comes out byte-reversed on the client relative to the oracle
     image (`01020000000000d1` → `d100000000000201`), so the image carries it reversed.
2. *Link.* The bundle setter `0x409710` copies the 0x78-byte A value to `obj+0x218`. When the
   identity at `+0x258` changes, it calls the linker `0x1195F0` for the old and new key. Serving
   the player key therefore links the player when the value lands, whatever the load order. Live:
   `entry+0x50 = 0x4EFA2007`.
3. *Gate record.* The gate's record is the bundle's B record (`obj+0x200` = 0x290), not A. The
   client rewrites B every tick from local state in `0x4098E0` (path `0x409BD1`), called from the
   sensor post-pass `0x2CF13D`/`0x2D1F46`. Serving B is pointless (the format is
   `has_b, flag→+0x16, fresh, presence, object, u32→+0x10`).
   - `+0x298` = local id ≤ 0x7F and `!0x9141F0()`.
   - `+0x299` (the gate byte) latches to 1 when the A value's `+0x38` (`obj+0x250`) equals
     `players[i]+0x738` (copied to `+0x2C8`; −1 live), and `0x77B920()` holds (`[[0x26CA0C8]] == 4`,
     true live).
4. *Value served for sub 7.* A `0x808022A5` with:
   - `+0..8 = ffffffff c59d1c81`
   - `+8/+9 = 1`
   - `+0x38 = 0xFFFFFFFF`
   - `+0x40` = the player key reversed

   `d1-winguest.py sensor-clients --all --bundle` writes it.

Result: `+0x298 = 01 01 00 02`, the spawn word is consumed, and the biped is `0x4CFAC067` within
seconds of in_world.

A SYNTHETIC one-byte test (`+0x250 = -1`) on the previous run gave the same spawn first. The fully
served run then reproduced it with nothing written.

Tooling: `d1-watchwrite.py` drains pending breakpoint hits before detaching. A queued hit
delivered after detach killed run `bundle7d` with an unhandled `0x80000004`.

Next:
- earn back the three drive pokes (task 7, rung-34 tasks 9/10). Task 9 reads this same bundle
  record's `[+8]`, now 1.
- input/possession past the Ghost intro.

*Rung 34 now clears by itself (13 October, run `later-56578-noforce2`).* With the served bundle in
place, `d1-drive-in-world.ps1 -NoForce` writes neither the task-7 suppress byte nor the done-mask.
Rung 34 reached `requested 0x7FF started 0x7FF done 0x7FF missing 0`. The client went
initial_slice_set_loading → physics_join → in_world on its own (rung 34 to in_world in under 30 s),
and the Guardian spawned (biped `0x4CFAC067`, gate byte latched, link `0x4EFA2007`).

The only synthetic step left on the route is the launch poke from orbit (`d1-winguest.py launch`).

A BUFFALO error the operator saw at boot that evening came from my own stop. Killing the boot
script mid-run left the previous session's `shadnet-server`/`d1-responder` alive, and the next
emulator probe got "reachable but no ServerInfo header". Kill all four server processes (and check
port 31313 is free) before relaunching.

*The last poke, and the Director (13 October, runs `director`–`director5`).* The launch poke stands
in for the desired-activity session parameter, which the orbit Director sets when the player
chooses an activity. Driving orbit by pad (`d1-drive-to-orbit.ps1`):
- The served unlock flags show their popups ("Crucible unlocked", Armsday, Age of Triumph, a
  Destiny 2 pre-order ad: dismiss that one with Circle, never Cross).
- "Set Destination" opens the Director (bottom-left button).
- The Director draws only Earth, Earth gets no hover tooltip, and nothing is selectable. Same
  wall as 01.29 (architecture.md §3.5).

Measured on 56578, with counters installed before orbit loads:
- The node builder `0x9DAF40` (01.29 `0x9D92A0` + 0x1CA0; sticky "do not offer" ORs at
  `0x9DB760`/`0x9DBA80`/`0x9DBA91`, `[+0x138]`/`[+0x100]` at `0x9DBAC0`/`0x9DBAEF`) **never runs**:
  not at orbit load, not on opening the Director, not on clicking Earth.
- Its callers are vtable methods. The per-frame Director update `0x9E7B50` passes the
  family-4/type-4 lookup (`0x5753C0(4)` non-null every frame) but calls the builder only when the
  focused-destination index `[0x955380()+2]` differs from its cached `+0xEA`, or `+0x100` has
  bit 0x8000. Focus never changes because Earth never takes focus.
- Availability: `0x2601F0` (the requester, 70 callers) sees valid=1, dirty=0 and never rebuilds
  through `0x260280`; the other builder `0x2604B0` ran twice at load.

Next:
- why Earth's map node is not focusable (the planet/book node list, upstream of `0x955380`);
- whether it needs the activity-availability content our server does not yet supply.

*Tooling.* `d1-wincount.py` now places stubs in the text segment's unused tail
(`0x18D4400`, 0x3C00 bytes, zeroed again on remove). It range-checks every rel32 before writing,
and rolls back on any failure.

An early-boot install had crashed mid-way (an external stub page out of range) inside a 20-try
retry loop. That left the client failing sign-in with CHICKEN (`error.no_investment_connection`).
The retry loop is now 3 attempts and each attempt is all-or-nothing.

*Correction: the Director's "focus" is the desired-activity session parameter.* `0x955380` jumps to
`0x78B450`, which returns the value (`+0xE4`) of the session's desired-activity parameter at
params `+0x5A8` (class `c_network_session_parameter_desired_activity`, vtable `0x196D8C0`), or
NULL when it has no value. Live at orbit `has_value = 0`. So the node builder runs only once a
desired activity exists. The launch poke writes that same parameter class (params `+0x9B8`), so it
stands in for exactly the choice the Director would make. Why Earth is not focusable is still
open.

*Control after spawn (13 October, served route plus launch poke only).* After the Ghost-raise
intro the Guardian is under pad control:
- right stick (shadPS4 IJKL) turns the view freely (Earth, the base, the horizon);
- left stick (WASD) walks; the position changes, into a structure;
- radar and health bar are drawn.

The view keeps the empty first-person hand with the Ghost out, and R2 shows no weapon. Most likely
no equipped loadout is served (inventory). Touchpad (Space) did not put the Ghost away.

Next: serve an equipped loadout (`design/geared-guardian-fixtures.md`, `d1-oracle inventory`).

*Gear for 56578 (13 October, runs `geared2`/`geared3`, patch `d1-56578-item-tables.patch`).*
The 01.29 fixtures cannot be reused: 3,164 of 7,544 item content indices moved and 3,681 world
tags changed.

Item index (`d1-oracle item-index write`, now per build via `item_index::source()`):
- 56578 reads `ps4_investment_globals_012f_6` entry 8126 (tag `0x80A5FFBE`, class `0x80801933`),
  8,609 items. Generations `012f_2`–`012f_6` all carry the entry; `0130`–`0133` do not.
- Written to `capture-rig/builds/later-56578/item-content-index.tsv` (generated, not in a repo).

`inventory rebuild` bug, fixed:
- It recovered each staged item's hash by looking up the element's OLD content index in the NEW
  table, so 01.29 Gjallarhorn (ci 6299) came back as 56578 ci 6299, a different item (`0x7EAF157D`).
- `--from-index <old table>` maps through the old build's table instead.
- Use it once, from the 01.29 staging. Re-running it on converted output double-maps.

Art table (`art-arrangement write`, per-build path):
- Every staged item's equip ordinal matches the 01.29 table.
- Gjallarhorn moved 6299 → 6323 and keeps arrangement 1229; Oryx Mask moved 6488 → 6514 and keeps
  curve 70. These are the new 56578 pins.

Fixtures built:
- `profile build` → `profile-geared-385.bin`: 20 equipped, with `+0x2B80` unchanged on 56578
  (fields 0–9 do not move).
- `character build-peer --with-family2` → `rosters-f0-geared` (20/20 equipment, 14 with a content
  index).
- The inventory element checksum is still `0xEF38431D` on 56578 (184 B), so the items are accepted.

Result in world (served route, launch poke only):
- Rung 34 still clears by itself with the geared family-0/1.
- The biped spawns.
- The first-person arm now draws ARMOR (a white wrap over the glove; the base glove before).
- Still the Ghost-raise pose and no weapon on R2: profile plus inventory alone changed nothing
  visible; the roster changed the arm.

Next: what holds the player in the Ghost pose instead of the weapon-ready state.

*No weapons exist (run `geared3`).* The sobject pool (header `0x2692220`; 1,500 × 0xF0; type byte
`+8`; free when `+6 == 0xFEFE`) in world held:
- 1 biped;
- 0 weapon (11), 0 gear (19), 0 item;
- the rest: 356 system, 98 prop_simple, 39 interactive, 39 emitter, 20 static_mesh, 19 prop_simulated.

So the player's weapons are never created. The armor seen is the biped's appearance (roster
`0x80801855`), not gear objects.

None of the action classes ran, idle or under R2 / Triangle / R1 / touchpad:
- `c_action_impl_ghost_deploy` (vtable `0x1932E70`: `0x379980`/`0x379C80`/`0x379EB0`);
- `ghost_summon` (`0x37BDB0`/`0x37BF10`/`0x37C050`);
- `weapon_ready` (`0x39AE30`/`0x39B320`/`0x39B3A0`).

The empty hand with the Ghost out is what the client draws with nothing to hold. The next layer is
the gear content demand, the world-tag loader (`items-and-gear.md` §B15): eight
`c_investment_world_tag_loader_fill_desired_*` strategies (56578 typeinfos `0x193EB50`…),
including `..._items_peer_view` and `..._items_default_gear`. Weapons need their world tags
resident before they can be created.

*Teardown crash.* After `error.inactivity_timed_out` the served-route client crashed in cleanup at
eboot `+0xBC09AE`, inside entity-definition vfunc 19 (`0xBC0990`), the same site as the forced
route's crash. It is a teardown crash too, not only a forced-spawn artefact.

*Activity 407 is NOT Elimination on 56578 (13 October, operator's hunch, confirmed from packages).*
d1preservation's activity table (`activity-launch.md` §2.1, 407 = Elimination) is 01.29's: block
1063 of `012f_3`, at `+0x9920`, stride 0x10. 56578 carries the table in `ps4_investment_globals_0131_6`
blocks 182 and 187 (`+0x29F30` in block 182; `012f_6` has no copy; found by scanning every block
for 01.29's element-1 hash `0xDF283AFB` and Elimination's `0x6D714C3A`).

The private-match band moved:

| mode | 01.29 → 56578 | mode | 01.29 → 56578 |
|---|---|---|---|
| Clash | 393 → 400 | Skirmish | 402 → 410 |
| Control | 394 → 401 | Inferno Clash | 403 → 411 |
| Supremacy | 395 → 402 | Inferno Control | 404 → 412 |
| Mayhem Supremacy | 396 → 403 | Mayhem Clash | 405 → 413 |
| Inferno Supremacy | 397 → 404 | Zone Control | 406 → 414 |
| Rumble Supremacy | 398 → 405 | **Elimination** | 407 → **416** |
| Rumble | 399 → 406 | Inferno Rumble | 408 → 417 |
| **Salvage** | 400 → **407** | Inferno Skirmish | 409 → 418 |
| Rift | 401 → 408 | Inferno Salvage | 410 → 419 |
|  |  | Inferno Doubles | 411 → 420 |
|  |  | Mayhem Rumble | 412 → 421 |
|  |  | Sparrow Racing | 413 → 422 |

So every 56578 run so far launched **Salvage** (hash `0x19BC2182`, which is the composition log's
`game mode`). Serving phase 6 drew "MISSION END: Seconds remaining: 0", so phase 6 is the ending
phase, not RUNNING. `activity_modes.rs` is 01.29-pinned like the item index; it needs the same
per-build source.

Run `later-56578-elim` (`--activity-id 416`, everything else as `geared3`, phase 3):
- composition `game mode 0x6D714C3A` (Elimination) on pvp_moonbase;
- rung 34 clears by itself;
- the served spawn works: link `0x4EFA2007`, biped `0x4CFAC067`, no pending sensor clients with the
  Salvage-derived list, phase 3.

The view is identical to Salvage: same spawn point, Ghost-raise pose, armored forearm, and still
no weapon sobjects (types: 1 biped, the rest props). The mode was wrong but was not the cause.
Launch Elimination with **416** from now on.

*World-tag loader and the 4/2 + 4/4 objects (13 October, runs `wtl`, `f42`).*

Per d1preservation (`character-and-investment.md` §7.4), the character-instance art loader works
from family 4/2 and 4/4 via the selection manager, and our responder sends neither by default. Both
instance checksums are unchanged on 56578: 4/2 `0x80800FB2` → `0x0ECCD1F8`; 4/4 `0x8080102E` →
`0xC4060975`. Run `f42` served `D1_FAMILY42=1` and `D1_UNLOCKS=key`.

The eight `fill_desired_*` strategies (56578 methods `0x575F30` character_instance, `0x576410`
talent grids, `0x575FA0` peer_view, `0x576070` select-character view, `0x576150` default_gear,
`0x5763B0` character creation, `0x576480` rich presence, `0x5764A0` reward incidents):
- counted from boot (text-segment cave): they run during boot and orbit, 1–6 calls each;
- they make **no** further calls during the Elimination launch, world load or spawn. Run `wtl`'s
  all-zero reading was only because those counters went in after orbit had loaded.

With 4/2 + 4/4 served:
- the orbit nameplate now shows the character's EMBLEM (it was blank);
- the Director is unchanged (only Earth, not selectable);
- in world the view is unchanged, and the sobject census still has 0 weapons.

So weapon creation is not driven by these per-view loaders at spawn.

Spawn-time counts (run `spawncount`, counters installed at orbit, Elimination 416):

| function | orbit | at in_world | +45 s |
|---|---|---|---|
| spawner `0x412800` | 50 | 825 | 2163 |
| spawner accept path `0x412DDC` | 0 | 0 | 127 |
| sobject constructor `0x32DBF0` | 0 | 579 | 584 |
| creation helpers `0x308CA0` | 0 | 22 | 44 |
| `0x3090D0` | 0 | 20 | 39 |
| `0x309360` | 0 | 0 | 9 |
| `0x32E4F0` | 0 | 11 | 35 |
| `0x32E340` | 0 | 47 | 47 |

So 5 sobjects were constructed after in_world (1 is the biped). The census afterwards still shows
no weapon and nothing pending delete, and the only recently freed slot near the top was an emitter.

Other leads that came up empty:
- `c_simulation_weapons_game_system` methods only allocate its globals (`0x9210`-byte block).
- The equipment iterators' vtables are referenced only inside the investment layer (`0x2693A0`,
  `0x269470`).

*Every construction typed (run `ctor2`, Elimination 416).* `d1-watchwrite.py --exec` (new:
execute breakpoints, logging RAX and the definition at `[rbp-0x150]`, with RF set on resume) sat
on the constructor's type store `0x32DD69` (breakpoint at `0x32DD6E`). It ran from orbit through
the launch, load and spawn: 585 constructions.

| type | count | type | count |
|---|---|---|---|
| system | 357 | static_mesh | 23 |
| prop_simple | 98 | prop_simulated | 19 |
| emitter | 40 | prop_cinematic | 6 |
| interactive | 39 | vehicle | 1 (the orbit ship, built at orbit) |
| biped | 1 | | |

The biped is created by the spawner at `0x413B96` (definition head `abc1d2ba…`). After it came only
three interactives (from `0x6678E3`) and one system object. **No weapon (11), gear (19) or item
sobject is ever attempted.** The weapons are not created and then lost: the client never asks
for them.

On the spawner's success path (`0x4145DF…0x414C7E`, ~60 callees), the only callee with an
object-creation sequence is `0x7420F0`, which places an object at a transform (Ghost/corpse-like,
`[U]`).

Next:
- find what the 01.29/retail flow does to arm a spawned player; candidates are a later event or
  message (e.g. an Activity Host loadout/equipment message) rather than the spawner itself;
- check the AH's message list for equipment or loadout types we never send.

*The spawn never reads the profile's equipped items (runs `equipread`/`equipread2`).*
- `d1-memfind.py` (new) located the profile's equipped-SOID list live at `0x37F90BCC0` (stable
  across runs).
- `d1-watchwrite.py --read` (new: RW=11 breakpoints) watched the subclass, primary and heavy slots
  from orbit through the Elimination launch and spawn: **0 accesses**.
- Positive control, same slots: opening the character menu at orbit gave 50 reads from `0x269708`
  (the equipped accessor, 01.29 `0x269080`), stack `0x9F3366`.

The AH protocol has no loadout/equipment udt. udt 21 (`client_authoritative_data_update`), which
the client does send, carries the session id string (bytes XOR 0x80:
`Tue_10132026_221513_000001@orb@D1Guar…`).

So whatever arms a spawned player in this build does not come from the profile at spawn. Armor
comes from the family-0/1 peer roster's appearance. Next suspects:
- the peer-roster equipment path;
- a sim-queue element that hands the loadout over (the 95 sim-queue payload schemas,
  `architecture.md` §8).

*Peer-roster equipment reads (run `equipread2`, partial).*
- The family-0/1 peer object was found live at `0x37F8E4040` (SOID `0xD100000000000501`).
- Equipment block at `+0x218` (`0x30` block + `0x1E8`), 64-byte elements, content index at `+8`;
  it holds helmet 6514, primary 5043, special 5263, heavy 6323 (the 56578 indices).
- Read breakpoints on the primary, heavy and helmet content indices from orbit through launch:
  498 hits, all from ONE site `0xA2F862` (fn `0xA2F6D0` ← `0xA3FA22`, UI code range) on the
  heavy slot only, 466 of them at orbit. Primary and helmet were never read.

The client then crashed at the known cleanup site (`+0xBC09AE`) around the launch, so the
in-world half is not conclusive; rerun on a clean launch. The open question is which
spawn-time path creates a player's weapon sobjects (type 11), and what it needs.

BUFFALO again at one boot this evening on a fully clean start (fresh server, port free): the probe
connected and gave up after 3 s with no ServerInfo header. A plain relaunch passed. So the probe
failure is intermittent and separate from the orphaned-server cause; its root cause is open.

*Tooling.* `d1-wincount.py`:
- now checks every prologue before hooking any, and hooks with the process frozen;
- takes `D1_COUNT_ADDRS`.

A mid-way refusal had left a live counting jump on the lifetime getter, with no state file to
remove it. It was restored by hand to the original bytes and verified. 01.29's next steps
(two code hooks and a spawn poke to construct a biped) are not attempted on 56578.

*🎉 Build 56578: a Guardian in world, rendered and moving (13 October, run
`…-later-56578-forced-spawn`, `pvp_moonbase` / 407).* This is 01.29's GUARDIAN-IN-WORLD recipe,
re-derived for 56578, driven through `d1-winguest.py`:
1. **Served:** `D1_ROSTER_DIR=capture-rig/rosters-f0-hybrid` (family 0 answered with the
   2-object hybrid fixture, 2,415 B) with `D1_ANSWER_FAMILIES=0,2`, and the Activity Host with
   membership. With the fixture, **rung-34 task 7 cleared on its own** (done `0x17F → 0x1FF`, no
   suppress poke). The character definition was installed (`player+0x85C = player+0x1660 =
   0x811C9DC580A6A06C`, 01.29's value), and **player slot 0** appeared in the spawner's player table
   (`0x2689580`, 20 slots; it was empty without the fixture, which is why the spawner never reached
   its gate in earlier runs).
2. **Synthetic:** the launch poke, then the frozen hooks (`NtSuspendProcess` around the write): the
   getter `0x10C500` set to `mov [rdi],3; mov eax,1; ret`, and the per-player reject `je` at
   `0x412DD6` (spawner `0x412800`, gate `0x415700`) set to NOPs. Then the done mask forced to
   `0x7FF`, `in_world`, and the spawn word `player[0]+0x0C = 3` (pool `[[0x273EA80]+8]`, stride
   `0x2AF0`; player 0 is at the start of the pool).
3. **Body built:** `player[0]+0x644` went `0xFFFFFFFF → 0x7BFAC57B`.
4. **Picture:** still black until the stuck screen fade was cleared. That is `[0x26AD7F0]` (01.29
   `[0x2694B90]`) → block `0x3806FCA00` (**the same heap address 01.29 recorded**): count 1,
   mode 1, alpha 1.0. All three were zeroed (`unfade`), and the task-7 suppress byte is held by
   re-writing it every 2 s. **First-person view on the Moon, Ghost out, radar up**
   (`cfw-staging/2026-10-13-op/64-unfaded.png`).
5. **Control:** 2 s of left stick moved the view through the scene (`65-moved.png`). The forced
   spawn is controllable on 56578, which 01.29's first forced spawn was not.

6. **End of run:** the emulator later exited on an access violation at eboot `0xBC09AE`, the first
   dereference (`mov edi,[rcx]`, rcx NULL) in **`c_simulation_sobject_entity_definition::vfunc[19]`
   = `0xBC0990`** (RTTI). That is 01.29's documented forced-path crash (`0xBBDEDE`, "the NOP'd-gate
   biped lacks an entity-definition backpointer a legit spawn would fill"), on the same vfunc. No
   guest crash ticket. The servers stopped cleanly.

Still synthetic: the lab-orbit client patches, the launch poke, the getter hook, the reject NOP, the
done-mask force, the spawn word, the fade zero and the held suppress byte. The served route
(udt-5 phase) remains blocked at the residency sweep (above), and the done-mask force skips the
world content that would fix it.

*Orchestrator.* `Start-D1ContainedBoot.ps1` now:
* taps confirm while the log reports `profile_setup`, controlled by `-WizardTaps`/`-WizardIntervalSeconds`;
* brings the window to the foreground and sends real key input, with `-NoForeground` to opt out.

A PowerShell trap cost one run: `$wizardTaps` and the `$WizardTaps` parameter are the same variable.

*Lab patch files* (PS4 tree):
* `CUSA00219-56578-lab-diag-skip-wizard.xml` is lab-diag plus the c1 skip.
* `CUSA00219-56578-lab-orbit.xml` is that plus the voice entries. It is the set that reached orbit.

## Handoff items

- 12 October package `matildaspc-20261012T201422-…`: NAS re-check at
  21:03:55 UTC passes (all three payloads match `hashes.json`, no unlisted
  files). Receipt: `PRIVATE_ENGINEERING/2026-10-12-daily-handoff-receipt.json`
  beside the dated handoff notes. No existing `PRIVATE_ENGINEERING` folder was
  found on this PC, so its location should be confirmed.
- This work is after the 12 October cutoff and belongs to the 13 October edition.

## Next

1. ~~Apply the redirects~~ (done 13 October); OAuth and `/SignOn` now pass.
2. ~~Unsigned-manifest lab byte~~ (approved and run 13 October; the game moves
   on to `marionberry`).
3. **Windows guest transport:** give the guest a non-loopback, non-routable
   local address (the 01.29 Linux design used `10.77.0.2` on an isolated
   interface). On Windows that means a host-only adapter, which is a system
   network change the user would make as administrator. Then point the redirects
   and the loopback-only servers at that address, and fix shadPS4's empty
   `sceNetCtlGetInfo` local IP if it persists.
4. Re-derive the build-specific parts: Oodle entry points (blocking schema
   `0x80A5E37A`), the eboot atlas, BAP tables/registry, the manifest cap and the
   account/profile schemas. Use RTTI and strings, since the 01.29 bytes are not
   available for signature matching.
5. Check for aknumbers' 1.33 release before deep address work. Its executable
   hash decides whether his addresses apply directly.

Window frames are window-rectangle captures and include a sliver of the desktop
at the edges. They are private evidence and need cropping before any daily
media use.

### 14 Oct: the clean spawn never reads the peer roster's gear slots (run peerread3)

- The socket fix held. The run reached orbit and then Elimination (416, `pvp_moonbase`) with no BUFFALO, CHICKEN or CENTIPEDE. The Guardian spawned in world holding the Ghost, with no weapon.
- The peer f0/1 object was again at 0x37F8E4040, with equipment at +0x218.
  - Heavy content index 6323 (Gjallarhorn) is at 0x37F8E4460. The earlier "primary" label was wrong.
  - Helmet 0x1972 is at 0x37F8E42A0.
  - The slot at 0x37F8E4560 (content index 0x5E8) is read about 50 times a second at orbit by 0xA2F862 (from 0xA3FA27). That is a UI consumer.
- An 8-byte read/write watch on the heavy and helmet slots ran for 200 s, from just before the launch until well after the spawn: **0 hits**.
  - The spawn path does not consult this copy of the equipment.
  - The armour that renders must come from another copy, such as the character transcript or the profile's render data.
- Together with 0772832 (the equipped list is never read), this rules out "the data is served but ignored by the spawn".
  - What is missing is the step that asks for the player's gear on spawn.
  - Next candidates: the activity script's loadout grant at round start (a sim-queue element), and whether Elimination waits for a round state that phase 3 never reaches.

### 14 Oct: where weapons are not made, phases, and teams (runs `weapcount`, `phasestep`, `teams`)

**Not the weapon path.**
- The constructor callers clustered at 0x711690–0x71B7B0 are the projectile system: `c_projectile_component_game_system` has slots at 0x7286A0…, and 0x728160 → 0x7161D0 → 0x7157D0 spawns projectiles.
- In world, 0x7161D0/0x728160 ran 1,253 times and the creator 0x7157D0 ran 0 times.
- The `c_native_weapon_instance` code (0x6FCE80–0x70A0D0) ran 0 times through the launch and spawn.
- The usable-equipment iterators (0x269370/0x269510/0x269550) ran 0 times.
- The gear-manager content request 0x640160 ran 26 times, and `c_gear_manager_component_game_system[3]` (0x63FA80) once.
- Counter tool note: 0x63FA10 and 0x63F9B0 start with a RIP-relative `lea` and can't be hooked. Pre-check with a capstone pass before installing; one refusal aborts the whole install.

**Lifetime phase stepping** (the new `D1_AH_SENSOR_PHASE_FILE`; public `f1fc2a7`), in world on Elimination:
- 4: the screen goes black with the loading glyph.
- 5 and 2: no visible change; still no weapon.
- 7: the post-game "ACTIVITY REWARDS" screen, then `cleanup` and the known teardown crash at +0xBC09AE.
- So 7 is post-game. This agrees with d1preservation's note that the post-game skill write is requested only at game phase 7. No phase produced a weapon.

**Teams.**
- In every run the client logged "Activity configured for teams, no teams found, highest index '-1'" and "Multiplayer Team=-1".
- The client picks teams itself in `setup:matchmaking`: handler `c_world_controller_state_setup_matchmaking_handler[7]` 0x7AA6C0 → 0x7AB270 → 0xB9A040 → 0xB9A140 → `team_picker` 0x418360/0x418C80. These are 01.29's 0xB97FC0/0xB980C0 shifted by +0x2080.
- The lab launch poke writes goal 0x24 (in_world) directly. 0x7AB5D0 reads that as "Activity-session host has finished matchmaking and is telling us to move on" (logged), so the pick never runs. Even the no-teams mode would have written 0, so −1 means it was never called.
- `d1-teams.py` (new) reads and sets the team byte at `membership + player*0xE8 + 0x1D08`. Membership = `[eboot+0x26CA4E0] + i*0x131C8 + [..+8]*0x98E0 + 0x630`.
- Holding team 0 in both sessions from orbit through the Elimination load: the client logged "Multiplayer Team=0" and the composition warnings disappeared. **But the player was never spawned** (`player+0x644` stayed 0xFFFFFFFF, spawn word 1). It then timed out for inactivity and returned to orbit cleanly, without the teardown crash.
- Team 1 set in world, and phases 4/5 with team 1: still no spawn.
- A second launch from the same session (Rumble 406, team 0, then −1): no spawn. That run is confounded: it was a second launch, and the sensor list was for Elimination's world.
- So a team ≥ 0 at load stops the current served spawn path. The spawn we have is the no-team path. Teams are probably needed for a real PvP round, but the team-0 spawn needs something else as well (team spawn zones or a round start); not found yet.
- The team byte is read in game code by 0x40E3D0 (no direct callers; it builds a player→team list via 0x40EAB0), 0x7977A0 and 0x756C80.

**Item loader.** `c_investment_item_loader_game_system` (vtable 0x193EAC0) drives the world-tag loader object at [this+8]: 0x576AE0, 0x5764D0, 0x577120. Its `fill_desired_*` strategies were already shown not to run at spawn.

Next:
1. Count 0x576AE0/0x577120 in world. Find what makes the world-tag loader refill for a player in world (the peer_view strategy), since a weapon can't be created before its item tags are resident.
2. Trace the team-0 spawn refusal at the spawner (0x412800, gate 0x415700) with the existing counters.
