# RPCS3 test history

Historical PS3 results; current acceptance is recorded in the dated checkpoints.

## PS3 comparison 001 — Official RPCS3 baseline

- Date: 2026-09-24. Official RPCS3 `v0.0.42-20043-b5a174e9`, Vulkan, PS3 disc game `BLES01857` version `01.00`, with firmware from the user's disc and a private working copy outside Git. No PS3 title update was installed.
- The first boot precompiled 6,650 shader interpreter variants and reached a visually intact "Press X to Play" title screen.
- Player 1 was changed from Keyboard to XInput. RPCS3 detected the user's Xbox controller as `XInput Pad #1`; Xbox A mapped to PlayStation Cross.
- Pressing A displayed Destiny's **canary** message: "No internet connection detected." Gameplay was not reached. The screenshot was supplied by the user; see `notes/ps3-route.md` for the setup and comparison.

## PS3 comparison 002 — Network status connected

- Date: 2026-09-24. Same official RPCS3 installation and PS3 game as comparison 001. Changed only Network Status from `Disconnected` to `Connected`; PSN Status stayed `Disconnected`.
- Pressing A advanced to **buffalo**: "You must be signed into PSN in order to play Destiny." This confirms the first network gate was passed. Gameplay was not reached.
- The current RPCS3 PSN Status menu offers `Disconnected` and `RPCN`. No account setup or private server connection was attempted.

## PS3 comparison 003 — Local RPCN sign-in

- Date: 2026-09-24. Isolated copy of the same official RPCS3 build at `D:\Destiny360\RPCS3_TEST003_LOCAL_RPCN` (baseline install untouched). Network Status `Connected`, PSN Status `RPCN`.
- Sign-in service: RPCN server 1.10.0 (protocol 32, matching RPCS3 `b5a174e9`; 1.11.0 speaks 33 and is incompatible), run from `D:\Destiny360\RPCN_LOCAL` bound to `127.0.0.1` only, email validation and ticket signing disabled. A throwaway account `d1localtest` was created on that local server only with `scripts/rpcn_local_account.py`. No public RPCN account was created.
- First attempt: still **buffalo**, plus an "RPCN unknown error" popup. Cause: RPCN's UDP helper was bound to `[::1]` only, so RPCS3's IPv4 address check on `127.0.0.1:3657` went unanswered. Setting an unparsable `HostIPv6` makes RPCN fall back to `127.0.0.1:3657`.
- Second attempt: **buffalo cleared**. Destiny showed "SIGNING IN", then "Destiny servers are not available. Dismiss this message to try again."
- Log evidence (private: `D:\Destiny360\LOGS\PS3-Test003-local-rpcn.RPCS3.log`): the game called `sceNpManagerRequestTicket2` for service ID `UP0002-BLUS31181_00` and the local RPCN issued each ticket. Its network threads are named `DW Thread` and `_DW HTTP Processor` (Demonware). At boot it looked up `dev-stun.demonware.net`. After each ticket it tried HTTPS (port 443) sign-on hosts in turn: `www8.signon.deadorbit.net`, `www1.signon.gravityshavings.net`, `www1.signon.deadorbit.net`, `www2.signon.gravityshavings.net`, `www2.signon.deadorbit.net`.
- Those names resolved publicly, so these attempts reached Bungie's live sign-on service with a locally issued, unsigned ticket. RPCS3 was stopped once this was noticed. Future local tests should keep the game from contacting live services.
- What this proves: RPCS3 + a local RPCN server satisfies Destiny's PSN sign-in check, and the next gate is Bungie/Demonware sign-on over HTTPS. What remains unknown: the sign-on request and response format, and whether a compatible local service can be built.

## PS3 comparisons 004–013 — Local-only sign-on path

Date: 2026-09-24. All runs used a fresh copy of `D:\Destiny360\RPCS3_TEST003_LOCAL_RPCN` made by `server/prepare_rpcs3_copy.py` (the baseline stays untouched), the same working game copy (`BLES01857` 01.00), and the loopback RPCN server from comparison 003. From 004 on, RPCS3's **DNS address** was `127.0.0.1` and `server/dns_sinkhole.py` answered every name locally, so no run could reach live Bungie/Demonware hosts by name. Tools are described in `server/README.md`; private logs are in each `D:\Destiny360\RPCS3_TESTnnn_*\log` folder and `D:\Destiny360\SIGNON_LOCAL\logs`.

| Test | Change from previous | Result |
| --- | --- | --- |
| 004 `RPCS3_TEST004_SIGNON_PROBE` | DNS sinkhole (all names → 127.0.0.1); sign-on probe on 127.0.0.1:443/80 | New error **marionberry** ("Could not connect to the Destiny servers"). Only `dev-stun.demonware.net` was looked up; no PSN ticket request, no sign-on connection. |
| 005 `RPCS3_TEST005_SIGNON_NETTRACE` | `sys_net`/`DnsHook` trace logging only | Game sends UDP from its port 3074 to the STUN host on 3074: pairs of 3- and 4-byte probes, six attempts ~540 ms apart. With STUN at 127.0.0.1 they loop back to the game's own socket. Also UPnP SSDP to 239.255.255.250:1900. marionberry again. |
| 006 `RPCS3_TEST006_STUN_STUB` | STUN host → 127.0.0.2; `server/stun_stub.py` answers there | **marionberry cleared.** Probes: `1e 03 00` (IP discovery) then `14 02 00 xx` (NAT discovery); stub replies in the Black Ops 4 emulator layout were accepted. Game requested the NP ticket and connected to the probe for `www8.signon.deadorbit.net`, `www1/www2.signon.gravityshavings.net`, `www1/www2.signon.deadorbit.net`. ClientHello: **SSLv3 only**, suites 3DES-SHA, RC4-SHA, RC4-MD5, DES-SHA, export suites, no SNI, no extensions. OpenSSL 3 refused (`VERSION_TOO_LOW`). Screen: "SIGNING IN" → "Destiny servers are not available", fully local. A 29-byte type `0x0e` UDP packet repeats every 15 s to the STUN host (unanswered). |
| 007a/b `RPCS3_TEST007_SSL3_LOCAL_CA` | Pure-Python SSLv3 server; per-host loopback IPs/certs; local CA (CN/O, UTF8String, CRLF) over all 50 firmware roots | Guest access violation at `0x0010723c` ~70 ms after `sceNpManagerGetTicket`, before any sign-on DNS. Reproduced twice. The game reads only `CA01`–`CA05`; the loader stopped after CA01. |
| 008 `RPCS3_TEST008_DNAS_SHAPED_CA` | CA reshaped to C/O/CN with SKI/AKI, only CA01–05 replaced | Same crash. Register dump: `r23 = "ps3_ppu"`, `r24 = "https://www5.signon.gravityshavi…"`, `r29 = 0xd38` — looks like the game's own fatal-error path during sign-on setup. TTY log empty. |
| 009 `RPCS3_TEST009_CA_LF` | LF line endings | Loader still stops after CA01; same crash. |
| 010 `RPCS3_TEST010_CA_PRINTABLE` | PrintableString names (as in the SCEI roots) | Same. |
| 011 `RPCS3_TEST011_CA_SLOT5_ONLY` | Local CA only in CA05; CA01–04 original | CA01–05 all read, but the libssl second pass (CA21 etc., present in 006) never ran; same crash. |
| 012 `RPCS3_TEST012_CA_SLOT1_ONLY` | Local CA only in CA01 | Loader stopped after CA01 again → the loader rejects our root wherever it sits. Same crash. |
| 013 `RPCS3_TEST013_CA_FIXED_DATES` | Validity fixed to 2014-01-01…2037-12-01 | Same stop and crash. |

What this establishes: with DNS, STUN (NAT discovery) and PSN sign-in served locally, Destiny reaches its HTTPS sign-on step without any live service. The sign-on client speaks SSLv3 only, which `server/sslv3.py` implements (self-tested for RC4-SHA and 3DES-SHA). The game trusts only the firmware SCEI DNAS roots CA01–CA05, and the firmware `libssl` certificate loader rejects any replacement file tried so far — after line endings, name shape and string types, and validity dates were matched — so a local CA cannot be injected through `dev_flash`. RPCS3's own HLE `cellSslCertificateLoader` does no validation, but this game uses the LLE `libssl.sprx`. Whether the check is a signature/whitelist test or something else is not yet known.

