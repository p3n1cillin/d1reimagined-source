## What
- Add four test-only cases for BAP matchmaking messages 42/43 in `ps4/d1-server/crates/d1-bap/src/matchmaking.rs`.
- Independently pin zero-valued optional-field wire bytes; verify absent/present fields and `u32`/`u64` limits.
- Exercise response configuration lengths 0, 1, 127, 128, and 65,535 bytes.
- Reject malformed, duplicate, type-mismatched, truncated, and overflowing protobuf fields; skip unknown fields safely.
- No production codec or existing wire vector changes.

Related to #4; **does not close it** because other `d1-bap` encoders still need coverage.

## Tested
- Isolated Windows checkout of upstream `ab18afbd45ddea2bb13cea06c5f431ea7f486429`.
- `cargo test --offline --locked --jobs 1 -p d1-bap --lib -- --test-threads=1`: **178 passed, 0 failed**, up from 174 baseline.
- `cargo clippy --offline --locked --jobs 1 -p d1-bap --all-targets`: exit 0 (existing warnings outside the changed code).
- `git diff --check`: exit 0. The added tests match `rustfmt`; workspace-wide `cargo fmt --all -- --check` fails on pre-existing formatting differences, so unrelated source was not reformatted.
- Upstream CI and independent maintainer review are requested, not claimed.

## Seen in the game
None. These are source-only synthetic codec tests. A 65,535-byte synthetic response is a parser boundary test, **not** a client-compatible payload claim. No game executables, assets, keys, firmware, credentials, or private traces used.

## Checklist
- [x] Based on current upstream `main`, one topic, one changed Rust file
- [x] No game files, keys, credentials, personal paths, or raw private traces
- [x] No hardcoded values inferred from a single trace
- [x] Tests needing private inputs not introduced
- [x] AI-assisted: yes
- [ ] Independent maintainer approval and GitHub CI