<!-- SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project -->
<!-- SPDX-License-Identifier: GPL-2.0-or-later -->
# script-host: the mission-script sidecar

This is a Node 22 process that runs Destiny 1 build 56578's mission and activity scripts on Blam
Network's MIT [BullShcript](https://github.com/Blam-Network/tiger/tree/main/packages/bullshcript)
compiler and VM. `d1-activity-host` starts one per activity instance and talks to it over JSON
lines. [PROTOCOL.md](PROTOCOL.md) is the contract. The design and the current coverage are in
[notes/ps4-56578-script-sidecar-2026-11-05.md](../../notes/ps4-56578-script-sidecar-2026-11-05.md).

## Inputs, none of them in this repository

| Variable | What it points at |
|---|---|
| `D1_TIGER_DIR` or `D1_BULLSHCRIPT_DIR` | a built tiger checkout (`npm install && npm run build -w @blamnetwork/bullshcript`), or its `packages/bullshcript` folder. If neither is set, the sidecar looks for `third-party/Blam-Network_tiger` one or two folders above the repository. |
| `D1_SCRIPTS_DIR` | the 56578 script folder: Fukugen Daihon `destiny1/v56578_17_04_12_1251_evil`. These scripts are unlicensed: read them in place, and never copy them, quote them, or commit anything derived from them ([licensing](../../ps4/design/licensing.md), rule 2). |

## Use

```sh
node --test                              # unit tests; the 56578 ones skip without D1_SCRIPTS_DIR
node tools/coverage.mjs --seconds 600    # compile/run coverage for every activity (Markdown)
node src/sidecar.mjs                     # the sidecar itself: JSON lines on stdin and stdout
```

CI's `script-host` job builds BullShcript from the public tiger repository at a pinned commit and
runs `node --test`. It does not check out Fukugen Daihon, so the tests that need the 56578 scripts
skip there.

## Layout

| Path | What it does |
|---|---|
| `src/sidecar.mjs` | the process: reads lines, handles `hello` and `end`, and hands everything else to the session |
| `src/session.mjs` | one activity instance: the runtime, the inputs, and the outputs as protocol messages |
| `src/scripts.mjs` | the compiled script set, the script source the runtime reads, stand-in bundles, and area-bundle guesses |
| `src/types56578.mjs` | 56578's row type ids, the lifetime wire mapping, and the native type → row type table |
| `src/natives56578.mjs` | the 56578 native stubs (each one logs when called) and the missing-native checks |
| `src/incidents.mjs` | incident id ↔ name hash, from Codex's catalogue (`ps4/fixtures/56578/incidents-56578.tsv`) |
| `src/bullshcript.mjs` | finds and loads the BullShcript checkout |
| `tools/coverage.mjs` | the coverage report |
| `test/` | Node tests on synthetic scripts we wrote (`test/synthetic.mjs`) |
