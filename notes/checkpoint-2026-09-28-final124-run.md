# 01.24 PS3 live boot checkpoint — 2026-09-28

This records the first final-build runs on the owner's Windows PC. Raw logs,
screenshots, sign-on requests, game files, keys, and package data remain under
`local/` and are excluded from Git. The source repo contains only the stand-in
changes and this account. The `d1xenia-data` repo has the previously exported
ELF, type and service schemas; no new game binaries are pushed here.

## Setup

- Main repo baseline: `1c46ac8` after fast-forwarding `master` from origin.
- Data repo baseline: `0e0b527` on `main`; corrected 01.24 schema has 5,041
  types, package-derived service map has 94 services.
- PS3 title: BLES01857, exact 01.24 EBOOT and 1,034 required update package
  files (10,305,603,584 bytes) installed in isolated RPCS3 copy
  `local/RPCS3_FINAL124_SMOKE001`. The package inventory and sizes passed
  `d1_final_preflight.py`.
- The isolated copy has both local RPCS3 patches for the 01.24 PPU hash:
  private local CA and unsigned content manifest. Its controller config is
  `Default.keyboard.automation.yml`; `Invoke-RpcsStep.ps1 -Keys X` presses
  Cross without the physical controller. The user's ordinary RPCS3 setup is
  not the test copy.
- Local stack: loopback RPCN, DNS, STUN stub, SSLv3 SignOn/manifest,
  Demonware lobby probe and BAP stand-in. The 01.24 title config uses a
  binary override format, not the 01.00 protobuf. It sets override `0x65`
  (manifest GUID) and `0x66` (manifest URL). Live GET `/manifest` verifies
  those values reach the client. SignOn field 14 reports `127.0.0.1` in
  the later runs.
- The `scripts/Start-FinalBuildSmoke.ps1` launcher now builds a diagnostic
  manifest from the disc plus the latest installed patchable update file
  per English/neutral package ID. This yields 311 entries. Marker-zero
  shared-manifest packages are excluded. It is a local hypothesis for the
  content manifest, not a recovered production manifest.

## Live runs

| Run | Change | Visible result | Recorded technical result |
| --- | --- | --- | --- |
| 001 | Initial 01.24 boot, XInput profile | Taken King title | Controller unavailable for autonomous input. |
| 002 | 01.00-format title config, keyboard profile | RPCS3 crash after Cross | PPU `0x008d32f8`; game string nearby: `Client config decoding failed. Check for invalid overrides.` |
| 003 | Omit SignOn config | `termite` | No config URL to download. |
| 004 | Empty binary config, magic `0x123456`, terminal zero ID | `termite` | Parser accepts binary wrapper; no URL. |
| 005 | 01.24 binary GUID/URL config; disc-only manifest (160 packages) | `marionberry` after Verifying Content | Client fetched `/manifest` (HTTP 200, 7,925 bytes); only 12 package opens, no BAP. |
| 006 | Map new 01.24 STUN names to local stub | `marionberry` | New `00 01 00 01` UDP probe reaches stub; stub gives no response. |
| 007 | Add SignOn response field 14 = `127.0.0.1` | Passed Verifying Content to Preparing, then RPCS3 crash | 192 package-open events; PPU `0x0012a4a4`, read access violation at `0x8`. Disc `ps3_globals_0151_0.pkg` was opened immediately before the crash. No BAP/lobby connection. |
| 008 | Same as 007, replace only manifest package ID `0x0151` with update `_4` | Black loading screen; no crash during capture | Client opened `ps3_globals_0151_4.pkg` from update folder. 179 package opens and 292 `ENOTCONN` log entries during the captured window; no BAP. |
| 009 | Same as 007, manifest covers latest patchable updates and disc-only IDs (311 entries) | Black loading screen; no crash during capture | 325 package opens across 317 unique paths, including update `ps3_client_bootstrap_patchabl_0172_2.pkg` and `ps3_globals_0151_4.pkg`; 165 `ENOTCONN` entries, no BAP. |

