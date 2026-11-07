# `proto/` - provenance

Two files live here. Neither is ours, and neither may be hand-edited.

## `shadnet.proto`

Vendored **verbatim** from the shadPS4 emulator, `src/shadnet/shadnet.proto`.

| | |
|---|---|
| sha256 | `00caee32f645afbd0b6154dc4767a3632222e8d6886e7d7795ec2da0bcebba01` |
| taken from | [d1reimagined-shadps4](https://github.com/p3n1cillin/d1reimagined-shadps4) branch `d1-upstream-1107` (fork PR #10), commit `79b8a71e96e16d108942283699eb7866110d638b` (7 November 2026: upstream shadPS4 50c5c1b merged into the fork, branch `d1-upstream-1107`, fork PR #10) |
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
| `src/shadnet/client.h` (`PacketType`, `CommandType`, `NotificationType`, `ErrorType`) | `4aa258b67fb9585878700f3993b1b471c06729fd74b68c6240a35f8908007c0c` |
| `src/core/libraries/np/np_types2.h` (`OrbisNpMatching2Event`, `OrbisNpMatching2EventCause`) | `eb600e689777ed6058d6f586d37dc3b4a0f78ec2514b80a30d33fdd542f4d73c` |

Taken from d1reimagined-shadps4 `d1-upstream-1107` (fork PR #10) at `79b8a71e96e16d108942283699eb7866110d638b` on 7 November 2026 (first from `~/dev/shadPS4-d1trace` at `1a51a41696a17a9a3b67ad6658dda547ddca4a98`). That brought 21 new commands (13 `SetClientVersion`, 41 `LookupOnlineId`, 201-214 TUS and TSS, 301-302 trophies) and `ErrorType::TooLarge`.

`shadnet-server controls` re-derives both hashes **and** asserts each block still occurs verbatim in
the upstream file, whenever the shadPS4 tree is present (`SHADPS4_SRC`, default
`~/dev/shadPS4-d1trace`). When it is not present that check reports **SKIP** - never
PASS. A skipped check is not a passed check; that is how the last two instrument defects in this
project hid under a green board.
