# 29 October 2026 (late edition): people in the Tower

Candidate public update: The Tower is no longer empty. After A Guardian Rises, the Guardian arrives in Tower North, and civilians now stand along the walkways, along with a research-assistant Frame and the mysterious Xûr at his door. Their placement comes from the game's own content; how many appear is our reconstruction for now.

Private production input. This report authorises no publication, final-video edit or release.

## Reporting window and provenance

- Europe/London reporting day 2026-10-29. Follows the 15:39 and 17:39 UK packages, which are referenced, not resent. Coverage: 17:40Z to 22:35Z.
- Build: PS4 56578 under the project's emulator fork, served by the project's own activity host and backend from the public source's main branch (commit in metadata). The Tower's character list was read from the game's own content; no game content is in the package.

## What changed

1. **Tower North is populated (run sense60, 22:11Z).** Tower North has 15 groups of characters, each with authored slots: civilians who stand, sit, kneel, lean on railings or walk; vendors; a research-assistant Frame; and Xûr. The server now tells the game how many of each group to show. All 20 characters appeared within 12 s of arrival, and the Guardian walked among them.
2. **Earlier the same evening (sense57):** a first test filled the same groups with values copied from the mission, and the characters appeared. That confirmed the mechanism, and the late run uses the per-slot choice instead.
3. **Orbit and the Director work.** When the Guardian is idle in the Tower, the game returns it to orbit. The orbit screen, its announcements and the Director's solar-system map all display.

## Tests

- sense60: the full unattended run from boot through the mission, the ending, the end-of-mission screens and CONTINUE into the Tower (22:11Z), then a camera circle and a walk. 20 of 20 characters are present. **PASS** for populating Tower North.
- Talking to characters ("Approach") shows its prompt, but no vendor screen opens yet. **Open.**
- An experiment sending the next story step to a hangar world (sense59) loaded that world but went no further. It is not used.

## Maturity

- Tower North characters: **Partially Working**. They appear in their authored places. Which ones appear, and how many, is a reconstruction. It is not the original servers' schedule; Xûr in particular appeared on a fixed weekly schedule in the original game, and here he appears because our reconstruction enables his slot.
- Vendors: **Planned**. Their prompts appear; their menus do not open yet.
- Travel to the Tower: unchanged since the 17:39 edition. It still skips the original story cinematic.

## Evidence index

The clip is a continuous game-window capture of run sense60 (29 Oct 2026), recorded shortly after arrival. Native 1920×1080 at about 30 fps, with game audio from the emulator's audio output (one port, no microphone), re-encoded as H.264/AAC with no edits.

| Clip | Shows | Notes |
|---|---|---|
| clip01 (60 s) | The Guardian walks past civilians on the Tower North walkway, up to Patches 99-40 (Research Assistant) and on to Xûr, Agent of the Nine, at his door | The walk is lab input steered by the on-screen marker, so the movement is mechanical, with stops and turns |

Screenshots (frames from the same capture):
1. A civilian leaning on the walkway railing.
2. A civilian standing on the walkway.
3. Patches 99-40, Research Assistant.
4. Xûr, Agent of the Nine, at his door.

Proved visually: characters are present in Tower North and can be walked up to. Inferred, not shown: that their number and selection match the original game (they are a reconstruction).

## Remaining issues and next steps

- Vendor menus: make Approach open the vendor screens.
- The other Tower areas (the hangar, the plaza with the Cryptarch, Postmaster and Gunsmith) have their own character groups, still to be served.
- The story cinematic between the mission and the Tower.
- The values on the end-of-mission screens are still synthetic. The experience gain is recorded correctly but shown nowhere in this build's post-game layout, so the post-game screens will not show it.

## Privacy, audio and rights

- Game window capture only; no overlays, personal identifiers or desktop. The border is cropped.
- Game audio comes from the emulator's own audio output (no microphone). The game's music may be present and is flagged for review.
- The character name shown is the project's test character.

## Handoff state

Late edition: 1 clip (fully decoded at packaging) and 4 screenshots. It supplements the 15:39 and 17:39 UK packages.
