## Problem and change

On PS4 build 56578, route replay can infer the opposite camera turn direction when its calibration movement slides against scenery. Run 12 did this at the garage and spiralled away from the first waypoint.

Add optional `d1-walk.py --turn-sign {-1,1}` to hold an already measured direction. Calibration still measures rate when its sample agrees; opposite samples are rejected and the later automatic sign flip is disabled. Omitting the option preserves existing behavior.

Update the continuation journal with the native garage isolation and Divide results: prop 3 alone opened the passage; all twelve squad states were read back in the native client; visible ground combat remains unverified; activating six authored members crashed and is excluded from replay. Record the empty cached weapon-stat lead without claiming an ammo fix.

## Validation

- Python compilation, help and invalid-direction checks passed.
- Script suite: **184 passed**.
- Native garage traversal completed with direction fixed to +1; the fan route also reached the Divide, with an explicitly documented diagnostic interruption/restart. This is not claimed as another uninterrupted full-route pass.
- Native read-only checks and failed member activation are recorded separately in the journal. No combat, kill, ammo or survivability fix is claimed.

## Inputs and attribution

Original tool changes and factual research notes only. No executable, game content, derived native fixtures, raw logs, screenshots, run captures or credentials are included. Existing attribution is preserved.
