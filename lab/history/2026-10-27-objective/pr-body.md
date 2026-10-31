## Problem and change

On PS4 build 56578, the Divide handover left “Fight Through the Wall” visible and treated mode 3 as completion. Clearing objective 14 to 0 and presenting 17 in mode 3, with marker 46 and local machine 12, now shows “Fight Through the Fallen”, its authored description and a yard waypoint without a new-mission banner.

Add `d1-oracle sensor objective` and synthetic controls to generate the handover while preserving authority and other clients. Fix a separate route-loop regression where arrival tolerance and off-route recovery alternated without sending movement. Update the public checkpoint and journal, including the authored squad positions that narrow the remaining encounter investigation.

## Validation

- Focused Rust objective control/test passed; release oracle build passed.
- 189 Python script tests passed, including an execution of the real route loop at the failing synthetic geometry.
- Native divide16: Rust-generated list accepted, objective fields read back, resolver traced to the authored point, correct title/description/waypoint inspected. A reset to 0 then mode 3 also worked without mode 1. The full unattended mission steps do not yet include this handover.
- Native route retry sent movement beyond the former busy-loop point; the Divide route and combat are not complete.
- Broader oracle library run: 40 passed, 7 failed. Six require the absent reference executable; one requires a build-specific item table not generated in the source checkout. The existing 56578 sensor tests passed. No substitute reference inputs were used.

## Inputs and attribution

Original Rust/Python changes retain existing GPL notices. New controls use synthetic data. Only factual findings and original source are included; owned packages/executables, generated lists, screenshots, keys and raw run evidence stay private. Authored placement facts were checked against the user's owned 56578 content and loaded native bytes.
