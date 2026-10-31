# 26 October 2026: into The Breach, the first rifle, and the first Fallen attack

## Summary

The PS4 version of A Guardian Rises now plays from the Cosmodrome highway into The Breach, where the
Guardian finds and picks up the starter rifle and the mission moves on to Fight Through the Wall.
Fallen then attack and down the Guardian, who respawns. Today's three clips are the first recorded
footage of this stretch, with the game's own audio. The run is driven by scripted controller input.

## What's new

- **Into The Breach (clip 1, ~60 s):** the Guardian walks through a lit interior hall into The Breach; the
  location card "THE BREACH / OLD RUSSIA, EARTH" appears. The Ghost is in hand, as in the original
  opening (the Guardian has no weapon yet).
- **The starter rifle (clip 2, ~60 s):** the objectives FIND A WEAPON and GRAB THE RIFLE appear; walking
  onto the rifle picks it up (the weapon materialises in hand and the HUD shows the auto rifle), and the
  objective changes to FIGHT THROUGH THE WALL. The rifle arrives with no ammunition ("NO PRIMARY AMMO");
  an ammo pickup a little later fills it (12 | 114).
- **The first Fallen attack (clip 3, ~55 s):** an enemy health bar and red radar contact appear, the
  Guardian takes damage (red screen edge), and "GUARDIAN DOWN - Dreg, House of Devils" follows, then a
  respawn ("SUPER CHARGED").
- **Colour grading (separate before/after package):** the black, blue-tinted shadows and blown-out
  lighting from earlier runs are fixed; all of today's footage is with the fix.
- Clips are recorded from the game window only, with game audio only (no microphone, no desktop sound).

## How it works

- **Lighting and the barrier (prop authority).** The Breach's lights, barrier and rifle are objects the
  game client creates only while it holds their "authority". The activity server now assigns those blocks
  to the player's machine, so the client builds and keeps them.
- **The rifle.** The original server announced a world loot drop and accepted a pickup claim. The
  server now sends the drop, decodes the client's claim (service 601) and answers it in the exact format
  the client expects (an empty reply made the client disconnect). It then delivers the item: an inventory
  change carrying the Khvostov auto rifle, a profile update that unlocks the primary weapon slot (the
  slots start locked in the tutorial), and new character cards so the Guardian is re-equipped live.
- **Unarmed start.** The tutorial character is now served without weapons, so the rifle is genuinely the
  first weapon, as in the original.
- **The Fallen.** Enemy squads are simulated by the client. Each squad needs a state record; three hash
  fields in it must hold the client's own "none" value (a zero there is read as a real reference, and the
  squad then found no place to spawn). With that fixed, one desired member per squad, sent 30 s after the
  rifle, brings the Dreg and Vandal attack.
- **Colour grading.** The game builds a 32x32x32 colour-grading table one slice per draw. The emulator's
  texture cache confused it with other small images that reuse the same memory and dropped its darkest
  slices every frame. Fixed in the emulator: the table now grows as one chain and keeps all slices.

## Still in progress

- The rifle arrives without ammunition; in the original it is loaded. The firing presses in this
  recording did not fire (the ammo count does not change in the footage); firing was verified in earlier
  runs (12 -> 6 with ejected casings) but is not shown in today's clips.
- Fight Through the Wall starts, but the route past The Breach is not mapped yet, and the objective chain
  after it (the graveyard and the Fallen waves along the way) is not served yet.
- The Fallen encounter is the first spawn only: authored Dreg variants, encounter completion and
  correct difficulty are still open.
- Menu item icons stay as loading spinners inside the mission.
- This run used a 1280x720 internal display setting in a 1920x1080 window, so the clips are 1080p but
  upscaled from 720p; future recordings use 1080p internally. Audio is aligned by timestamp and has not
  been checked by ear for exact lip/sound sync. Frame rate in these clips averages about 29-30 fps.

## What's next

- Find the walkable way from The Breach toward the graveyard objective and record it as a route.
- Serve the next objectives in order (14 -> 17 -> 20 -> 23 -> 26) with the Fallen encounters on the way.
- Load the rifle on pickup, and record the Khvostov firing and a Fallen being killed.
- Fix the loading-spinner icons in the character menu.

## Commits

Public source repository, since the last update (25 October):

- `6c1b25784f531bff7b04e61f0c91a4cfaf760ac4` The Breach lights up and the barrier opens (prop authority)
- `75730ff6ab144c52811f702d47dacad9942f3dd9` black, blue-tinted shadows were a corrupted colour-grading LUT
- `5a26ad7f3532ae28535b9ad1a84bcb671a250781` the lit Breach renders correctly with the LUT fix (notes)
- `9c020da1b07b8805d61347ff0e70cf16c83280f6` the starter rifle drops in The Breach and the client claims it
- `4954638da127b2700a16590ae85c398011cab725` the claimed rifle is granted as a family-4 item delta
- `627add21d3b64f45e5df3338818cf5c20bf82b6a` picking up the rifle unlocks the primary slot
- `39780d02d41db26ce77924923c116ed0e8ef0bfe` the granted Khvostov is equipped live and fires
- `a776ddcf0f0cda0f0debc58d644aa38768dcc703` the tutorial Guardian starts unarmed and the rifle is its first weapon
- `11bc694b439719481c727b2f41e054e3b24c93fa` the Breach Fallen spawn and attack the Guardian
- `e321a3dbb3f7512def7a4a998796a89f56d473ed` Fight Through the Wall starts after the rifle; the mission's nav points
- `85b628ebc6d0f70c3bb7032b80af2c169d52e819` the 15-minute inactivity kick; multi-key pad calls
- `44d1a39cb33709ea51f40013826088825bb94cac` the natural crossing into The Breach; Investigate completes
- `71ef188f8945c61ed0eb8d04e0f21bb498ac82ea` Enter the Wall completes and Find a Weapon starts from the trigger report
- `bde2bc55ef9c82b7160b32fa08b783d12b6d3859` d1-explore.py, a pad-driven route finder; its first test did not find the way out of The Breach
- `afba29a98fa509a70e35bd80b5553bf7ac37183c` recording with game audio only, and the daily evidence package

- `a179bd2288b60035c96a59a06b30cfb81c10604a` the project's emulator build is public (setup guide now points to it)

New today: the emulator build the project runs is open source as its own repository,
https://github.com/p3n1cillin/d1reimagined-shadps4 (branch `d1`): upstream shadPS4 v0.19.0 plus one
commit per fix, with a build script, settings and environment variables, so contributors can rebuild the
exact emulator used for these runs.

- `4e746a98538ecb236e63887f4e812537d3612f8b` net: a real guest address, and TCP sendmsg/recvmsg on Windows
- `c446ce1bf75121f6af4478969d78f2c78fa05c7c` shadnet: retry the startup probe and allow 10 s per attempt
- `49358237f1981f4d6a39a6430cfb9c706dd2e55e` cpu_patches: do not JIT-decode an unreadable guest address
- `01e87c22e928655bb301aef223cbc87df401baf0` shader: V_CMP_CLASS_F32 with a single NaN kind
- `d93d64ab3f47092080aa954dd0038ea84d392b84` texture cache: Destiny's colour-grading LUT keeps all its slices
- `0381fe4e669986619112a35b1cb244f0d392bbd7` audio: game-audio capture for recordings
- `05caefb88ba7a0c33f001fdb496d72b98db443f0` build script, settings and environment variables