## PS3 comparison 014 — Game CA patch, firmware roots untouched

- Date: 2026-09-24. Fresh copy `D:\Destiny360\RPCS3_TEST014_GAME_CA_PATCH` of the comparison 003 baseline, made with `server/prepare_rpcs3_copy.py --game-ca`. Same local DNS sinkhole, STUN stub, loopback RPCN and `signon_probe.py --mode terminate --destiny-hosts` as comparison 006, with the SSLv3 server from 007. Firmware `CA01`–`CA05` left original.
- Static analysis (EBOOT decrypted with `rpcs3.exe --decrypt` into `D:\Destiny360\RESEARCH\ps3_decrypt`, outside Git): at `0x106880` the game calls `cellSslCertificateLoader(0x1f)` for CA01–CA05, then `cellHttpsInit(2, …)` with that buffer **and** a PEM string built into the executable at `0x011b7218` ("Tiger Root Authority", 2013–2033, then GeoTrust Global CA). If either step fails, the game takes its fatal path, which explains the crash in 007–013. The SSL callback registered with `cellHttpClientSetSslCallback` (`0x10816c`) reads each certificate's subject name, then returns `verifyErr` with a masked set of flags cleared, so the game itself refuses untrusted chains.
- Change: an RPCS3 patch (`server/game_ca_patch.py`; hash `PPU-10d41d88e5ce442299c2ba4cb681f0b307f33527`) writes the local `ca_v4` PEM plus a NUL over the embedded string in guest memory. The log confirmed `PAT: Applied patch … (<- 2)`. Game files on disk are unchanged.
- Result: **handshake completed.** After the NP ticket request, the game resolved `www7.signon.deadorbit.net` (→ 127.0.1.7). It accepted the local leaf over SSLv3 RSA_RC4_128_SHA and sent `POST /SignOn?platform=ps3_ppu&build=41375.14.07.03.1501.live` (`User-Agent: PS3Application libhttp/4.6.0-000 (CellOS)`, `X-TransactionID: 0`, `Content-Type: application/octet-stream`, 390-byte body). The probe answered 503 and the game made no further sign-on attempt. The screen showed "SIGNING IN", then "Destiny servers are not available. Dismiss this message to try again." (user screenshot). No crash occurred. The raw request is kept privately under `D:\Destiny360\SIGNON_LOCAL\captures`.
- Body shape (protobuf wire format): field 1 = 2; field 2 = 294-byte NP ticket (version 2.1, the local RPCN account `d1localtest`, service `UP0002-BLUS31181_00`); field 3 = `"en"`; field 5 = a 15-character ID string; field 6 = build `41375.14.07.03.1501.live`; field 7 = 0; field 13 = a 64-bit varint; field 15 = packed varints `0xE0000002, 0xE0000001`; field 17 = `"europe_disc"`; field 19 = `"en"`. Field meanings are not yet known.

## PS3 comparison 015 — Local SignOn success reply

- Date: 2026-09-24. Fresh copy `D:\Destiny360\RPCS3_TEST015_SIGNON_SUCCESS` (same as 014, game CA patch applied). The one change: `signon_probe.py --mode signon` answers `POST /SignOn` with HTTP 200 and a success message built by `server/signon_proto.py`. The schema comes from the game's nanopb descriptors (request `0x011ca1c0`, response `0x011ca300`; handler `0x126744`). The reply points the game at `127.1.1.127:12850`, where `server/session_listener.py` logs traffic.
- Input: the user was away, so Player 1 in this copy was switched from XInput to the keyboard handler (Cross = X; original saved as `Default.xinput.yml.bak`), and X was posted to the game window from a script. A Windows Firewall prompt for the new `rpcs3.exe` path appeared and was left unanswered; all traffic was loopback.
- Result: **SignOn accepted on the first attempt.** The game did not contact the session address. Instead it resolved `panther-ps3-auth3.prod.demonware.net` (sinkholed to 127.0.0.1) and sent `POST /auth/` with `Content-Type: application/json`, retrying at 1, 2 and 4 s while the probe answered 503. The screen stayed on "SIGNING IN". The raw requests are in `D:\Destiny360\SIGNON_LOCAL\captures`.
- Next gate: Demonware authentication. This session did not continue past it.

## PS3 comparison 017 — Lobby header acknowledged

- Date: 2026-09-24. Copy `D:\Destiny360\RPCS3_TEST017_LOBBY_HEADER`, services as in 016, plus `server/lobby_probe.py` on `127.0.3.1:3074`, which answers the greeting with `server_header_ack` (protocol 210).
- Result (017b): the client accepted the header and sent a 153-byte `client_auth` frame (`AB 82`). The probe did not reply, and the client disconnected after about 10 s (**cattle**). Captures are in `D:\Destiny360\SIGNON_LOCAL\captures\lobby210`. Static analysis of that frame is in `notes/ps3-route.md`.

## PS3 comparison 018 — Lobby authentication completed

- Date: 2026-09-24. Fresh copy `D:\Destiny360\RPCS3_TEST018_LOBBY_AUTH` of 017. The only change: `lobby_probe.py` derives the `CLIENTCHAL` verify from the auth3 session key and answers `client_auth` with `server_auth_done`. Before the live run, the derivation reproduced the 017b client challenge offline.
- Input: X posted to the game window from a script. The game reached the lobby without further input.
- Result: SignOn and auth3 succeeded as before. In the lobby, the probe logged `client_auth title=5378 challenge_verified=True` and sent `server_auth_done`. RPCS3 shows the client read the full 10-byte frame and began waiting for the next frame. A game job thread closed the socket about 70 ms later, after a `stat` of `/dev_hdd1/downloads/` returned ENOENT. No further DNS lookups or connections followed. The screen showed **termite**: "Failed to download configuration files from Bungie servers, please try again." (RPCS3 screenshot, kept privately under the test folder.)
- Next gate: termite. Leads are listed in `notes/ps3-route.md`.

## PS3 comparisons 019–022 — Title configuration, content manifest, packages

- Date: 2026-09-24. Each run used a fresh copy of 017 (`RPCS3_TEST019_MANIFEST_URL`, `…020_UNSIGNED_MANIFEST`, `…021_MANIFEST_PACKAGES`, `…022_MANIFEST_HASHES`). From 020 on, the copies were made with `prepare_rpcs3_copy.py --game-ca … --unsigned-manifest`. Services were the same as in 018, plus `signon_probe.py --manifest-url http://content.local.test/manifest` (and `--manifest-packages <disc packages folder>` from 021).
- 019: The SignOn configuration supplied the URL. The game sent `GET /manifest`, the probe replied 503, and the screen showed **nightingale**.
- 020: An unsigned, GUID-only manifest was accepted. The screen showed "The game disc cannot be read." The halt text read from guest memory was "failed to load bootstrap_patchable".
- 021: The manifest listed 160 disc packages. The halt text was "Patchable package registration failed with result (-90)".
- 022: Each package entry also carried its header u64. Registration completed, and the game connected to the SignOn session address `127.1.1.127:12850`, sending an 80-byte message with the SignOn token. There was no reply; after 10 s the screen showed **centipede**. Screenshots and logs are kept privately in the test folders and `SIGNON_LOCAL/logs`.
- Details and addresses are in `notes/ps3-route.md`.

## PS3 comparisons 023–026 — BAP session server

- Date: 2026-09-24. Fresh copies of 017 with `--game-ca … --unsigned-manifest` (`RPCS3_TEST023_BAP_SHELLO` … `RPCS3_TEST026_WS_ZERO`). `server/bap_server.py` replaced `session_listener.py` on 127.1.1.127:12850.
- 023: `shello rsp` was accepted, then an encrypted `qz_reg req` arrived (timeout, centipede).
- 024: `qz_reg rsp` was accepted, then `rrc req` arrived.
- 025: `rrc rsp` and `echo rsp` were accepted. `c->ws req` for service 503 went unanswered; after 10 s the screen showed **chicken** ("connection to Destiny character servers lost").
- 026: A zero-filled `c->ws rsp` was received by the client but still ended in chicken.
- Details are in `notes/ps3-route.md`.

