# AGENTS.md

D1 Reimagined revives Destiny 1 by rebuilding its online services and running the final PS4 build
(56578) on the project's shadPS4 fork. The game's own code is the reference: the server's job is to send
what the client expects, found by reading the client. [CONTRIBUTING.md](CONTRIBUTING.md) has the full
rules. These are the ones agents break.

- **No game material in the repository.** Never add game files, extracted assets, firmware, keys,
  account records, native traces, memory dumps, screenshots or derived binary fixtures. Private evidence
  stays outside the repository; summarise it in the pull request.
- **Never hardcode a value from one observation.** A number read from one trace, one run or one video
  frame (a stat, a hash, a count, an address) is a test target, not a constant. Derive it from content or
  from the client's own reports, or leave it open and say so.
- **Addresses belong to a build.** 56578 is the target; 01.29 offsets are only a reference for finding the
  same code. Never apply an offset across builds without re-deriving it.
- **Tests that need private inputs skip without them.** Gate them on the input being present (the
  executable via `D1_EBOOT`, the repository root via `D1_REPO`) and print why they skipped. They must
  never fail on CI for lack of inputs, and never pass vacuously when the input is there.
- **Say what was observed and where.** Unit tests, static exports and synthetic responses are offline
  results. Claim in-game behaviour only from a native run, naming the build and emulator commit.
- **Don't drive the emulator unasked.** Launch, input, capture and watchpoint tools act on a real
  emulator process. Run them only when the person you work for asks, and never stop processes by name.
- **No local paths.** Use environment variables or placeholders such as `<home>`, never a real home
  directory or drive layout.
- **One implementation per format.** Reuse the existing codecs, readers and message definitions instead
  of writing parallel ones.
- **Keep licence headers.** New PS4 source files carry the SPDX header their neighbours use.

Before pushing, run the tests for what you changed (see [CONTRIBUTING.md](CONTRIBUTING.md)) and the
conventions check:

```sh
python3 .github/scripts/check_pr.py --title "area: summary"
```

Pull request titles read `area: summary`. Fill in every section of
[the template](.github/pull_request_template.md), including `AI-assisted`.
