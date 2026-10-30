# D1 Reimagined — PS4 preservation source

The active target is **Destiny 1, CUSA00219, executable build `56578.17.04.12`**
(package metadata 01.33), using shadPS4 and the local Rust services in this tree.
**The complete game is not playable.** This source publication includes the
supplied preservation project's servers, protocol libraries, diagnostic tools and
generators, with our tested preparation and content-reader fixes. Game inputs,
firmware, generated catalogues, native fixtures and private run evidence are
excluded.

## Builds — `capture-rig/builds.json`

The executable build is a parameter. `D1_BUILD` selects an entry in
`capture-rig/builds.json`; the default is `later-56578`. Its identity pins drive
every executable, manifest and client-patch gate (`d1-build` crate for Rust,
`capture-rig/d1build.py` for Python and shell). **01.29 stays the address
reference**: every fixed address in `design/`, the probe-site catalogues and the
BAP tables was measured on it. Those catalogues keep their 01.29 derivation hash
and fail their build gate on any other image. A build with
`addresses_valid: false` gets no client patch, and address-bearing tools refuse
it until its addresses are re-derived. Run the 01.29 golden controls with
`D1_BUILD=01.29`.

## Current local checkpoint — 12 October 2026

The owned repack's executable is build 56578 (April 2017), not 01.29, so the
target was moved to it. Its full game directory (2,119 files, 2,105 content
packages) was extracted privately. The default English manifest policy selects
1,392 packages, above the 1,325-descriptor cap measured on the 01.29 client; the
cap for this build is not yet measured. Address-independent static controls pass
on the new executable (RTTI, NID resolution, name hashes, character/roster
schemas); account/profile schemas and all fixed-address controls fail, as
expected until re-derivation.

Unpatched contained Windows boots fault before the title screen at executable
address `0xFAAF4`: a non-worker thread runs a pending job with a null job-worker
pointer. `patches/CUSA00219-56578-job-helper-guard.xml` is a **synthetic** guard
that returns the function's own "no job run" result in that case. With it, the
game shows its logo, title and **Connecting**, then reports that its servers are
unavailable. Its PSN OAuth host (`auth.api.sonyentertainmentnetwork.com`) differs
from 01.29's and must be redirected locally; `/SignOn` is not reached yet.
shadPS4 0.19 requires `AppElf="eboot.bin"` on patch entries. See
[the current evidence summary](../docs/STATUS.md).

The supplied project's `status/` and `design/` documents preserve its August 2026
research checkpoint. Its Guardian-in-world observations involved explicitly
synthetic interventions and a different operator's inputs. They are historical
research, not successful runs on this installation. Private citations may name
files that are intentionally absent from this source publication.

## Build without game inputs

Use stable Rust and Cargo. Package/scanner development needs no game files:

```sh
cd ps4/d1-server
cargo test --locked -p d1-pkg --bin d1-byte-scan --bin d1-platform --bin d1-pkg-header-audit
cargo run --locked -p d1-pkg --bin d1-byte-scan -- controls
cargo run --locked -p d1-pkg --bin d1-platform -- controls
```

Broader servers need the dependencies listed in [BOOTSTRAP.md](BOOTSTRAP.md),
including `protoc`, a C/C++ toolchain and CMake for some analysis crates.
The `d1-trace` and `d1-observe` crates are standalone; build them separately.
For the decoder and synthetic controls without a GUI, run from the repository root:

```sh
cargo test --locked --manifest-path ps4/d1-trace/Cargo.toml --no-default-features
cargo run --locked --manifest-path ps4/d1-trace/Cargo.toml --no-default-features --bin d1-synthetic-gate -- controls
```

The gate fixture is original synthetic data generated through the existing
tracer encoder, with zero registers except synthetic RAX. It is never native
evidence. Its generator and rejection control ship as source. Two eboot golden
integration suites require the opt-in `private-fixtures` feature and locally
regenerated files under `d1-eboot/tests/fixtures/`. They are excluded from default
public checks; never call that a native verification pass.

## Owned-input and native work

Follow [BOOTSTRAP.md](BOOTSTRAP.md) for the input/generator order, then inspect
the canonical `capture-rig/run-boot.sh` plan and effective settings. That guide
records upstream checks from 11 August; it is not a promise of a boot here.
Use `tools/ps4_setup` from the repository root with explicit paths to this `ps4/`
tree and your private tool/dump locations. Their defaults come from the local
path contract below. The already-applied source fixes mean their
hash-guarded patch installers are unnecessary in this public checkout.

### Local paths

The `tools/ps4_setup` scripts find private files through one contract
(`d1paths.py` for Python, `D1Paths.ps1` for PowerShell; nothing is hardcoded).
Set `D1_ROOT` to the folder that holds `tools/`, `d1preservation-main/` and
`01.29/`, or put `{"root": "D:/d1"}` in the per-user config file
`%APPDATA%\d1reimagined\paths.json` (Windows) or
`${XDG_CONFIG_HOME:-~/.config}/d1reimagined/paths.json`; see
[paths.example.json](../tools/ps4_setup/paths.example.json). An environment
variable wins over the file, and each location can be moved on its own:

| Variable (config key) | Default |
| --- | --- |
| `D1_TOOLS` (`tools`) | `<root>/tools` |
| `D1_SHADPS4` (`shadps4`) | `<tools>/shadPS4/0.19.0` |
| `D1_LIBORBISPKG` (`liborbispkg`) | `<tools>/liborbispkg/0.2.245` |
| `D1_WORKSPACE` (`workspace`) | `<root>/d1preservation-main` |
| `D1_STAGING` (`staging`) | `<workspace>/cfw-staging` |
| `D1_PKG_DIR` (`pkg_dir`) | `<root>/01.29/d1shad` |

`python tools/ps4_setup/d1paths.py` (or `pwsh tools/ps4_setup/D1Paths.ps1`)
prints each location and whether it came from the environment, the config file
or a default. Scripts call the lab helpers (`d1-pad.ps1`, `d1-winguest.py`, ...)
from `D1_TOOLS`, so copy `d1paths.py` and `D1Paths.ps1` there with them.
`Build-D1ShadPS4.cmd` reads only `D1_TOOLS` or `D1_ROOT` from the environment.

A Linux network namespace contains Linux children only. Windows guest isolation
and transport remain unaccepted. Never send a game client to public services
as a debugging shortcut. Do not post game files, decrypted inputs, certificates,
account records or raw private traces in issues or pull requests.

`d1-server`, the original PS4 tools and their existing notices retain
**GPL-2.0-or-later**; see [d1-server/LICENSE](d1-server/LICENSE) and
[attribution](d1-server/ATTRIBUTION.md). The repository's GPL-2.0-only default for
the retained PS3 source does not replace these notices.
