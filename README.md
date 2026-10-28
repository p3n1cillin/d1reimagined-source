# D1 Reimagined

[![Source checks](https://github.com/p3n1cillin/d1reimagined-source/actions/workflows/source-checks.yml/badge.svg)](https://github.com/p3n1cillin/d1reimagined-source/actions/workflows/source-checks.yml)
[![Good first issues](https://img.shields.io/github/issues/p3n1cillin/d1reimagined-source/good%20first%20issue?label=good%20first%20issues)](https://github.com/p3n1cillin/d1reimagined-source/labels/good%20first%20issue)
[![Discord](https://img.shields.io/discord/1558817182551646220?label=Discord)](https://d1reimagined.com/discord)

Experimental, independent Destiny 1 preservation research and original local service implementation. **The complete game is not playable.** This consolidated repository includes local service stand-ins, protocol experiments, mission rules, diagnostic tools, scoped emulator patches and regression tests.

Project website: https://d1reimagined.com
Community Discord: https://d1reimagined.com/discord (questions, ideas and contributor discussion)

## Get involved

- **Pick up a task:** [good first issues](https://github.com/p3n1cillin/d1reimagined-source/labels/good%20first%20issue) and [help wanted](https://github.com/p3n1cillin/d1reimagined-source/labels/help%20wanted). Many need no game files at all, including research, documentation, tests and CI.
- **Follow progress:** [Dev Blogs](https://d1reimagined.com/devblogs), and #dev-updates on the [Discord](https://d1reimagined.com/discord).
- **Ask first:** the Discord help and ideas forums are the place for questions and early ideas. Issues are for confirmed problems and planned work.
- **Emulator:** the project's shadPS4 build is [d1reimagined-shadps4](https://github.com/p3n1cillin/d1reimagined-shadps4) (branch `d1`).

## Scope

The active target is **PlayStation 4 CUSA00219 version 01.33** (internal build 56578, the final PS4 patch) with [the project's shadPS4 build](https://github.com/p3n1cillin/d1reimagined-shadps4).
Earlier work pinned 01.29; its addresses are kept as a reference for locating the same code in 01.33.
The [PS4 source tree](ps4/README.md) now includes the Rust services, protocol
libraries, diagnostic tools, canonical harness and generators from the supplied
preservation project, with the latest preparation and content-reader fixes.
The retained PlayStation 3 implementation targets version 01.24 with RPCS3.
See [current results and contribution priorities](docs/STATUS.md). The first
mission's recorded route now runs unattended from the Cosmodrome highway through
The Breach into the Divide. The starter rifle fires, and the Divide's
“Fight Through the Fallen” title and waypoint have native acceptance. Subsequent
contained runs have decoded Fallen kills and reached Dock 13 with “Find the Ship”.
The [28 October research note](notes/ps4-56578-pilots-clock-combat.md) records
the pilot-program candidate, missing activity-clock configuration and remaining
combat questions. Mission completion, progression, native stability and
multiplayer remain unproven.

This is a research repository, not a ready-to-play installer. The `data/` directory contains material derived from the historical PS3 research snapshot: schemas, logs, update diffs and a checksum manifest. Game packages, executables, firmware, third-party runtimes, account databases, private keys and current runtime captures are not included; supply your own authorised copies and check them against `data/SHA256SUMS.txt`. Use only inputs you are authorised to use. No public service, console, account or third-party system is a testing target.

## Layout

| Directory | Contents |
| --- | --- |
| `server/` | Loopback-only local service stand-ins, persistence and protocol tests |
| `scripts/` | Preparation, owned-run control, diagnostics and scoped audits |
| `missions/` | Original experimental mission and traversal rules |
| `patches/` | Scoped source increments for a separately obtained RPCS3 tree |
| `tools/content_capture/` | Optional private evidence capture and durable handoff tools |
| `tools/ps4_setup/` | Original preparation helpers and narrow patches for the PS4 runtime |
| `tests/` | Additional regression cases |
| `ps4/` | Active Rust preservation services, tools, research docs and canonical harness |
| `docs/` | Current contributor checkpoint and publication provenance |
| `notes/` | Imported PS3/PS4 research journals and historical evidence |
| `data/` | Derived PS3 schemas, logs, update diffs and checksum manifest (no game files) |

## Development

Use Python 3.14, PowerShell 7 where applicable, and the declared packages:

```powershell
python -m venv .venv
.venv/Scripts/python.exe -m pip install -r requirements.txt
.venv/Scripts/python.exe -m unittest discover -s server -p 'test_*.py'
.venv/Scripts/python.exe -m unittest discover -s scripts -p 'test_*.py'
.venv/Scripts/python.exe -m unittest discover -s tools/content_capture -p 'test_*.py'
```

Some checks require additional privately supplied source or runtime inputs. Missing inputs, skips and blocked native acceptance must remain explicit. The publication audit checks source integrity and syntax; it does not certify that the game is playable or that every runtime integration can be reproduced from this checkout alone.

The retired global-process punch-sweep launcher is retained only as `notes/Invoke-Final124PunchSweep.retired.ps1` for historical reference. Use the current owned-run wrapper; never stop unrelated emulator or service processes. Historical research journals and selected evidence are retained in `notes/` and `data/`; current machine state stays in ignored local directories.

Set `D1_LOCAL`, `D1_RPCS3` and `D1_USRDIR` to your own runtime locations. Launch helpers use this checkout's `data/` by default; set `D1_DATA_REPO` only to select a different data directory. The default ignored `local/` directory is for private inputs and generated state. Individual PS4 helpers expose path parameters; use their help and explicitly select your own installation rather than assuming example locations exist. Do not run native launch or input tools without inspecting their scope and effective containment. A Linux namespace does not contain a Windows process.

## Contributing

See CONTRIBUTING.md for small, reproducible changes, CODE_OF_CONDUCT.md for community rules and SECURITY.md for private security reports. Never attach game files, raw private traces, certificates, credentials or personal information to issues or pull requests.

All changes reach `main` through pull requests:

- **Required checks:** Python and Rust tests on Linux and Windows, plus a guard that rejects game files, binaries and keys.
- **Merging:** maintainer PRs merge automatically once the checks pass. Contributor PRs are reviewed first.
- **Labels:** PRs are labelled automatically by area, and get `needs rebase` when they conflict with `main`.

## Licence and third-party rights

Original project source is offered under GPL-2.0-only unless a file carries a different existing licence notice, including GPL-2.0-or-later annotations. See LICENSE and THIRD_PARTY_NOTICES.md. Preserve upstream copyright and licence notices when applying patches. Source licensing does not cover Destiny assets, executables, firmware, trademarks or third-party binaries. Imported `data/` payloads retain their original ownership and are not relicensed by the project source licence.

D1 Reimagined is not affiliated with or endorsed by Bungie, Sony Interactive Entertainment, or RPCS3. Destiny and associated names and assets belong to their respective rights holders. There is no warranty, compatibility promise, legal immunity or announced playable release.

## Repository consolidation — 20 October 2026

`d1reimagined-source` is the canonical public repository. It incorporates the former `d1reimagined` source/research history and `d1reimagined-data` snapshot/history. No separate companion repository is required. See [the migration record](docs/REPOSITORY-CONSOLIDATION.md) for preserved refs, layout and verification. Statements about separate private repositories in dated research records describe the original layout.
