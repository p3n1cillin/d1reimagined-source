# Bungie's own description of the activity host (GDC 2015)

Source: Justin Truman (Bungie), *Shared World Shooter: Destiny's Networked Mission Architecture*, GDC 2015. It's on the GDC YouTube channel, https://youtu.be/Iryq1WA3bzw (56 min). The user found it on 5 November 2026.

This note summarises the talk in our own words, with timestamps. It then checks the talk against what the captures and the lab have shown (`d1-live-captures-2026-11-05.md`, `ps4-56578-public-sessions-2026-11-02.md`). The talk describes launch-era Destiny, which is the generation of 56578 and the Xbox 360 captures. Treat it as design intent, not a wire format.

## What the talk says

**Three kinds of host** (3:07, 34:33-37:37, 46:23):

- **Physics host.** One of the consoles in a bubble. It runs combat, AI bodies, bullets and damage as Halo-style peer-to-peer.
- **Mission activity host.** One per fireteam, in Bungie's data centre. It runs the activity script (Lua) for the current activity, and it owns *every private bubble* in the destination.
- **Bubble activity host.** Exactly one per public bubble *instance*. It runs that bubble's ambient AI, chests, resource nodes and public events. Strangers in the bubble each keep their own mission host.
- **World server.** Separate again. It holds characters and progression.

**Activity state and sensors** (24:42-30:24):

- Scripts run only in the cloud; no activity-script Lua runs on the console.
- An activity declares all its sensors up front. A sensor is a small piece of mission-critical state, such as "has this squad spawned, how many are alive".
- Both sides hold a full copy of the sensor state.
- **Auth state** flows down from the activity host and is always the authority. The slides use this name; on the audio it sounds like "off state".
- **Sense state** flows up from the physics host. It is time-sliced to about **10 Hz** and sent atomically: all of an update's sensor changes go together.
- Script example: `squad place()`, then `sleep_until(0 alive)`, then `mission complete()`.
  - Place sends auth state down, and the physics host spawns the AI.
  - Kills come back up as sense state.
  - When the count reaches 0, the script continues and sets an objective sensor, which shows the banner.
- Sensors aren't one-to-one with objects: a squad sensor covers several AI, and one object can carry several sensors.

**Migration and reconcile** (30:55-34:03):

- The activity host is the session authority. It arbitrates which console becomes physics host, and every migration goes through it.
- On a migration it calls a **reconcile** on every sensor. The new physics host must change its simulation to match the auth state, for example by spawning or killing AI until the alive count matches. That reuses the sensor's normal place code.
- Scripts keep running with nobody in the bubble. **Entering an empty activity bubble is just a reconcile**: the entering player starts the bubble in its default state, is elected physics host, and fixes up only the declared sensors.

**Mission host versus bubble host rules** (37:37-40:07):

- A mission host may do anything in its private bubbles.
- In a public bubble it **may not instantiate or modify networked objects**. It may still play dialogue, set objectives and play full-screen effects, which are local to the fireteam.
- Mission-specific events in a public bubble are written into the *public bubble's* script. The script checks something like "if a player here is on activity X, enable this interactable". The Vault of Glass entrance encounter runs on the Venus bubble host this way, which is why passers-by can help.
- Public and private halves of an activity talk only through a few **global flags**.

**Bubbles and matchmaking** (10:16-13:21, 45:20):

- A bubble is the unit of simulation, asset streaming and matchmaking. The standard bubble is 6 players and 25 AI, with at most two bubbles in memory.
- The airlock ("Z leg") timeline:
  - assets start pre-caching about 15 s out from the bubble swap;
  - matchmaking for the next bubble's host starts at about 10 s;
  - if nothing is found by about 5 s, matchmaking is abandoned and the player starts a standalone instance that strangers can join later.
- Fireteam members always match into the same instance of a bubble, but they're untethered and can be in different bubbles.
- Each player reserves public-bubble slots for absent fireteam members, so live public bubbles averaged about 3 players.

**Scale** (30:24, 54:14-55:39):

- An activity host is a stripped-down Destiny executable of about 45 MB, ticking at 10 Hz. About 4,600-5,000 of them fit on one 40-core server.
- A story mission has fewer than 100 sensors, typically around 50. A public bubble script has 20-40, and the peak is in the low hundreds.
- Only 15-20 sensors are instantiated in any one bubble at once.
- If an activity host dies, the player is sent to orbit with one of the animal-name error codes (48:29).

## What this means for our host

- **It matches what the captures show.**
  - The captures' long-lived "PAH" connection, which alone carries region records, plays the *bubble activity host* role: one per public instance, staying up for the whole time in it.
  - Concurrent public instances are separate bubble-host connections.
  - The per-host reading in `d1-srcp.py` (one session per host) is the right model.
- **Restoration's public-bubble stall (step 4).**
  - By the talk's rules, our mission host must not create or change networked objects in a public bubble; the bubble host does that.
  - Story beats in a public bubble would live in that bubble's own script, gated on "a player here is on activity X".
  - Next: find where the PAH's script, rather than the mission host, drives the Restoration step in the captures.
- **The cinematic record is a sensor.**
  - It has a change counter, and a playing byte that the setter `0x403140` applies only when the counter moves. Ghidra's decompile shows this. The setter copies the 0xD4-byte record to `+0x1AC`, so the record's counter (`+4`) sits at `+0x1B0` and its playing byte (`+8`) at `+0x1B4`. It returns early if the counter is unchanged.
  - That is auth state applied as a delta. A cinematic is a "full-screen effect", which a mission host may play even in a public bubble.
  - Reconcile is the time a new physics host is made to match. So on bubble entry the client should get the current cinematic state as a reconcile, not only as a later change.
- **Election and entity slots.** The activity host chooses the physics host, so the entity-slot handover (udt 25→23, 26→24, with incident 2938) is a likely candidate for the migration or reconcile handshake. This is a hypothesis to check against the captures.
- **Tick rate.**
  - The real host ticks at 10 Hz and the console sends sense state at 10 Hz. Our slow turns of 1.5-4.5 s (`ps4-56578-lab-stalls-2026-11-05.md`) are 15-45 ticks late.
  - This is more reason to take the codec off the connection loop.
- **Bubble swap timings** (15 s, 10 s, 5 s) are the numbers to match when the host matchmakes a player into a public instance. If the host has no instance to offer, the client should fall back to its own standalone bubble.
- **Sensor counts** are a sanity check on the sensor lists in `ps4/fixtures/56578/mission`: about 50 per story mission, and 15-20 live per bubble.
