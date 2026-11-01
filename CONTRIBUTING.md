# Contributing

New here? [docs/GETTING-STARTED.md](docs/GETTING-STARTED.md) gets you from clone to green checks in about
15 minutes, and [the contributor checkpoint](docs/STATUS.md) says where things stand. The PS4 build and its
inputs are described in [ps4/README.md](ps4/README.md), and the retained PS3 checks in the root README. For questions and early ideas,
use the help and ideas forums on the [community Discord](https://d1reimagined.com/discord). Issues are for
build failures, protocol investigations and proposed experiments.

## Already working in a fork or your own project?

Anyone who wants to help is welcome, including people who have started a project of their own: the
maintainer is happy to work with you. Please send your work here, even if it's rough. One shared tree means your work reaches everyone who builds
the project, gets reviewed and tested by people who know the protocol, and isn't lost when a fork goes
quiet. Everything merged is credited in [CREDITS.md](CREDITS.md) and the release notes.

- **Open it as a draft pull request** and say what's missing. Unfinished work is welcome; we'll help get
  it over the line.
- **Big or mixed changes:** split off the smallest piece that stands alone and send that first. Or open an
  issue that links your branch, and we'll work out the order together.
- **Leave "Allow edits by maintainers" ticked**, so a maintainer can fix a title, a test or a merge
  conflict for you instead of sending it back.
- **Copied files into a fresh repository instead of forking?** That's fine. Make a normal fork, copy your
  changed files over a branch from `main`, and open the pull request from there; a maintainer can help.
  The inputs check keeps game files, keys and large binaries out of every pull request, so nothing private
  can slip in by accident.

## Opening a pull request

1. Fork, branch from `main`, and keep the change small and focused.
2. Title it `area: summary`, for example `d1-bap: cover loot claim round trips`.
3. Run the tests on your machine, including the ones that need your game inputs, from `ps4/d1-server`:
   `D1_EBOOT=<eboot.elf> D1_REPO=<repo root> cargo test --workspace`. CI has no game files, so those
   tests skip there.
4. Run the conventions check, `python3 .github/scripts/check_pr.py --title "area: summary"`, then fill in
   the template: the change, what you ran (skips and failures included), and what you saw in the game, if
   anything. Run an analysis tool's controls before citing its output, and don't claim in-game behaviour
   from unit tests or static exports alone. Working with an AI agent? Point it at [AGENTS.md](AGENTS.md).

Never include game files, extracted assets, firmware, credentials, keys, account records, personal paths
or raw private traces. Share only your own work, or material you may redistribute, and keep licence and
attribution notices. Keep the exact executable and manifest pins, and one implementation per format.
Report security issues privately.

## What happens next

CI runs on every push: the Python and Rust tests, the package tools on Linux and Windows, an inputs check
(no game files, keys or large binaries) and a conventions check (title, no secrets, no broken relative
links). A bot comment summarises your pull request for review (template, linked issues, tests, what to
look at closely), and lint findings on the lines you add show inline under *Files changed*. A maintainer
then reviews it. Once approved and green, it squash-merges automatically. Labels
show where it stands:

| Label | Meaning | What to do |
|---|---|---|
| `needs review` | Waiting for a maintainer | Nothing yet |
| `changes requested` | Review comments to address | Push fixes; it returns to `needs review` |
| `approved` | Merges once the checks pass | Nothing; new commits need a fresh review |
| `checks failing` | A required check failed (a comment links it) | Push a fix |
| `update branch` / `needs rebase` | Behind `main`, or conflicts with it | Rebase onto `main`, or merge `main` in, and push |
| `awaiting CI approval` | A first-timer's CI waits for a maintainer to start it | Nothing |
| `maintainer merge` | Touches `.github/`, so it's merged by hand | Nothing |

You also get size and area labels, and a comment if another open PR touches the same lines. Until you've had
a pull request merged you can have three ready for review at a time (drafts don't count); after that, up to 20. A PR with no activity for 30 days
gets a `stale` reminder; nothing is closed automatically.

## Licence

PS4 contributions are GPL-2.0-or-later and keep their attribution notices. Original retained PS3 files use
the root licence unless their notices say otherwise.
