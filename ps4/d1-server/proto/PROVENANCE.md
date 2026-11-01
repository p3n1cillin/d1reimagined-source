# `proto/` - provenance

Two files live here. Neither is ours, and neither may be hand-edited.

## `shadnet.proto`

Vendored **verbatim** from the shadPS4 emulator, `src/shadnet/shadnet.proto`.

| | |
|---|---|
| sha256 | `87520f427ee254db3bf0a66af9cad42fc4569d2fc28903f55492dffb0cc99c17` |
| taken from | [d1reimagined-shadps4](https://github.com/p3n1cillin/d1reimagined-shadps4) branch `d1`, commit `34c579fee98c5fec3ce0910d3d939213e702dec1` (1 November 2026) |
| earlier upstream commit | `8bdec718b6512ac8d17a44efa0039c06c6810f6e` |
| upstream | <https://github.com/shadps4-emu/shadPS4> |
| licence | GPL-2.0-or-later (header: rpcsn Project 2019-2026, shadNET Project 2026) |
| messages | 98 |

The emulator the project ships is the authority, so the schema is re-vendored from its `d1`
branch whenever that branch moves on. Re-vendoring is a whole-file copy, never a hand edit, and the
controls below catch a copy that wasn't refreshed.

**The hash above is not load-bearing.** It is re-derived at run time, from the file itself, and
compared against the copy recorded in `ATTRIBUTION.md`:

```
cargo run -p shadnet-server -- controls
#   PASS  the compiled schema's sha256 is the one ATTRIBUTION.md records (re-derived, not transcribed)
```

`crates/shadnet-proto/build.rs` compiles this path with `prost`, and
`crates/shadnet-server/src/lib.rs` embeds the same path with `include_str!`, so the file the
controls hash is the file the types are generated from.

### Why vendor rather than reimplement

The schema is the authority. Generating from it means our encoding cannot silently drift from the
client's - and drift in a protobuf schema is not a compile error anywhere else. Coupling to the
**schema** is checkable with a hash; coupling to an implementation would not be.

## `shadps4-client-constants.h`

A **verbatim excerpt** of six enum blocks, so the Rust enums in `shadnet-wire` can be checked
against the client's own text instead of against numbers somebody typed in.

| source | sha256 |
|---|---|
| `src/shadnet/client.h` (`PacketType`, `CommandType`, `NotificationType`, `ErrorType`) | `987e69ee45183a1403caaa964fcde72fc3f169ffe58d854d31bbebd019e87639` |
| `src/core/libraries/np/np_types2.h` (`OrbisNpMatching2Event`, `OrbisNpMatching2EventCause`) | `31e801dff01898ceaf3fbc079c9229231f204ab459ad327cc602364e3d30580d` |

Taken from d1reimagined-shadps4 `d1` at `34c579fee98c5fec3ce0910d3d939213e702dec1` on 1 November 2026 (first from `~/dev/shadPS4-d1trace` at `1a51a41696a17a9a3b67ad6658dda547ddca4a98`). That brought 21 new commands (13 `SetClientVersion`, 41 `LookupOnlineId`, 201-214 TUS and TSS, 301-302 trophies) and `ErrorType::TooLarge`.

`shadnet-server controls` re-derives both hashes **and** asserts each block still occurs verbatim in
the upstream file, whenever the shadPS4 tree is present (`SHADPS4_SRC`, default
`~/dev/shadPS4-d1trace`). When it is not present that check reports **SKIP** - never
PASS. A skipped check is not a passed check; that is how the last two instrument defects in this
project hid under a green board.
