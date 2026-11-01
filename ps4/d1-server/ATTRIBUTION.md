# Attribution

`d1-server` is **GPL-2.0-or-later**. Some of it is derived from other people's work, and this file
records what and from where. Credit is not a formality here - this project only exists because these
projects published theirs.

## Derived from - shadPS4 / shadNet / rpcsn

**`proto/shadnet.proto`** is vendored **verbatim** from the shadPS4 emulator
(`src/shadnet/shadnet.proto`), sha256
`87520f427ee254db3bf0a66af9cad42fc4569d2fc28903f55492dffb0cc99c17`, taken on 1 November 2026 from
the project's build of it, [d1reimagined-shadps4](https://github.com/p3n1cillin/d1reimagined-shadps4)
branch `d1` at commit `34c579fee98c5fec3ce0910d3d939213e702dec1` (shadPS4 v0.19.0 and its later shadNet
work). Earlier copies were taken at upstream commit `8bdec718b6512ac8d17a44efa0039c06c6810f6e`. Its own
header reads:

```
SPDX-FileCopyrightText: Copyright 2019-2026 rpcsn Project
SPDX-FileCopyrightText: Copyright 2026 shadNet Project
SPDX-License-Identifier: GPL-2.0-or-later
```

`crates/shadnet-proto` compiles that schema with `prost`, so the generated types are derived from it.
We vendor rather than reimplement **deliberately**: the schema is the authority, and generating from
it means our encoding cannot silently drift from the client's.

**`crates/shadnet-wire`** is an original Rust implementation whose **protocol constants, packet
framing and enum discriminants are taken from shadPS4's `src/shadnet/client.{h,cpp}`** (GPL-2.0):
the 15-byte little-endian header layout, `PacketType`, `CommandType`, `Notification`, `ErrorType`,
`SHAD_PROTOCOL_VERSION`, `SHAD_HEADER_SIZE`, `SHAD_MAX_PACKET_SIZE`, the reply payload shape
(`ErrorType` byte then a `u32`-prefixed protobuf blob), and the `ServerInfo` handshake. Those values
must match exactly or the wire breaks, so they are quoted rather than invented.

**`proto/shadps4-client-constants.h`** is a **verbatim excerpt** of six enum blocks -
`PacketType`, `CommandType`, `NotificationType` and `ErrorType` from `src/shadnet/client.h`
(sha256 `987e69ee45183a1403caaa964fcde72fc3f169ffe58d854d31bbebd019e87639`), plus
`OrbisNpMatching2Event` and `OrbisNpMatching2EventCause` from
`src/core/libraries/np/np_types2.h`
(sha256 `31e801dff01898ceaf3fbc079c9229231f204ab459ad327cc602364e3d30580d`) - taken from the same
d1reimagined-shadps4 commit `34c579fee98c5fec3ce0910d3d939213e702dec1` (first from the local working tree at
`1a51a41696a17a9a3b67ad6658dda547ddca4a98`). It is vendored so that
`shadnet-server controls` can check our Rust enums against **the client's own text** rather than
against constants we typed in ourselves. See `proto/PROVENANCE.md`.

**Room-model semantics** in `crates/shadnet-server` - the `has_details()` requirement on
`CreateRoomReply`/`JoinRoomReply`, the dotted-quad `addr` / host-order `port` conventions and the
`IpStringToAddr` failure behaviour, the per-recipient `ctx_id` notification-routing contract, the
fact that `HandleRoomEvent` implements only four of the eight room events, the reply/notification
payload framing differences, and the 21-byte `Matching2StunPing` layout - are read out of shadPS4's
`src/shadnet/client.{h,cpp}`,
`src/core/libraries/np/np_matching2/np_matching2_{mm,internal,signaling}.{cpp,h}`,
`src/core/libraries/np/np_types2.h` and `src/core/libraries/network/p2p_sockets.cpp`, in the local
working tree at commit `1a51a41696a17a9a3b67ad6658dda547ddca4a98` (upstream
`8bdec718b6512ac8d17a44efa0039c06c6810f6e` for the vendored schema),
<https://github.com/shadps4-emu/shadPS4>, **GPL-2.0**. **No code is copied** - the room state
machine, the emitter guards and the session registry are original - but the wire shapes and the
client-side preconditions are quoted, because a server that gets them wrong fails *silently*.

**Per-instance isolation facts** in `crates/d1-harness` - that shadPS4's user directory resolves to
`$PWD/user` **first** and only then to `$XDG_DATA_HOME/shadPS4` (so there is no `--user-dir` flag and
a stray portable directory silently merges two instances' identities); that the NP online ID sent in
`LoginRequest` is the per-user `shadnet_npid` field persisted in `users.json`, not a global setting;
and that the backend addresses are the `shadnet_server` / `shadnet_webapi_server` settings parsed as
`host:port` - are read out of shadPS4's `src/common/path_util.{h,cpp}`, `src/core/user_manager.{h,cpp}`,
`src/core/emulator_settings.h`, `src/core/libraries/np/np_handler.cpp` and `src/shadnet/client.cpp`,
in the local working tree at commit `1a51a41696a17a9a3b67ad6658dda547ddca4a98`,
<https://github.com/shadps4-emu/shadPS4>, **GPL-2.0**. **No code is copied**; the facts are quoted
because the harness's whole job is to refuse when one of them is violated.

⚠ RPCS3's RPCN client was **not** used. It was examined and found protocol-incompatible above
discriminant 11 (shadNET relocates the room commands to 100-115; only 18 of their 91/71 messages
share a name, and the shared ones differ in field count), so there was nothing to port.

- shadPS4 - <https://github.com/shadps4-emu/shadPS4> - GPL-2.0
- rpcsn / shadNet - the NP-replacement lineage shadNet builds on

Thanks in particular for the fact that made this project's content gate cheap: **GoldHEN's FTP
decrypts SELF/sprx on read**, and shadPS4's protocol being *in the tree* turned a presumed
reverse-engineering project into an afternoon.

## Deliberately NOT used

Recorded so nobody wonders whether we quietly borrowed. **No AGPL-3.0 and no unlicensed prior art
enters this project in any form** - not linked, not ported from, and **not read**, including as a
source of facts extracted by a human or an agent. An unlicensed repository is *more* restrictive
than AGPL, not less: no licence means all rights reserved. Independently of licence, prior art from
other Demonware-era titles implements TCP with 3DES-EDE3-CBC and HMAC-SHA1, which is **not** the
transport our client uses (an SCTP-shaped association over UDP, with DTLS), so that code would not
have fitted. ⇒ `design/licensing.md`.

## In scope for future use, with credit

Because `d1-server` is **GPL-2.0-or-later** - note the *or later* - we can combine with GPL-2.0,
GPL-3.0, LGPL, MPL, Apache-2.0, MIT and BSD code. (GPL-2.0-**only** could not have taken GPL-3.0
material; the "or later" is what buys that.) **AGPL-3.0 and unlicensed code stay out.**

If we fork or crib from any of these, it gets an entry above with the specific files and the
upstream commit - not a vague nod.

- Any GPL-compatible or permissively licensed implementation, evaluated on its merits when a
  concrete need arises. None is in use today.
- Public write-ups, forum threads, conference talks, and RFCs: facts, freely usable, cited where they
  inform a design decision.

## Our own work

Everything else - the server, the gate ledger, the containment harness, the run tooling and the
research documents - is original to this project, and is offered under GPL-2.0-or-later so that the
next person preserving something can take it the way we took shadPS4's.
