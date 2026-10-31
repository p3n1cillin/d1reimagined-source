## Problem and change

On PS4 build 56578, entering the Divide changes the local machine from 3 to 12. PR #30 let a live control list carry `@authority 12,2,0`, but the same list in a scheduled mission step or world table silently kept the launch authority. A `%96|900` step therefore could not reproduce the working live update.

Use one sensor-list reader for all three paths. Preserve lifetime fields, client filtering and both spawn-team variants; reject malformed or duplicate authority headers with file/line context instead of silently truncating values or reverting to defaults.

## Validation

- `cargo test --locked -p d1-activity-host --lib`: **90 passed**, with the owned `later-56578` executable selected. Five new synthetic tests require no game inputs and cover overrides, defaults, malformed headers, lifetime/client preservation and team patching.
- `cargo check --locked -p d1-activity-host --bin d1-activity-host`: passed.
- `cargo build --locked --release -p d1-activity-host --bin d1-activity-host`: passed.
- Initial library run without build selection: 81 passed, nine failed because the default executable was absent. The explicit 56578 selection resolved all nine.
- Native scheduled Divide replay remains pending. The existing native authority result documented in the journal used PR #30's live-control path; this PR does not claim a new native pass.

## Inputs and attribution

Original server/configuration code only. No game files, derived fixtures, credentials, captures or raw run evidence are included. Existing licenses and attribution are preserved.
