## Problem and change

Activating the Divide's six authored squad members on PS4 build 56578 crashed at `0x1BDCC1`. Their A image (`0x80800868`) had an all-zero optional binding: the client treats that as present, caches it, then assumes its lookup resolves.

Add `d1-member-states.py` to generate the tested unbound diagnostic activation with the absent reference at +0x50 and index -1 at +0x58. Preserve authority, unrelated clients and existing B values; reject missing/duplicate members or incompatible A types before writing the output. This tool supplies no performance or flight path and is not an encounter script.

Document the corrected native test and all twelve authored squad labels, linked to actual native ids. The extracted entry headers and label bytes independently matched the loaded native counterparts.

## Validation

- **188 script tests passed**, including the failing zero-binding negative control against the native reference-cache condition, metadata/B preservation, type/duplicate refusal and keeping the output intact when a member is missing.
- Native run 14: corrected member 21 first, then all six authored members; all six read back at generation 1 with the absent binding, repeatedly updated for over ten minutes without the earlier crash.
- The source tool generated the same six A images and its output was published to the native client successfully.
- This fixes the tested invalid-binding crash. Ground combat, kills, correct encounter order and flight paths remain unverified; the all-squad diagnostic shows multiple ships together.

## Inputs and attribution

Original tool/tests and factual research notes only. Owned entries, generated client lists, credentials and raw native evidence remain private. No reference-build Oodle control pass is claimed. Existing licenses and attribution are preserved.