## PS3 comparisons 027–027b — Investment sign-in result code

- Date: 2026-09-24. Run with `scripts/Invoke-Rpcs3LocalTest.ps1`, which makes a fresh copy from TEST017 and restarts `bap_server.py` with the given arguments.
- 027b (`--ws-result 5`): the client disconnected immediately after the service-503 reply and showed **chimpanzee** (Bungie account problem). With result 0 (026) the client waited, then showed chicken. The reply is therefore processed; the missing piece comes after sign-in. Details are in `notes/ps3-route.md`.

## PS3 comparisons 028–032 — Investment sign-in, GDB tracing

- 028: after the sign-in reply, pushed empty `self` and `select_character` queuez families. No change (chicken after 10 s).
- 029: GDB trace of milestone/error calls. Chicken is (0x18, 0xa1), raised by the sign-in step's 10 s timeout.
- 030: typed sign-in reply with the account id (`--ws-signin`). No change.
- 031: GDB checkpoints showed the second success predicate (`0x294a5c`, user entry state 3) never passes.
- 032: GDB entry trace of the user-manager range `0x286000`–`0x297000`. Sign-in is started by posting event 0x22.
- Details are in `notes/ps3-route.md`.

## PS3 comparisons 049–062 — Character creation to orbit

- Date: 2026-09-24/25. RPCS3 copy `RPCS3_TEST048_SETUP` (XInput pad, async shaders) reused for every run; logs `SIGNON_LOCAL/logs/TEST05x_*`/`TEST06x_*`.
- 049–054 (Codex): lobby keepalive echo; service 501 (create character) answered with an id; select_character/self families carry the character. Finish still spun. See `notes/checkpoint-2026-09-24-character.md`.
- 054 (idle ~1 h): **caterpillar**. `lobby_probe.py` read at most 97 frames, and 40 s keepalives used them up. The loop is now unbounded.
- 055: GDB showed the 501 reply delegate is the table's no-op; creation succeeded client-side. Killing an attached GDB client froze the emulator: RPCS3's stub serves only the first connection per session. Added `scripts/rpcs3_gdb_daemon.py` (holds the one connection, serves `hits`/`mem` on 127.0.0.1:2346).
- 056: `bap_server.py --store` remembers the created character; the next boot showed **character select** with the Titan (level 0). Pressing it sent service 504 and the screen went black.
- 057–058: the bootflow's character step (`0x32f748`) needs the user entry's active character (+0x20), copied from the self root's u64 at +0x60 (`0x2942fc`). 504 now sets it via a versioned self delta; the step then completed, followed by **centipede**.
- 058b–060: centipede is BAP health condition 15 (connection state != 7). The connection failed with reason 0x12 because replies must answer the oldest pending request (`0x7c3458`), and "acct id trans" (0x17) was unanswered, so the next 701/702 replies matched nothing. (059: leaving 701 unanswered showed "Contacting Destiny Servers" and failed at once, so replies stay on.)
- 061: 0x18 reply (the request echoed plus one u64 account id per name) fixed centipede. The game subscribed to peer (0) and roster (2), called service 104, sent `client config req`, then **pig** (0x18, 0xa2): 104's reply carries the universe object, whose root must be `0x7fffffffffffffff`.
- 062: 104 reply with the universe root plus a `client config rsp` (`{1: 1, 2: 1}`): **the game reached orbit** ("Set Destination", player card d1localtest, level 0).

## PS3 comparisons 063–081 — Server clock, unlock flags, activity join

