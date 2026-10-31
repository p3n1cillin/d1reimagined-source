# First native enemy kill (run sense35, 28 Oct 2026, 08:12 UK)

- Guardian inside the garage interior past the Divide gate; Vandal point-blank (Divide squad 17, distance field ~2.2).
- Aim fix that made it possible: the hip/ADS reticle is at (968, 652) in the 1936x1119 capture, not the frame centre 571 (helpers fired ~80 px high).
- 08:12:49.530 client incidents: 734, 1825, 2389, 3056, 2462, 2512, 3246, 3856 (kill set; differs from player death 734/1825/2079/2767).
- 08:12:49 read-only B snapshot (Codex snapshot-b-images.py, file 2026-10-28-sense-codex/b-images-claude-aim36.jsonl):
  squad 17 i32 [2,0,0,1] -> [2,0,0,0] (B+0x0C 1 -> 0), scalar16 (B+0x10) 2.2 -> 2040.0, byte 0x20 -> 01.
- 08:12:49.592 udt 6 payload 95 bytes received (not captured: capture budget spent by udt 18 rows).
- 08:12:52 player death (all squads' B+0x10 -> 2040.0), 08:12:54 respawn at the checkpoint outside the garage (incidents 566/3749/222); squad 17 stays B+0x0C = 0.
- Conclusion: B+0x0C behaves as the squad's live-member count; B+0x10 as a distance to the player (2040 = none).

## Super / melee hang (sense35 08:12, 08:22; sense36 09:11)
- Pressing R1 (U) point-blank, or Q+U, puts the Guardian (served Titan Striker, subclass index 710) into a
  third-person pose with arc on both fists that does not complete. sense36: pose from 09:11:04, the
  Guardian stands helpless until the death incidents 734/1825/2079/2767 at 09:11:37 (respawn 566/3749/222).
- sense35 08:22:42-46: same pose cast deliberately outside after a respawn.
- Expected for a Striker: Fist of Havoc, one leap and ground slam. The user first read the pose as a
  Warlock super. Open: why the ability never executes (animation/physics, or a server-owned piece).
