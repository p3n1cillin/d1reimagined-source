# Contributing

Keep changes focused and describe the observed problem, implementation, validation and remaining limitations. Use disposable local fixtures and report failed or skipped checks accurately. Do not claim native gameplay acceptance from isolated unit tests or static data exports.

Start with [the contributor checkpoint](docs/STATUS.md). The active PS4 build and
input requirements are in [ps4/README.md](ps4/README.md); retained PS3 checks are
in the root README. Run each analysis tool's controls before citing its output.
Keep exact executable and manifest pins intact, and keep one format implementation.
Bring small pull requests with reproduction steps and actual checks. GitHub issues
are enabled for build failures, protocol investigations and proposed experiments.
For questions and early discussion before an issue, use the help and ideas forums
on the [community Discord](https://d1reimagined.com/discord).

PS4 contributions retain GPL-2.0-or-later and their attribution notices. Original
retained PS3 files use the existing root license unless their notices say otherwise.

Do not submit game files, extracted assets, firmware, credentials, keys, account records, personal paths or raw private traces. Share only original code and material you have permission to redistribute. Preserve licence notices in upstream-derived changes. Report security issues privately rather than including exploit targets or sensitive records in a public issue.

## How pull requests are handled

Pull requests move from opening to merge automatically, so you can follow yours from its labels:

1. **Checks.** Every pull request runs the source checks (Python, the Rust workspace's tests that need
   no game inputs, the package tools on Linux and Windows), the contribution guard (no game files,
   keys or large binaries) and the conventions check (a title like `area: summary`, no secrets, no
   broken relative links). A first-time contributor's checks start once a maintainer approves the run.
   CI has no game files, so tests that need them (the game executable, the guest symbol file) skip
   there. Run them on your machine before opening the pull request, from `ps4/d1-server` with your
   inputs set (see [ps4/README.md](ps4/README.md)), for example
   `D1_EBOOT=<eboot.elf> D1_REPO=<repo root> cargo test --workspace`, and report the result under
   Validation.
2. **Review.** Your pull request is labelled `needs review` and a maintainer reviews it. You'll get
   either an approval or `changes requested` with comments. Pushing new commits after an approval
   puts it back to `needs review`.
3. **Merge.** Once approved, it's labelled `approved` and squash-merged automatically when the checks
   pass. Pull requests that change `.github/` (workflows and automation) are labelled
   `maintainer merge` and merged by hand.

While it waits, the checks are tracked with labels too: `awaiting CI approval` until a maintainer
starts a first-timer's runs, `checks failing` (with a comment linking the failed checks) until a push
fixes them, and `update branch` when `main` has gained a required check your branch predates, which
you clear by rebasing onto `main` or merging it into your branch.

Along the way it is labelled by size (`size/XS` to `size/XL`) and by area, flagged `needs rebase` if
it conflicts with `main`, and gets a comment if another open pull request touches the same lines.
Until one of your pull requests is merged you can have one open at a time; after that, up to 20.
Pull requests with no activity for 30 days are marked `stale` and closed 14 days later; reopen them
whenever you pick the work up again.
