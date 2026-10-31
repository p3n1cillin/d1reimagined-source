## Problem and change

The PS4 56578 route explorer could retry a previously fatal direction: when every direction was blocked, recovery cleared the lethal entries along with temporary wall evidence, then jumped backwards without consulting the planner.

Candidate selection now excludes the saved lethal set directly. Recovery clears only temporary wall evidence and returns to normal planning. A cell with all directions marked fatal stops before movement. The Windows driver loads only for a live run, allowing portable policy tests and `--help`.

## Validation

- Windows / Python 3.14: `python -m unittest discover -s scripts -p 'test_*.py' -v` — 184 passed.
- Eight new regression cases cover retained fatal moves, recovery wiring, all-fatal stopping without input, other-cell evidence, bounded candidate selection and portable help.
- `git diff --check` passes.
- These are policy and fake-driver checks. They do not establish a new native route, reliable death detection or mission completion.

## Inputs and attribution

Only original code, documentation and synthetic policy fixtures are included. No game data, native addresses, captures, credentials or private traces are added. Existing traversal/death-detection limits are recorded in the new research note.