- 063: universe +8 is the server clock. `--universe-time s` (Unix seconds) turned "trial mode unavailable" into post-launch state (faction ships, Dark Below / House of Wolves news).
- 064–077: account and character unlock flags (self root +0x12a8 table A, +0x16a8+1 table B, character +0x3bc0 table C; tri-state, 2 = true) feed the unlock-expression evaluator (`0xb67480` sources). Bisection: **B0 is the full-game purchase flag**. Story restriction ("temporarily restricted") persisted with all A/B values; C all true skipped it by jumping into a story cutscene and an activity join.
- 078–080: the activity join sends service 105, acct id trans, client config, 104, subscriptions, then **ahp req** (0x10; reply 0x11 is 16 raw bytes, decoder `0x416f14`). Zero reply: unsubscribe, error **currant** after ~0.2 s; patterned reply: 12 s wait, then currant `(0x18, 0x21)` from `0x333050`. No socket connect happened.
- 080–081: at the join the client also sends an encrypted Demonware lobby frame. Envelope (send path `0xa391bc`): `u32le len | AB 85 | u32le seq | IV16 | AES-128-CBC(BDDATA key 0, pt = u32le len + message + pad) | HMAC-SHA1(BDDATA mac key 0, frame[:-8])[:8]`. Captured plaintext (GDB at `0xa396c4`): header `86 8a`, bdMatchMaking (service 0x8a) task 1 (createSession): addresses blob, session params blob with "d1localtest". Unanswered, so the join times out (currant). `server/bdbuffer.py` reads/writes the typed bdByteBuffer.
- Tooling: keyboard pad in the TEST048 copy (user's XInput config saved as `Default.xinput.user.yml.bak`); `Start-LocalStack.ps1`, `Invoke-TitanLogin.ps1` (title to Log In unattended, ~1.5 min), `Invoke-RpcsStep.ps1` (keys + F12 screenshot; PrintWindow captures go stale while the desktop is locked), `Send-RpcsKey.ps1` (real scan codes, holds); GDB daemon `hits ... r5:512` memory dumps.

- 082: lobby_probe decrypts/answers encrypted lobby frames (`LobbyCrypto`). A task reply (type byte 0x01, typed uint64 transaction 0x8000000000000000, uint32 error, uint8 task id, uint32 count, uint32 total) with no results: the client followed up with bdMatchMaking task 3 (deleteSession) of a zero session id and failed the join (currant).
- 083: createSession reply with one 8-byte session-id blob result: the later deleteSession carried **our** id, so the reply format is right. The join now fails differently: **cloudberry** ("Could not connect to the Destiny servers").

- 084–089: the post-ahp wait is the activity-join step; `c->b_ahp` (0x10) is consumed by `0x42a5a0`: 16 bytes = u64 request id (must equal the request's, here 0) | u32 IPv4 | u16 unused | u16 port, stored via `0x1dac14`; then `0x429c64` opens a second BAP connection to that address. (A patterned id was ignored; an all-zero reply gave address 0.0.0.0 and failed at once.)
- 090: ahp reply `0000000000000000 7f010180 0000 3233`: the client connected to a second BAP stand-in (127.1.1.128:12851), shello with field 2 = 2, then sent **`c->ah mgr req`** (0x06, 3103 bytes: activity host manager / host startup request). Unanswered: join fails after ~11 s. Next: the 0x07 reply (decoder `0x415be4`).

- 091–093: second BAP stand-in (activity host proxy, AHP) answers `c->ah mgr req` with **`c->ah mgr rsp`** (0x07): exactly 137 bytes, `02 | u64 host id | char[128]` (decoder `0x415be4`, consumer `0x42a514` -> `0x31181c` strncpy 0x80). With host id 1 and a host string, the client next sends **`c->ah not`** (0x08) via the proxy: `u64 host id | 01 | u32 3 | u32 0xe8 | 232 bytes` (probably the host startup request), then fails with **(0x18, 0x2a)** after ~7 s (previously 0x21). Next: the activity host's replies (host startup response, host identifier, join) through the proxy.

- 094–098: activity host join. A `b->c not` (0x09) pushed on the proxy connection is routed `0x305484 -> 0x42a498 -> 0x258778`, which dispatches on the subtype (table at `0x175cf5c`); host messages are typed structs in the investment bit format (`0x25bd0c` / `0x25bc88`, client->host types at `*(0x179eba4)`, host->client at `*(0x179eba8)`; schema exported to `SIGNON_LOCAL\activity_schema.json`). `c->ah not` is `u64 host id | 01 | u32 type | u32 length | u32 0 | body` (the u32 0 was first mistaken for part of the body). The host join request (type 3) is a `0x80800e98` struct: i32 random token, i64, ids (account and character), player info (u4, `char[128]` e.g. `Fri_09252026_112315_000001@ps3@User@RPCS3-110`, …); the client keeps a copy (`0x259c84`). The **join response** is type 4 (`0xd0d560 -> 0x256ad8`), a `0x80800a58` struct: the request echoed (the token must match), result 0 (3 bits, min 1), `char[128]` session id, then ints. With it the join passed (`0x256dc0`, flags 0x10|0x20): the client then sends types 17 (8 bytes), 37 (1087 bytes) and type 15 every ~33 ms, and fails with **(0x18, 0x2b)**. Failure codes are step index + 0x18 in the table at `0x175eafc`: 0x2a = timed out waiting for join response, 0x2b = **timed out waiting for entity indices**. Entity indices arrive as host type 0 (`0xd0d020`): a `0x80800d64` struct holding a 384-word bitmap ORed into the client's index set (+0x5bf8). `bap_server.py --ah-join [--ah-entities FIRST:LAST]`; `Invoke-JoinCycle.ps1 -AhHost NAME`.
- 099–101: after the join response. **Entity indices** (host type 0, `--ah-entities 8192:12288`) moved the failure to 0x2c (timed out waiting to establish client); `0x25831c` = indices received (+0x61fa). **Established** (`0x258324`) also needs host flag 0x100 (host type 11, body ignored, `0x25715c`) and an activity state: host type 1 (`0xd0d250`), a `0x80800e76` struct whose 0xa20-byte body goes to the simulation (`0x1c95f0`, +0x50b0, present flag +0x50a8) with bool @0 set and bool @1 clear or i32 @0x10 > 0. With `--ah-state` (type 11 + minimal state) the client was established and the join moved on to **loading activity resources**: failure (0x18, 0x36) from `0x336610` (0x36 - 0x18 = 0x1e, `error.loading_activity_resources`), error code **lime**. Meanwhile the client sends host types 17 (8 bytes), 37 (1087 bytes), 21 (172 bytes, player string), 15 (1 byte, ~30/s) and 14 (16 bytes). Next: the activity state fields (@8 u64, @0x60 `0x80800cc5`) the resource loader needs.
- 102–111: activity loading. The descriptor in the activity state (@0x60 @0x994, char[40]) is a **scenario name** looked up by prefix in the resource registry (`0xe72f4`, names in guest memory such as `orbit:scenario_client`, `.map:orbit:root`, `city_tower_default1:scenario_client`, `act_cosmo_m0_highway:scenario_client`, `act_cosmo_m1_dome_a…`, `act_cinematic_queen_101…`). World id: `--ah-world 125` (the i16 in the client's service 105 request) passes the DLC check (`0x4105c4`, "World-transition requires DLC."). The loader (`0x3362c0`, state at +0x30) starts loading the scenario (state 0, `0x1f4a54`), waits for it (state 1, `0x1f4864`), asks `0x30c448` whether to abort (state 3 = fall back to orbit resources, status 0x1f), otherwise computes the activity index (`0x30b698`, global override `*(0x175ea34)`, default 1) and posts **event 0x36** `{index, scenario name, 3 bytes}` (`0x30bdd0`); the world controller's RPC 0 object (0x18ea574, registered for 0x36 at `0x30c54c`) receives it (`0xd24e14` -> `0x30d488`) and stores the index at `*(0x18ea60c)+0x94`, which state 2 needs. RPCs 1/2/3 use events 0x35/0x33/0x34 (event dispatcher `0x262a80`, table `*(0x18cd744)`, poster `0x262bc4`). The loader skips the post when the requested scenario equals the loaded one (case-insensitive `0xba738`), so `--ah-map orbit` never sets the index; `act_cosmo_m0_highway` also ended in the orbit fallback (under investigation). RPCS3 GDB breakpoints only fire at function/block entries.
- 112–158: see `notes/checkpoint-2026-09-25-orbit.md` (activity selection, unlock and permission
  checks, checksum/bubble count, lobster). 158b: with the activity state's zone record set to -1
  (`--ah-zone -1:0x811c9dc5`), the client stays in the activity ~72 s, then **baboon**.
- 159–162: baboon = activity-host status 36 `private_activity_host_host_crashed` (no host message
  within the timeout). `--ah-heartbeat 1` (type 11 every second) removes it: 162 stayed in the
  activity 6+ minutes, on the loading screen (no host simulation stream yet).
- 163: `--ah-zone 105:0x811c9dc5` (bubble 13 sub-slot 1): the world controller loads that zone
  set, loading tasks 0-7 complete; tasks 8-10 wait for the host simulation's zone record.
- 164: + `--ah-world-state 105:1` (type-11 world-state delta): zone set 105 active, loading tasks
  0-8 complete; task 9 still running.
- 165–166: `--ah-roster 0` (type-5 simulation packet with the client in roster slot 0, resent
  with the heartbeat): the client adds the peer; no local player yet (peer table empty).
- 167–176: local player binding traced (machine table set at the world change, cleared by RPC 16
  from an empty group entry; host machine list; RPCS3 GDB has no watchpoints). 174: host type 24
  (per-host index grant) sent at the join had no visible effect. 176: host type 43 naming the
  client = boot list -> error **bass**.
- 177–179: listeners never fire from boot; player creation is event 0xc from the RPC 16 sender,
  gated on the manager's expected-player list (empty) - see the checkpoint note.
- 180: world-state member mask (+0x5028) -> RPC 16 carries our machine, player join event 0xc,
  player pool 1, local user bound. Task 9 now waits for the player's object and a kind-0x10 lookup.
- 181–182: all bubble states 0 / bubble 0's zone set active: no change; task 9 waits for the
  game-mode object's slot byte and the player's world object (not spawned).
- 183: traces: the spawner runs for our player every frame and `0x35793c` returns 0 - the
  player has no simulation player (pla +0x50); the 16 kind-0xc simulation-player entities have
  owner id 0. See the checkpoint note.
- 184: `--ah-sim-player` (owner = peer id, wrong: the binder compares pla +0x18, the player id):
  the object is received into the record buffer but nothing is applied (other records still
  await a baseline).
- 185: `--ah-baselines` for all 22 replicated objects + owner 0x0a3d4a2c8fbea6ba: states
  applied, the local player is bound to simulation player 5, spawn event 0x4c posted; task 9
  and the spawn check wait on the game-mode state (0).
- 186: + `--ah-game-mode 0x4786c0e0:3:2`: **all loading tasks done, loading screen gone** (black
  screen, no unit). Spawn check now fails on the active zone set (105 is not a bubble base).
- 187–188: `--ah-zone 104` / `0` without bubble states: **lobster** (base entries absent).
- 189: `--ah-bubble-states -1,13=0` + zone 104: zone set 104 active, 121 replication records,
  99 without a baseline -> nothing applied.
- 190: `--ah-baselines @FILE` (all 121, dumped with `scripts/rpcs3_replicated_objects.py`):
  loading done, sim player bound; spawn check refuses game-mode state 2.
- 191: game-mode state 3: readiness passes, spawn event state 1; the spawner waits for the sim
  player's entity +0x154 byte (grant).
- 192: grant attempt on the wrong field (real32 +0x10): no change.
- 193: `--ah-sim-player ...:1` (state bool +0x48 = entity +0x18c): the spawner runs the spawn
  body, finds a spawn point, and fails creating the player object: blueprint handle (player
  +0x1428) is -1 because the peer family (0) is empty.
- 194: `--peer-family` (family 0 = account + character summaries): blueprint set, player unit
  created - **the guardian spawns in the mission** (HUD radar, world rendered; "Urgent message"
  dialog and "Level Up" toast on top).
- 195: game config: tiling + write color buffers: same black blocks / exposure drift.
- 196: + read color/depth, write depth: correct lighting at first, black block back after ~1 min.
- 198: narrower self flags `1;2@A0-1023,B0,C0-1023` led to banana (required content) before the
  mission. The F12 screenshot is authoritative; PrintWindow showed a stale shader screen.
- 199: Force CPU Blit enabled with the TEST196 buffer options; RPCS3 crashed at character
  selection (Windows access violation `0xc0000005`, offset `0x3a56cb8`) before entering the
  mission. Setting restored. See the checkpoint for paths and external RSX issue references.
- 200: Allow Host GPU Labels enabled as the single graphics change. Mission reached; lighting
  temporarily even while stationary, then split dark-left/overexposed-right after movement
  (F12 14:12:11, persisting at 14:13:01). Level Up prompt appeared over the broken frame.
  Per-game setting restored to false. Next: obtain an RSX frame capture in a GUI-enabled run.
- 201: Strict Rendering Mode enabled as the single graphics change. Mission reached but the
  first F12 image already showed a purple band and dark blocks. Setting restored. An RPCS3 RSX
  capture was saved at `D:\Destiny360\RPCS3_TEST048_SETUP\captures\BLES01857_20260926143340_capture.rrc.gz`.
- 202: unlock-expression trace after the join: no evaluations; urgent message still shown.
- 203: OpenGL: RSX fatal before sign-in.
- 204: + RSX reservation accuracy + no async memory manager: movement works; split after
  movement, recovers when still - no fix.
- 205: strict texture flushing: purple/white band, frozen image - worse.
- 206: accurate SPU xfloat: same split (vertical at x=800 after moving).
- 207: + accurate SPU DMA + cache line stores: short moves clean, sustained movement brings the
  same blocks back; recovers when still.
- 208: ZCull occlusion queries disabled: broken after movement and no recovery - worse.
- 209: aliasing bias Prefer Color: broken after movement, no recovery.
- 210: vertex cache off: RSX hung before the mission. 211 (retry): mixed frames, still broken.
- 212: tiling off (read/write buffers on): same pattern.
- 213: RSX FIFO accuracy PS3: no change. Settings search closed (see checkpoint).
- 214: Force CPU Blit: 24.7 GB, never signed in - not usable.
- 215: diagnostic RPCS3 build: G-buffer copies are RSX blits into main memory read (and
  written) by SPU jobs; see checkpoint.
- 216: VRAM->main blits on the CPU path: normals copy complete in memory, frame still breaks.
- 217: texture-upload diagnostics: inputs and texture reuse look consistent; blocks remain.
- 218: patched build + FIFO PS3 + RSX reservation accuracy: still broken.
- 219-221: timeline traces (see checkpoint): SPU-built RSX command lists, written before use.
- 222: Disable FIFO Reordering: RSX hung before the mission. 223 (retry): blocks after
  movement - no fix.
- 224: render-graph trace (`d1trace_timeline.py --graph`): per-frame pass order recorded (see
  checkpoint); the trace slowed the game so much that all frames came out clean.
- 225: one-frame render-target dumps (`C:/dev/d1dump.on`, `scripts/d1dump_scan.py`): the black
  blocks are already in the HDR light buffer 0xCEB60000 after its first pass, and match exactly
  the HUD targets that alias that memory (0xCEB60000 1280x127 and 0xCEDD0000 1280x228 ARGB8,
  pitch 8192). Surface aliasing in RPCS3's surface cache, not SPU data.
- 226: draw/clear/inheritance trace of the light buffer: each frame RPCS3 rebuilds 0xCEB60000
  from cached pieces (HUD target 0xCEDD0000, split-off pieces of the old light buffer, the
  1280x127 HUD target); the block gets the HUD's cleared (zero) data.
- 227: blit trace: the HUD stage blits the top 127 rows of the frame into 0xCEB60000 as ARGB8;
  RPCS3 writes that into the FP16 light-buffer surface (fresh tag). Guest memory under the
  block holds real lighting values (read live) that RPCS3's surface lacks: the light buffer is
  written outside the RSX (SPU lighting), and RPCS3's 3-point memory check misses it.
- 228: surface memory check with 33 samples instead of 3: first movement pass clean (a thin
  noise stripe remained), blocks back for three frames in the second pass - better, not fixed.
- 229: **fix**: per-4KB-page write generations for RSX local memory, bumped by SPU DMA puts;
  a surface whose pages changed since its last sync fails its memory check and is reloaded
  from guest memory (3 samples again). Two full movement passes: 14/14 frames clean - no
  blocks, no overexposure, no stripe.
- 230: urgent-message investigation run (BAP codec table read live, see checkpoint).
- 231: `--client-config 0,1` (field 1 = 0 instead of 1): urgent message still shown.
- 232: GDB trace of the unlock evaluator 0xb67984 from the end of login: 0 hits - but see 234/235.
- 233: heartbeat without the empty type-11 push (`--ah-heartbeat 1:-1`): urgent message still
  shown (and no baboon: the machines/simulation repeats keep the host alive).
- 234/235: GDB traces get 0 hits even on functions that run constantly, with the patched and
  the official build: RPCS3 ignores software breakpoints under the LLVM PPU decoder
  (`ppu_breakpoint` returns false), so every trace since at least TEST202 was blind. 235
  (official build) also showed the old rendering fault, as expected.
- 236: first build with the PPU call hook: RPCS3 exited during boot (the hook table was a global
  in the executable, referenced from JIT code). 237: table moved to a fixed reserved address -
  hook works (`D1P` lines with registers, LR and return chain).
- 237: the universe-gated message list (0x40acc0) runs once at login: 17 entries, all testing
  universe flags 1858-1870 (== 2), all 0 - nothing collected; not the dialog's source.
- 238: first-call coverage (`coverage` in the watch file) plus `scripts/rpcs3_text_marker.py`:
  ~202k functions; 17.8k first called in the 5 s before the dialog text appeared (world load).
- 239: the 8680 functions first called in those 3 s armed for 3 hits with registers: none
  received the dialog's text object; the text is written by generic helpers used earlier.
  Candidates noted in the checkpoint; hunt paused (the dialog only costs one X press).
- 240: soak test with the fixed build (`scripts/Invoke-SoakTest.ps1`): frames clean (a dark-scene
  false positive led to an edge test in `frame_blocks.py`), but RPCS3's working set grew
  ~0.6 GB/min and the process vanished 12:24 into the game (6 min into the soak) with no fatal in
  its log and no Windows error event; private commit was ~75 GB against a 63 GB commit limit, so
  most likely an allocation failure. The stock build's RSX fatal ~12 min in (TEST195,
  `simple_array::reserve` verification) fits the same leak. ~72 texture-cache "does not match
  stored parameters" warnings per second (0xCF8B7600 32x78 <-> 64x39, 0xCF8C6000, 0xCEA18000).
  241: same run on the official build to see whether the leak predates the patches.
- 241: official RPCS3 build, same mission: private commit +14-15 GB/min (24.9 -> 106.7 GB in
  6 min), process gone ~7 min in. The leak predates the patches.
- 242: VRAM allocation limit 8192 MB: same growth (the budget is not the issue). Config restored.
- 243: `D1MEM` log (RPCS3's own Vulkan pools, texture/surface cache): all flat at ~500 MB total.
- 244: address-space diff: ~7 new 16.47 MB committed private regions per second, all segments of
  one NT heap (signature ffeeffee), ~1950 segments / 32 GB; mostly free blocks between live
  blocks of ~0x60600 bytes.
- 245: counters: 185,570 VK programs created at boot (none destroyed); in the mission ~270
  descriptor sets per second get their first `init()`, each reserving 16448-entry image/buffer
  info pools (~0.9 MB) -> ~14 GB/min. Fix in 246: pools of 256 + 64 entries, flushed at bind
  once 256 are used (~18 KB per set).
- 246: descriptor storage pools 256 + 64: growth down from ~14.5 to ~1.5 GB/min (10.2 GB at the
  mission start instead of ~24 GB); movement frames ok (the last ones very dark: facing a wall
  after eight W presses). Remaining growth: ~24 new 32 MiB regions per minute (arena-like records
  with heap pointers, most likely the driver's per-table descriptor pools, 16 sets minimum each)
  plus a few heap segments. 247: per-table pools start at 2 sets (autoscaling unchanged) - soak.
- 247: + per-table descriptor pools starting at 2 sets: 30-minute soak - 13 steps, all frames ok,
  private +1.2 GB/min, then the RSX thread stopped presenting 20:16 into the game (system RAM
  full at 30.7/31.2 GB, right after compiling another program); the process stayed alive (PPU
  running) for 2 h but F12 no longer saved. Root of the remaining growth: during gameplay ~6000
  programs/min are added - only 4 distinct vertex programs but ~2700 new fragment programs per
  minute (ids keep rising; 254k programs total). The fragment program cache keeps missing, so
  something that changes per frame (probably SPU-patched constant data) ends up in the FP hash.
  248: log the ucode of new FPs to compare near-duplicates.
- 248: warm pipeline cache: no new programs in the mission (the TEST247 churn was a cold cache),
  but ~15k descriptor tables per minute still committed for the first time (~1.4 GB/min).
- 249: + idle descriptor-table release (tables unused for 256 frames give back pool and set
  storage): 30-minute soak, 30/30 steps ok, RPCS3 alive throughout; private 12.9 GB flat for the
  first 12 minutes (live tables ~2600, ~260k released), then +0.2 GB/min while new areas compile
  new pipelines (programs 255k -> 288k), ending ~16 GB.
- 250: TEST249 build with `C:/dev/d1noblitfix.on` (VRAM->main blits back on the GPU path):
  does the write tracking alone keep the frame clean?
  Result: two full movement passes, 14/14 frames clean. With the local-memory write tracking the
  CPU path for VRAM->main blits (TEST216) is not needed; it was dropped from
  `patches/rpcs3-destiny-fix.patch` (the diagnostic build keeps it behind the switch).
- 251: PPU hook on 0x401048/0x40130c (object 0x18d9500: state 6, hash 0x6f7125cb), 0x5531bc,
  0x553154, 0x5524f0: all run every frame from `0x286cec`/`0x286da4` (a per-frame UI/loading
  manager), not the urgent-message trigger. Hunt paused.
- 252: second soak (pipeline cache now ~356k programs, preloaded at boot): 19/19 steps ok,
  private 15.4 -> 16.0 GB over 18 minutes. Pad config switched with `Set-PadConfig.ps1` and
  restored to the user's XInput pad afterwards; RPCS3 and the stand-ins stopped.
- 253: reconnaissance (`scripts/Invoke-Explore.ps1`: look around, Select, walk/turn stretches):
  the guardian walks around a lit interior; Select opens the Ghost's NAV MODE with location
  "DOCK 13" and "Leave Game", no waypoint or objective; no enemies anywhere. The client only
  sends type 15 (1 byte, ~30/s) plus a few setup messages to the host (3, 6, 17, 18, 20, 21, 36,
  37); its own movement is not sent. The 99 bubble-13 objects (groups 0x062dd95d, 0x3c2f5ed5,
  0x992bb471; kinds 1/3/4/5/6/0x1d/0x20/0x21) sit at the stand-in's empty default baselines, so
  whatever they are (spawners, combatants, doors, objectives) nothing drives them.
- 254: diagnostic `--ah-world-state 104:1:1` (group peer 1 instead of 0): **no mission
  result**. RPCS3 first opened a welcome prompt behind an NVIDIA Overlay crash dialog;
  after clearing both, sign-in succeeded but the unattended run's F12 screenshot timed
  out. The game remained at Set Screen Bounds. Do not treat this as a peer comparison.
  Static analysis below shows the existing group peer 0 is already the local slot.
- 255: TEST253 configuration plus Codex's new captures in Dock 13
  (`rpcs3_replicated_objects.py --json-out/--schema-out`, private files
  `D:\Destiny360\SIGNON_LOCAL\TEST255-objects.json` / `TEST255-object-types.json`): all 121
  records have baselines (awaiting 0). `rpcs3_world_peer.py` read zone -1, group peer -1,
  local slot 0 at the slice it reads (not zone 104 - the slice pointer or record offset
  needs checking). State types by kind: see checkpoint.
- 256: first-call coverage from boot: the per-component "apply replicated state" handlers are the
  first-call entries with LR 0x276230 (after the member-function call in `0x276154`): 14 handlers.
- 257: hooks on the apply path (`0x25f6b8(binding, state type, ...)`) and the 14 handlers: each
  handler's second argument points at the state type handle, giving the kind -> handler map
  (checkpoint).
- 258: hooks on the script-variable getters (`0x1fed60`, `0x1fee1c`, `0x1fedc4`, `0x1fed8c`; the
  variable API at 0xd07fd0-0xd08dec is only reached through these wrappers): the budgets were used
  up by per-frame queries - 0x9270fc16 (caller 0x34e5ec, spawner area), 0xf50d3fbe (0xb26b44, game
  mode), 0xbe4db480 (0x3547e8, spawner), 0xe00081c9 (0x456cc0) - plus a few one-off hashes
  (0x6999ecc1 from 0x403410, 0x8a9c2494, 0x1b6934 x7). Needs a "log each value once" hook mode.
- 259: bubble 0 instead of bubble 13 (`-ExtraAh --ah-bubble-states -1,0=0 --ah-zone 0:0x811c9dc5
  --ah-world-state 0:1`, same z104 baselines): the guardian spawns. 99 replication records, all
  with baselines (private `TEST259-objects.json`): the shared groups 0x992bb471, 0x3c2f5ed5 and
  0x4786c0e0 only - bubble 0 brings no bubble-local group (Dock 13 has 0x062dd95d).
- 260: TEST259 configuration plus exploration and "log each value once" (`u`) hooks on the
  script-variable getters. The Ghost's nav mode names the place **SKYWATCH**: night, aurora,
  snow, fences and sheds - not the mission start (daytime, cars in front of the Wall, "ENTER THE
  WALL"). The urgent-message popup and a "Level Up" toast appeared again. Variables queried
  (hash, caller): 0x1fed60 - 0x9270fc16 (0x34e5ec), 0x7005ebae (0x35a174/0x752e78/0x753288),
  0x4165bc21, 0x6841ced1, 0x0150f05d, 0x8edb7194, 0x1d7c72c6, 0x87b08641/0x9580f1ab/0x984b3e47/
  0xcb43928b (0x4e96d8), 0x6d94be13, 0x5363ac88/0xae40c103/0x4b565fd5 (0x64d*-0x64f*),
  0x0e8fba43; 0x1fee1c - seven from 0x1b6934 (0x9cbb1bca, 0x902f962c, 0x804595e8, 0x6cc5f20a,
  0x9da2485c, 0xae0f714c, 0xb0491511), 0x648200df/0x4f87f291 (0x2a2e98), 0xd500a5ba,
  0x6999ecc1/0x6ac978e9/0x1dd4fff4/0xdab1fe13 (0x403410-0x403468), 0x8a9c2494 (0x5e96d0-0x5e98ac),
  0xf50d3fbe (0xb26b44, game mode), 0xa8f8e198 (0xb49a88); 0x1fedc4 - 0x5055b670 (0x3547e8),
  0x46e1e45d/0xd2c94ab2 (0x35b014/0x35b02c), 0x8cad54c6; 0x1fed8c - 0xe00081c9 (0x456cc0).
- 261-262: bubble 0 again with `scripts/rpcs3_bubbles.py`: the object table's 22 bubble zone
  handles are 0x80cda00f (bubble 0), 0x80cda2df, 0x80cda3ea, 0x80cda725, 0x80cdb034, 0x80cdb536,
  0x80cdb5aa, 0x80cdba2a, 0x80ce01c8, 0x80ce06c7, 0x80ce0beb, 0x80ce11a9, 0x80ce1c7b, 0x80ce225e
  (13, Dock 13), 0x80ce2b61, 0x80ce3761, 0x80ce3f75, 0x80ce4154, 0x80ce450d, 0x80ce48d5,
  0x80ce5324, 0x80ce8511; only the loaded bubble's handle resolves. Nav mode confirms SKYWATCH
  (west of the Mothyards, not the start). Guest memory holds the mission's script object names
  (see checkpoint): phases `cosmo_m0_phase00_highway_intro` (cinematics `pf_104_res`,
  `pf_104_res_fp`), `phase00_highway`, `phase01_breach`, `phase01b_breach`, `phase02_graveyard`,
  `phase03_hangar`, `phase04_outro`. Entity group 0x992bb471 = FNV-1("act_cosmo_m0_highway").
  Plain area names ("Skywatch") are not in guest memory as text.
- Setup-screen flashing (user video): magenta/white blocks over the lower ~40% of Set Screen
  Bounds / Brightness / Subtitles on alternate frames. In the F12 archive every Set Screen Bounds
  capture before 2026-09-26 13:59 is clean; the first bad one follows TEST195/196 (Write/Read
  Color + Depth buffers, tiling), before any custom build. Cosmetic, back burner.
- 263-264: first bubble sweep (`scripts/Invoke-BubbleSweep.ps1`) while the user used the PC: F12
  screenshots failed - F12 is a Qt window shortcut and only fires while the game window is
  active. The instrumented build now takes a screenshot when `d1\d1shot.on` (next to rpcs3.exe)
  appears; `Invoke-RpcsStep.ps1` uses it. The private data moved from `D:\Destiny360` into the
  repo's ignored `local\` folder the same afternoon.
- 265-270: bubble sweep from `local\`, with `--ah-baselines-watch` feeding each bubble's records
  back to the stand-in during the run (TEST267: 150 new baselines picked up, spawn succeeded).
  Nav-mode names: bubble 1 none (sky and distant mountains, probably a fallback position),
  2 - (TEST266: RPCS3 livelocked while loading after character select, 8 threads at 100 %, no
  frames), **3 THE BREACH** (dark; own group 0xfc0e45a3: 30 kind 1, 45 kind 2, 1 kind 3, 20
  kind 4, plus activity-group kinds 4/0x1d/0x1e in bubble 3), 4 - (TEST268: RSX fatal
  "Verification failed (object: 0x0)" after decoding a garbage fragment program while the
  bubble loaded), 5 none (industrial corridor interior), **6 MOTHYARDS** (daytime). Bubble 0 is
  Skywatch and 13 Dock 13, so the highway start outside the Wall is most likely bubble 2.
- 271-276: retries. **Bubble 2 = THE BLAST** (TEST273; daytime industrial sheds, no bubble-local
  group). Bubble 4 crashes RPCS3 every time it loads (TEST268/274/276): the RSX thread decodes a
  garbage fragment program ("Invalid Src type", "Unknown/illegal instruction") and about a
  second later dies with "Verification failed (object: 0x0)". Separately, 5 of 12 boots since
  the 15:01 rebuild and the move to local\ froze in the login cinematic (TEST266/271/272/275,
  plus TEST263-era runs not counted): the loader opens ps3_ui_menus_0018_0.pkg and never reaches
  ps3_ui_orbit_0026_0.pkg (good runs open it immediately), 8 emulator threads spin at 100 % and
  frames stop 40-180 s later. The 6 boots that morning (older build, game files on D:) never
  froze. `Invoke-JoinCycle.ps1` now aborts when the BAP log is silent for 60 s (the client
  echoes every 5 s) and `Invoke-BubbleSweep.ps1` retries up to 3 times. Bubbles 7-12 and 14-21
  went to the second PC (MATILDASPC: Ryzen 7 5700X, 32 GB, RTX 2060 SUPER; project copy in
  F:\d1\d1xenia), TEST300 onwards.
- 300-340 (second PC): void - 300-319 Windows PowerShell 5.1 (no [Convert]::ToHexString; now
  pwsh 7), 320 froze the desktop while RPCS3 rebuilt the copied 445k-pipeline cache for the RTX
  2060 (cache set aside, 4 shader threads), 330 lost the character-select input on the slower
  PC (login now waits for the BETA BUILD watermark), 340 read "21,14" as bubble 2114 (lobster,
  back to the intro video; list parsing fixed).
- 341 (second PC, 18:09 build): **bubble 21 = THE DEVILS' LAIR** (dim industrial interior with
  stairs; no bubble-local group). 12 D1FPRELOAD events: the bound fragment program is rewritten
  in place by SPU DMA during normal play, and the build re-reads it.
- 342 (second PC): **bubble 14 (zone set 112) = the mission start** - the highway outside the
  Cosmodrome Wall: daytime, rusted car wrecks along the road, the Wall with its camouflage panels
  and water tower (nav name "COSMODROME"). This is the opening area of A Guardian Rises. Its
  objects: bubble-local group 0xecf574b5 (kinds 1, 2, 28x 4, 4x 0x1d, 0x28) plus activity-group
  (0x992bb471) kind 0x1c and 4x 0x1d in bubble 14 - no kind-3 objective sensor here. 11
  D1FPRELOAD events. Bubble 14 is one of the two bubbles with an extra slice set (113), which
  fits 113 = the intro cinematic.
- 350 (second PC): boot into slot 113 (bubble 14 state 0), then `--ah-zone-schedule 112@90`. The
  screen stayed black behind the URGENT MESSAGE dialog for 3 minutes; no cinematic, no spawn.
  Slot 113 is its own slice set: the client's records were only the global ones plus
  activity-group **kind 4 index 0 in bubble 14** (0x808007d9 / B 0x80800387) and **kind 6 index 1
  and 2** (0x8080254d) - bubble 14's gameplay group 0xecf574b5 was not loaded. The second world
  state (zone 112 at +90 s) was sent but not acted on (no new records, no spawn). Reading: the
  cinematic slice set loaded, but the host never started it: kind 6 holds up to 16 + 6 player ids
  (roster; EBOOT string "cinematic_with_roster"), kind 4's handler `0x2000e0` advances the object
  when the u32 at +0 rises above its value at object +0x110 (`0x200014`), bool +4 gates the
  follow-up. Both were default zeros. Next: TEST360 with the player id in kind 6's roster and
  kind 4 stepped 1, 2, 3 at 20/40/60 s (`--ah-object-at`), OK pressed on the urgent message.
- 360 (second PC): slot 113 again with the player id in kind 6 index 1's roster and kind 4
  stepped 1/2/3 at 20/40/60 s: black screen with a turning loading glyph for 150 s, no
  cinematic. Cause (test design): slot 113's kind 6 index 2 had no baseline (the Dock 13 file only
  lists index 10) and the live feed was not running, so no replicated state applied at all
  (0x1ce1b8). The steps also went out while the client was still loading. New
  `scripts/Invoke-ObjectExperiment.ps1` always feeds baselines, dismisses the urgent message and
  takes timed screenshots; TEST361 repeats with steps at 90/120/150 s.
- 351 (second PC): **bubble 7 = THE STEPPES** (daytime scrubland under the colony ship; no
  bubble-local group; 20 D1FPRELOAD).
- 361 (second PC, Invoke-ObjectExperiment.ps1, slot 113, roster + steps at 90/120/150 s): the
  live feed supplied the 2 missing baselines, the urgent message was dismissed, the screen stayed
  black with the Level Up toast; at +132 s "Removed from the game world ... baboon". Cause: a
  stand-in bug - the --ah-object-at scheduler reused `seconds`, the heartbeat interval variable, so
  after step 1 the next heartbeat came 90 s later and the client declared the host gone (the
  client kept sending type-15 notifications, ~30/s, body 00 with an occasional 0x41, throughout).
  Fixed; the cinematic test repeats as TEST363. The end-of-run export failed because the client
  had already left the world.
- 363 (second PC, fixed stand-in): slot 113 with roster and kind-4 values 1/2/3: heartbeats
  held (233 over 4 min, no baboon), every record awaiting 0, but the screen stayed black (Level Up
  toast, no HUD) - no cinematic. Static re-reading of kind 4: `0x200014` compares the new u32
  with the counter of the entity the object tracks (+0x2e4 via `0x1feff0`) and, if higher,
  destroys that entity (`0x1323e0`) and clears the handles so it is recreated: kind 4 is a
  **spawn slot with a generation counter**, not a play trigger (28 in the highway start, 20 in
  The Breach, 5 in Dock 13). Next: first-call coverage in 112 (TEST364) vs 113 (TEST365) to see
  how far the cinematic code gets.
- 364 (second PC): first-call coverage in bubble 14 (zone 112, guardian spawned): 16 first calls
  with lr=0x276230 (the replicated-state apply site). New against TEST256 (Dock 13):
  `0x26c414`, `0xb2f2e0`, `0x26ff1c` - the handlers of bubble 14's kinds 2, 0x1c and 0x28.
  `0xb2f2e0` sits next to kind 0x1d's `0xb2f99c` (mission-script code) and just copies the
  12-byte state {object id, u32} to the component at +0x108, so it is kind 0x1c; the other two
  are kinds 2 and 0x28 (pairing by the r4 state-type dump still to do).
- 352 (second PC): **bubble 8 = LUNAR COMPLEX** (dark interior with a Fallen shrine; no
  bubble-local group; 9 D1FPRELOAD).
- 366 (second PC): highway start with bubble 3 registered, `--ah-zone-schedule 24@150` (to The
  Breach), PPU hook on 0x25715c / 0x13f8ec / 0x3204a8 / 0x31f6bc. No reaction to the switch
  (screen unchanged, no loading, no kick). Trace: `0x3204a8` is the world controller's progress
  poster (states 2 -> 0x23 in order during join/load, 0x23 at emu 5:21 = in world); the slice-set
  loader `0x13f8ec` ran for orbit (0, 0x10) and for 112 only - never for 24. The type-11 handler
  `0x25715c` ran 9 times (the join delta, r5=0x771, then empty heartbeats) and **never again
  after emu 5:15.955**, although the stand-in kept sending a type-11 heartbeat every second and
  the client kept receiving host messages (no baboon). In the world, type 11 no longer reaches the
  world-state handler, so a mid-session world-state delta is dropped at dispatch. Host
  notifications enter at `0x42a498` (subtype at payload+9, 1..0x2d) and go to the receiver object
  registered at +4 (virtual call); the host-session type table is `0x258778` (17 entries at
  0x175cf5c). TEST367 traces both across the switch.
  Later check (uploaded log): both `0x25715c` and `0x31f6bc` stop at exactly 10 hits, which
  looks like the watch's hit budget, so the "no type 11 in the world" reading is unconfirmed;
  static analysis shows mid-mission switches start from the player's position instead (orbit
  checkpoint, "how a slice-set switch starts").
- 353 (second PC): **bubble 9 = TERRESTRIAL COMPLEX** (underground hangar with containers and a
  truck; no bubble-local group; 4 D1FPRELOAD).
- 367 (second PC, logs uploaded 2026-09-27): highway start, bubble 3 registered,
  `--ah-zone-schedule 24@150`.
  - **Stand-in log.** The client's type-21 slice record settles at join: target 112 (state 4),
    then current 112, target -1, state 0. The stand-in pushed zone set 24 at join +151 s. The
    client sent no further type 21 and no new message type, and stayed in the world until the
    run ended at +284 s: **a host-pushed zone set is ignored**, as the static analysis predicted.
  - **Hooks.** They did not run the runbook's TEST367 watch list. The hits are for 0x42a498 (7),
    0x258778 (2), 0x25715c (5), 0x13f8ec (3) and 0x3204a8. That matches the older example that
    `Invoke-ObjectExperiment.ps1`'s header showed under the name TEST367, or a stale
    `d1\d1ppu.txt` (the script only rewrote the file when `-Watch` was given).
  - **Handler calls.** `0x25715c` ran for the join delta (emu 5:07, r5=0x771) and four more
    times (5:08, 5:09, 5:23, 6:05, the last two with content). It did not run for the zone push
    at about emu 7:38. `0x42a498` last ran at 5:23. Without the budgets of that list, "the
    world-state handler stops in the world" stays unconfirmed.
  - **Fix.** The script now always rewrites the watch file and saves the list as
    `<Name>-watch.txt`. TEST367's own list (runbook step 2) is still to run; TEST368 (walk into
    The Breach) is the more useful next run.

## Session 2026-09-29 — sync to origin/master, 01.24 environment rebuilt

- **Repo sync.** Git was not installed on this PC and `p3n1cillin/d1xenia` is private, so the
  checkout had sat at `0cb84ba` (TEST263-270) with 12 files modified or untracked after it. Git
  2.55.0 was installed with winget and the remote was authenticated through Git Credential
  Manager. `origin/master` had advanced to `80d4427` (~90 commits, +10,238 lines) and already
  contains the whole 01.24 effort. Fetched, then compared every one of the 12 local files
  against the remote blob by SHA-256: `d1coverage_diff.py`, `Test-ScreenReady.ps1` and
  `Get-RpcsHotThreads.ps1` were byte-identical (already committed from the other PC) and
  `Invoke-ObjectExperiment.ps1` was **older** on the remote (2848 B local vs 3784 B remote).
  The local copies were therefore superseded, not lost: stashed as
  `pre-pull local state TEST271-367 (superseded by origin/master)` and merged
  `--ff-only` with no conflict. A copy of all 12 files plus `uncommitted.patch` was kept
  outside the repo at `F:\d1\d1xenia-backup-prepull`. `local\` (786,479 files) untouched.
- **Correction to an earlier reading of this log.** The 01.00-era assumption that the disc's
  `PS3_UPDATE\PS3UPDAT.PUP` is the source of 01.24 is wrong: its header is `SCEUF` / `4.60`,
  i.e. PS3 **system firmware**, not a Destiny title update. The disc genuinely carries only
  01.00. 01.24 comes from Sony's update server instead.
- **01.24 obtained.** `scripts/ps3_update.py list BLES01857` reaches
  `a0.ww.np.dl.playstation.net` and lists 01.01-01.24; 01.24 is
  `EP0002-BLES01857_00-DESTINYPATCH0024` (31.6 MB). Fetched `EBOOT.BIN` (26,183,840 B, against
  01.00's 24,855,968 B) and decrypted it to `EBOOT.elf` with `rpcs3.exe --decrypt`.
- **Every 01.24 constant verified against the real binary**, not just read from the notes:
  `d1_ppu_hash.py` gives `PPU-ac58999f254e8d9dc817426c8c84b068ee8caaea` for 01.24 and
  reproduces the known 01.00 hash `PPU-10d41d88e5ce442299c2ba4cb681f0b307f33527` exactly, which
  validates the tool. Through `d1_eboot.py` (PPU virtual addresses, not file offsets) the
  2,390-byte PEM really is at `0x12d3740`, referenced only from `0x113894`, and the manifest flag
  at `0x18dca64` reads 1 and is referenced from `0xc65d14` — both matching `BUILDS["01.24"]` in
  `server/game_ca_patch.py`. `patch_yaml(..., sites=build_sites("01.24"))` emits the 01.24 hash
  and `0x012d3740`.
- **Data repo.** `p3n1cillin/d1xenia-data` cloned to `F:\d1\d1xenia-data`; it carries
  `types_01.24.json` (5,041 types), `services_01.24.json` (94 services),
  `handles_01.00_01.24.json` and `package_inventory_01.24.json` (1,034 files).
- **Preflight.** `scripts/d1_final_preflight.py --data-root F:\d1\d1xenia-data` passes all seven
  read-only checks (EBOOT identity/hash/patch sites, 94 services, 1,034-package inventory,
  activity join/state payloads, encodable services, both mission scripts). Server tests: 60
  passed (29 before the pull).
- **Still missing on this PC**, so no 01.24 run has happened here yet:
  `local\RPCS3_FINAL124_SMOKE001` (the isolated 01.24-patched RPCS3 copy), `local\RESEARCH`
  beyond the EBOOT fetched today, and the installed 1,034 update package files
  (`local\01_WORKING\ps3_destiny\PS3_UPDATE` is still empty). Next step is
  `scripts\Invoke-FinalBuildExport.ps1 -Packages` (about 10 GB) to populate them, then the
  A/B on SignOn field 14 from the 01.24 checkpoint.
- 354-364 (second PC, 2026-09-27, reported 2026-10-01): rest of the bubble map of
  act_cosmo_m0_highway (01.00). **10 JOVIAN COMPLEX, 11 FORGOTTEN SHORE, 17 SERAPHIM VAULT,
  19 REFINERY, 20 ROCKETYARD**; 15 and 18 no name visible (an "(X) OK" dialog covered the nav;
  15 outdoor snowy rocks, 18 rusty interior); 16 no name shown (teal interior with yellow-chevron
  barriers); **12 failed 3/3** (TEST356-358, "RPCS3 did not save a complete screenshot within 20
  seconds" - like bubble 4, probably an emulator stop while loading; logs on the second PC).
  TEST368 (traced type-11 transition) was never run and is dropped (mid-session type-11
  transitions are a dead end, see HANDOFF-2026-10-01.md).
