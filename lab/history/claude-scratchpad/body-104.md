## What
- Add an independent `python-windows` GitHub Actions job with `windows-latest`, Python 3.14, the existing pinned-by-range requirements and the same four unittest directories as Linux: `server`, `scripts`, `tests`, `tools/content_capture`.
- Explicitly fail the PowerShell step on a nonzero unittest exit code; preserve per-directory grouping.
- Keep the existing required Linux check named **`python`** entirely unchanged, avoiding any branch-ruleset changes.
- Add a test-only synchronization in `server/test_ah_session.py`: await the asynchronous activity-host join before asserting timed heartbeats. **No production code changed.**

Related to #98. Leave #98 open until a Windows job has run and passed with any platform-specific skips explained.

## Tested
- The workflow edit is isolated to `.github/workflows/ci.yml`, based on upstream `ab18afbd45ddea2bb13cea06c5f431ea7f486429`.
- The original `python` job and all other jobs were left unchanged.
- An initial GitHub Windows Python 3.14 run found one heartbeat test failure (`0` received where `>=2` expected); this was fixed by waiting for the asynchronous join before collecting heartbeats.
- On Windows with Python 3.13 after the fix: heartbeat case **5/5** repeated passes; `test_ah_session.py` **8 passed**; entire `server` suite **225 passed, 1 skipped**. The separately installed local Python 3.14 lacks its standard library; GitHub Actions is the authoritative Python 3.14 check.
- GitHub CI/actionlint on the **updated commit** is pending; independent maintainer review is still required. Earlier local Python 3.13 runs without the declared dependencies also reported missing `numpy`, which is installed by this workflow.

## Seen in the game
None. CI-only; no emulator or game inputs.

## Checklist
- [x] Based on current upstream `main` and one topic
- [x] No game files, firmware, keys, credentials, private traces, personal paths, or binaries
- [x] Existing required check context preserved
- [x] AI-assisted: yes
- [x] Updated Windows GitHub Python 3.14 CI green (CI run 41036860223, head `98a28e27`)
- [ ] Independent maintainer review