The comparison between 007 and 008 strongly implicates manifest package
selection in the crash: the only intended protocol change was the package
entry for `0x0151`, the client used its update file, and the crash vanished
within the observed window. This does **not** prove that `0x0151` was the
sole fault or that our union manifest matches the retail server response.
Both 008 and 009 remained on black loading screens while RPCS3 was still
running; the network retries may be relevant but are not yet a proven cause.
The repeated `ENOTCONN` is specifically RPCS3's `sys_net_infoctl(cmd=6)`
on UDP socket 51, bound to `0.0.0.0:3074` with remote `0.0.0.0:0`.
It starts before SignOn, so its large count alone is **not** evidence that
this is the loading blocker.

The 01.24 client queries four regional `destiny-stun` names and eight
`stunN.signon` names under each of `deadorbit.net` and
`gravityshavings.net`; the local DNS maps these to `127.0.0.2`. It sends
the four-byte `00 01 00 01` probe and later 29-byte `0e 02 ...`
packets from port 3074. The older `stun_stub.py` understands 01.00 request
types 20/30 only, and logs both new packet shapes without answering. The
01.24 STUN response format remains unknown.

## Evidence kept locally

- Per-run raw logs: `local/SIGNON_LOCAL/logs/FINAL124_SMOKE00*-{rpcs3,signon,dns-queries,stun,bap,lobby}.log`.
- Structured indexes with each raw file's byte count and SHA-256, HTTP
  request lines, DNS results, unique STUN datagrams, package-open paths,
  socket-error counts, fatal lines, and screenshot fingerprints:
  `local/RESEARCH/ps3_updates/run-index/FINAL124_SMOKE00*.json` (runs
  001–009; 003/004 lack a preserved RPCS3 log). Runs 005–009 index every
  screenshot taken during that run.
- HTTP request captures (private, may contain account material):
  `local/SIGNON_LOCAL/captures/`; each relevant SignOn log names its capture.
- 007 crash screen: `local/RPCS3_FINAL124_SMOKE001/screenshots/BLES01857/BLES01857_screenshot_2026_09_28_19_25_20.png`.
- 008 black loading screen: same folder, `BLES01857_screenshot_2026_09_28_19_35_51.png`.
- 009 black loading screen: same folder, `BLES01857_screenshot_2026_09_28_19_40_58.png`.
- Diagnostic manifests: `local/RESEARCH/ps3_updates/manifest_0151_override.bin`
  (160 entries) and `manifest_final_union.bin` (311 entries).
- Full package download transcript:
  `local/RESEARCH/ps3_updates/fetch-final124-resume.log`.

`scripts/summarize_final_smoke.py` regenerates a local JSON index after
copying RPCS3's log to the per-run log name. Example:

```powershell
Copy-Item local/RPCS3_FINAL124_SMOKE001/log/RPCS3.log local/SIGNON_LOCAL/logs/MY_RUN-rpcs3.log
python scripts/summarize_final_smoke.py --logs-dir local/SIGNON_LOCAL/logs --run MY_RUN `
  --screenshot local/RPCS3_FINAL124_SMOKE001/screenshots/BLES01857/SCREENSHOT.png `
  --out local/RESEARCH/ps3_updates/run-index/MY_RUN.json
```

## Resume and next checks

The prepared RPCS3 copy and packages are reusable. To repeat run 009:

```powershell
pwsh -NoProfile -File scripts/Start-FinalBuildSmoke.ps1 `
  -DataRepo ../d1xenia-data `
  -Rpcs3Dir local/RPCS3_FINAL124_SMOKE001 `
  -GameUsrdir local/01_WORKING/ps3_destiny/PS3_GAME/USRDIR `
  -Name FINAL124_NEXT -Run
$env:D1_RPCS3=(Resolve-Path local/RPCS3_FINAL124_SMOKE001).Path
pwsh -NoProfile -File scripts/Invoke-RpcsStep.ps1 -Keys X -WaitSeconds 20
```

1. Inspect the repeated `ENOTCONN` source in RPCS3's network trace and the
   01.24 ELF. Determine whether the client is waiting for a STUN reply,
   a BAP connection, a content request, or another local service. Avoid
   assuming all `ENOTCONN` lines explain the black screen.
2. Recover the 01.24 `00 01 00 01` / `0e 02` STUN response format from the
   game's parser or a lawful capture from the owner's console. The older
   01.00 format is different. Record exact request/response bytes and
   validate with one changed variable per run.
3. Check why the 311-entry manifest still causes disc package opens after
   loading the updated bootstrap, and whether the manifest needs additional
   offer or localization fields. Compare the package-open sequence between
   008 and 009 in the saved indexes.
