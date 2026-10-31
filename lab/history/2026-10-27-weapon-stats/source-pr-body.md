## Problem and change

On PS4 build 56578 the equipped Khvostov used a 12-round magazine because the peer's primary investment-stat table was zero. Inventory-instance changes were accepted but did not supply that table.

Add `d1-oracle character weapon-stats` to resolve the equipped owned definition, read its base list at +0x50, replace one peer weapon table and round-trip it through the client's codec. The optional instance defaults at +0x68 remain separate. The command guards the executable hash, refuses invalid keys/slots and existing outputs, and preserves every other decoded field. Existing character-builder defaults remain compatible; regenerated fixtures need the explicit patch step. Documentation records the native source trace and reproduction workflow.

## Validation

- Release Rust CLI build passed. Four targeted synthetic tests passed (writer isolation/sentinels, unsafe shapes/keys, base-versus-instance lists, existing self-relative guards).
- Owned-content extraction produced 13 base pairs; the generated peer round-tripped IDENTICAL. Independent decode comparison found changes only in the primary 128-byte table.
- Actual CLI refusals passed for an existing output, an empty heavy slot and absent stat 63; original/output hashes stayed unchanged.
- Native natural trigger-35 trial: six owned/native code controls passed. Hardware trace followed Magazine 44 from the peer through the primary weapon builder to flat stat 20; the HUD changed from baseline 12/114 to 25/101. Special/heavy zero controls were preserved and the debugger detached cleanly.
- Fresh full-list native trial: natural trigger 35; the builder received Magazine 44, Impact 80 and Inventory 15. HUD 25/134; clean detach after four hits. Reserve still differs from reference 285 and remains open; this PR does not claim correct starting reserve or measured damage.
- All configured branch checks passed for fc8d98a. These hosted checks do not run owned gameplay.

## Inputs and attribution

No game files, generated native fixtures, keys, screenshots or raw run evidence are included. Content was read from the user's owned selected build. Existing GPL notices are retained. Native damage/armour, combat completion and the ship destination require separate validation.
