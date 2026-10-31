# 26 October 2026: colour grading fixed (before / after)

## Summary

Shadows in the PS4 version rendered pure black with saturated blue patches, and lit rooms in The Breach
blew out to white. The cause was the game's colour-grading table being assembled wrongly by the emulator;
with the fix, shadows show ambient light and detail and lit areas keep their colour.

## What's new

- Highway, same spot and view: before, the car wrecks and the gun are black and blue; after, they show
  rust, paint and shading (screenshots 01 before, 01 after).
- The Breach, same area (not the same view): before, pipes and walls render almost white; after, warm
  light and rust (screenshots 02 before, 02 after).
- Measured on the highway view: black pixels 9.8% of the frame before, 0.0-0.1% after.

## How it works

Destiny builds a 32x32x32 colour-grading table every frame, one slice per draw, and then reads it as a
3D texture to set the final look of the image. The memory it uses is shared within the frame with two
other small images. The emulator's texture cache mistook one of them for the table and grew the wrong
image, so the table's first slices (the dark, low-blue end) held stale data: dark colours turned black or
blue, and the bright end was wrong too. The fix makes the growing table extend only its true predecessor
(same format and size, exactly one slice fewer) and keeps rendered slices when the image grows. The
dumped table now maps black to black and white to white with smooth ramps between.

## Still in progress

- These screenshots are from 1280x720 test runs earlier today, not 1080p recordings.
- Overall image quality versus the original console has not been compared side by side.

## What's next

- Record a 1080p before/after clip of the same scene.

## Commits

- `b186831832924fa9bbc26aefbe6c8a9cab683c15` black, blue-tinted shadows were a corrupted colour-grading LUT (source repository)
- `92d9b513d6c18810f278d816436b7e9420dd7662` the lit Breach renders correctly with the LUT fix (source repository)
- `d93d64ab3f47092080aa954dd0038ea84d392b84` texture cache: Destiny's colour-grading LUT keeps all its slices (emulator repository)

Correction: the source repository's history was rewritten on 26 October to purge game files (executables, packages and a third-party runtime) from every commit. This package replaces the earlier one only to update the commit hashes; content and media are unchanged.
