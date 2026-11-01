# Getting started

There are two ways to help. Most contributions need **no game files at all**.

## 1. Without game files (about 15 minutes)

You need Git, Python 3.14 and, for the Rust services, [Rust](https://rustup.rs) with `protoc`
(`apt install protobuf-compiler`, `brew install protobuf` or `winget install protobuf`).

```sh
git clone https://github.com/<you>/d1reimagined-source   # your fork
cd d1reimagined-source
python -m venv .venv && . .venv/bin/activate              # Windows: .venv\Scripts\activate
python -m pip install -r requirements.txt
python tools/check.py                                     # the checks CI runs
```

`tools/check.py` runs the conventions check, the Python tests and the Rust workspace tests, and says
PASS, FAIL or SKIP for each. `--python` or `--rust` runs one half. Tests that need game inputs skip.

Then pick something:

- **[Good first issues](https://github.com/p3n1cillin/d1reimagined-source/labels/good%20first%20issue):**
  small and well scoped, each saying which files it touches and what done looks like.
- **Tests:** the message encoders in `ps4/d1-server/crates/d1-bap` and the activity host's step
  schedule (`crates/d1-activity-host/src/state.rs`) can all be tested with synthetic input.
- **Research:** the [notes](../notes/README.md) are full of open questions. A clear write-up of
  how something works in the 2014 game, with sources, is a real contribution.
- **Docs:** if something here confused you, fix it for the next person.

[ARCHITECTURE.md](ARCHITECTURE.md) explains how the pieces fit, and [GLOSSARY.md](GLOSSARY.md)
explains terms like *sensor list*, *squad*, *slice set* and *RSAT*.

## 2. With your own game inputs

Native work, seeing the game run, needs inputs you dump yourself from a console you own: the PS4
version CUSA00219 01.33 (executable build 56578). Nobody here can share them, and nothing derived
from them goes in the repository.

1. **Emulator:** build the project's shadPS4 fork,
   [d1reimagined-shadps4](https://github.com/p3n1cillin/d1reimagined-shadps4) (branch `d1`).
2. **Inputs and generators:** follow [`ps4/BOOTSTRAP.md`](../ps4/BOOTSTRAP.md) for the order, and
   [`ps4/README.md`](../ps4/README.md#local-paths) for where the tools look for your files (`D1_ROOT`).
3. **Play the first mission:** take the pad with `tools/ps4_setup/d1-agr-run.ps1`, or let
   [`lab/tower-route`](../lab/tower-route/README.md) play A Guardian Rises unattended, from boot to
   Tower North.
4. **Where it stands:** [STATUS.md](STATUS.md) lists what works, what doesn't, and where to start.

## Before you open a pull request

- Keep it to one topic, branched from current `main`, and titled `area: summary`.
- Run `python tools/check.py --title "area: summary"`.
- Say what you ran and what you saw. "Offline only" is a fine answer; claiming in-game behaviour
  from unit tests is not.
- Never include game files, extracted assets, keys, credentials, personal paths or private traces.

The full rules are in [CONTRIBUTING.md](../CONTRIBUTING.md). Questions go to the help forum on the
[Discord](https://d1reimagined.com/discord).
