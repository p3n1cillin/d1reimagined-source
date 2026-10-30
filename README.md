# D1 Reimagined

**The original Destiny (2014), running on PC, where it was never released.** An independent, open-source preservation project running Destiny 1's final PS4 version on PC through the shadPS4 emulator, against its own rebuilt services, one mission step at a time, with every step shown in public Dev Blogs and videos.

[![A Guardian Rises progress](https://img.shields.io/endpoint?url=https%3A%2F%2Fd1reimagined.com%2Fbadge%2Fmission.json)](https://d1reimagined.com/progress?ref=github)
[![Latest Dev Blog](https://img.shields.io/endpoint?url=https%3A%2F%2Fd1reimagined.com%2Fbadge%2Flatest.json)](https://d1reimagined.com/latest?ref=github)
[![From the lab](https://img.shields.io/endpoint?url=https%3A%2F%2Fd1reimagined.com%2Fbadge%2Flab.json)](https://d1reimagined.com/progress?ref=github)
[![YouTube](https://img.shields.io/endpoint?url=https%3A%2F%2Fd1reimagined.com%2Fbadge%2Fyoutube.json)](https://www.youtube.com/@d1reimagined?sub_confirmation=1)
[![Discord](https://img.shields.io/endpoint?url=https%3A%2F%2Fd1reimagined.com%2Fbadge%2Fdiscord.json)](https://d1reimagined.com/discord?ref=github)
[![CI](https://github.com/p3n1cillin/d1reimagined-source/actions/workflows/ci.yml/badge.svg)](https://github.com/p3n1cillin/d1reimagined-source/actions/workflows/ci.yml)
[![Good first issues](https://img.shields.io/github/issues/p3n1cillin/d1reimagined-source/good%20first%20issue?label=good%20first%20issues)](https://github.com/p3n1cillin/d1reimagined-source/labels/good%20first%20issue)

[![The latest Dev Blog](https://d1reimagined.com/latest/thumbnail)](https://d1reimagined.com/latest?ref=github)

**Watch** the [Dev Blog videos and Shorts](https://www.youtube.com/@d1reimagined) · **Read** the [latest Dev Blog](https://d1reimagined.com/latest?ref=github) · **Follow** the [first mission, step by step](https://d1reimagined.com/progress?ref=github) · **Talk** on the [Discord](https://d1reimagined.com/discord?ref=github)

Experimental, independent Destiny 1 preservation research and original local service implementation. **The complete game is not playable.** This consolidated repository includes local service stand-ins, protocol experiments, mission rules, diagnostic tools, scoped emulator patches and regression tests.

## Get involved

- **Star the repository:** it costs nothing, and stars are how other Destiny players find the project on GitHub.
- **Pick up a task:** [good first issues](https://github.com/p3n1cillin/d1reimagined-source/labels/good%20first%20issue) and [help wanted](https://github.com/p3n1cillin/d1reimagined-source/labels/help%20wanted). Many need no game files at all, including research, documentation, tests and CI.
- **Follow progress:** [Dev Blogs](https://d1reimagined.com/devblogs), and #dev-updates on the [Discord](https://d1reimagined.com/discord). To hear about each milestone here, choose **Watch → Custom → Releases**: every milestone gets a [release](https://github.com/p3n1cillin/d1reimagined-source/releases) with its Dev Blog and video.
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
mission, A Guardian Rises, now runs from the Cosmodrome through the Divide and
Dock 13 to its outro and post-game screens, with the project's services driving
its steps. △ CONTINUE launches the next story
activity and arrives in a populated Tower North, with its civilians, Patches and
Xûr ([29 October notes](notes/ps4-56578-continue-tower-2026-10-29.md)). Vendors
don't open yet, and the next mission's own world is a stand-in. Native stability,
progression beyond the first mission and multiplayer remain unproven.

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

You need Python 3.14 and a Rust toolchain; PowerShell 7 for the Windows helpers. The checks CI runs, none
of which need game files:

```sh
python -m venv .venv && . .venv/bin/activate      # Windows: .venv\Scripts\activate
python -m pip install -r requirements.txt
for d in server scripts tests tools/content_capture; do python -m unittest discover -s $d -p 'test_*.py'; done
cd ps4/d1-server && cargo test --workspace --exclude d1-oracle --exclude d1-activity-host
```

With your own inputs, `D1_EBOOT=<eboot.elf> D1_REPO=<repo root> cargo test --workspace` also runs the
tests that need the game executable; they skip without it. Set `D1_LOCAL`, `D1_RPCS3` and `D1_USRDIR` to
your own runtime locations for the PS3 helpers; the ignored `local/` directory is for private inputs and
generated state. Inspect native launch and input tools before running them, and never stop unrelated
emulator or service processes. Missing inputs, skips and blocked native acceptance must stay explicit:
passing checks do not mean the game is playable.

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md). In short: a focused pull request titled `area: summary`, with what
you ran and what you saw. CI checks it, a maintainer reviews it, and once approved and green it merges
automatically. Labels show where it stands. Never attach game files, raw private traces, keys, credentials
or personal information. Community rules are in [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md), and security
reports go privately via [SECURITY.md](SECURITY.md).

## Licence and third-party rights

Original project source is offered under GPL-2.0-only unless a file carries a different existing licence notice, including GPL-2.0-or-later annotations. See LICENSE and THIRD_PARTY_NOTICES.md. Preserve upstream copyright and licence notices when applying patches. Source licensing does not cover Destiny assets, executables, firmware, trademarks or third-party binaries. Imported `data/` payloads retain their original ownership and are not relicensed by the project source licence.

D1 Reimagined is not affiliated with or endorsed by Bungie, Sony Interactive Entertainment, or RPCS3. Destiny and associated names and assets belong to their respective rights holders. There is no warranty, compatibility promise, legal immunity or announced playable release.

## Repository consolidation — 20 October 2026

`d1reimagined-source` is the canonical public repository. It incorporates the former `d1reimagined` source/research history and `d1reimagined-data` snapshot/history. No separate companion repository is required. See [the migration record](docs/REPOSITORY-CONSOLIDATION.md) for preserved refs, layout and verification. Statements about separate private repositories in dated research records describe the original layout.