4. Once BAP is reached, use `types_01.24.json` and
   `services_01.24.json` from the data repo, preserve all BAP and activity
   host captures, and index each run before making handler changes.

### Cheapest next run: A/B SignOn field 14

Runs 005-009 differ in the manifest, but 007/008/009 all introduced one more
thing at once: **SignOn response field 14 = `127.0.0.1`**, and that is the run
that first got past "Verifying Content" into "Preparing". It is also the only
new field that feeds the STUN/relay/NAT path, so it is now the leading
suspect for the stall before BAP. Its byte order is unconfirmed, and
`0x132cf4` compares the value against the module's own address, so a
wrong-endian value may be worse than none at all.

The launcher now takes it as a parameter, so the control run is one flag:

```powershell
# control: field 14 omitted entirely (what 005/006 did, and it reached marionberry)
pwsh -NoProfile -File scripts/Start-FinalBuildSmoke.ps1 -DataRepo ../d1xenia-data `
  -Rpcs3Dir local/RPCS3_FINAL124_SMOKE001 -GameUsrdir local/01_WORKING/ps3_destiny/PS3_GAME/USRDIR `
  -PublicIp '' -Name FINAL124_NOFIELD14 -Run

# same manifest, field 14 present again (reproduces 009)
... -PublicIp '127.0.0.1' -Name FINAL124_FIELD14 -Run
```

Keep the manifest identical between the two, and index both runs with
`scripts/summarize_final_smoke.py` before drawing a conclusion. If omitting the
field changes where the client stops, the stall is in the NAT/STUN path, not in
the manifest, and that makes item 1 above the priority.

### Item 2: where the 01.24 STUN client lives (offline search, 2026-09-29)

`scripts/stun_scan.py` searches the code for the immediates that shape the
packets (`--li`, `--word`, `--cmpwi`; the Demonware range defaults to
`0xa00000-0xc00000`). Note the encoding, verified against capstone: in
`cmpwi` the immediate is the **low** 16 bits of the word (`0x2c030004` =
`cmpwi r3, 4`), so matching the high half gives thousands of false hits.

What is known:

- The client's own STUN shim is `0x11e348`, which registers the two handler
  tables at `0x185da68` / `0x185da70` and the module name
  `network_stun_client_demonware_shim` (`0x12d60c4`).
- `0x1329ac` loads the STUN host table: 16 names, `stun1`-`stun8.signon.
  deadorbit.net` and `stun1`-`stun8.signon.gravityshavings.net`, from the
  pointer array at `0x12d6dc0`. Alongside them sit the config keys
  `stun_udp`, `stun_version`, `stun_request_stage`, `client_ipv4`,
  `client_port`, `alternate_stun_ipv4`, `alternate_stun_port` (so the client
  is told its own view of its address and port, not just the server's).
- The four `destiny-stun.{us,eu,jp,au}.demonware.net` names are referenced
  from `0x11e704`; `stun.{us,eu,jp,au}.demonware.net` (`0x14028f8`+) are the
  01.00-era names, still present in 01.24.
- The Demonware transport-startup state machine is `0xb3012c` ("Starting
  external STUN client." at `0xb30230`), but it only orchestrates
  `b32dd4` / `b334e4` / `b2d91c` / `b2f8a4` through vtable slots; the packet
  codec is behind one of those virtuals and has not been identified.

**Not found, and worth not redoing:** the 01.00 request types (30 = `1e`,
20 = `14`) and reply types (31 = `1f`, 21 = `15`) do **not** appear as
`cmpwi` immediates anywhere in the Demonware range of 01.24. The dense
`cmpwi 0x1e` cluster at `0xba33a8`-`0xba3c50` is a generic varint/msgpack
value decoder (it branches on `0x1c`, `0x1d`, `0xf7`, `0xfb` and builds
values byte by byte), not a STUN type switch. So 01.24 does not dispatch on
the request-type byte by comparing it to a small constant: the type is most
likely read into a register (`lbz`) and used as a table index or compared
against a value loaded from `.rodata`. Continue from the vtable slots that
`0xb3012c` calls, or capture a real reply on the owner's console.

Keep raw run data under `local/`; never stage private keys, account data,
decrypted ELFs, packages, or full captures in the code repository.
