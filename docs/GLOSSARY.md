# Glossary

Terms used across the code, notes and pull requests. [ARCHITECTURE.md](ARCHITECTURE.md) shows how they
fit together.

**01.24, 01.29, 01.33.** Game versions. 01.24 is the final PS3 build (the early route, on RPCS3).
01.29 was the first PS4 target. 01.33 (executable build 56578) is the final PS4 patch and the
current target.

**56578.** The internal build number of PS4 version 01.33, the executable everything here targets.
Addresses from other builds don't apply unchanged.

**A Guardian Rises.** The first story mission (activity index 164, world `act_cosmo_m0_highway`):
the Cosmodrome, the Wall, the Breach, the Divide, Dock 13 and the hangar.

**Activity.** Anything you launch: a mission, a strike, the Tower, a Crucible map. Identified by an
index and a hash.

**Activity host.** Bungie's server role that runs an activity: it loads the world, owns the
sensors and steps the mission. Ours is `d1-activity-host`.

**BAP.** Bungie's application protocol between the client and its services (Demonware transport):
sign-on, characters, inventory, vendors. Codec in `crates/d1-bap`.

**Bubble.** A region of a world that loads as a unit. A world has several, and the client moves
between them.

**Bundle (activity bundle).** A set of sensors that belong together, e.g. `enc_breach_m0` (the Breach's
encounter). Its hash is the *group* in a sensor list, and a row's position in it is the sensor's
index. [`tools/ps4_setup/d1-bundle-rows.mjs`](../tools/ps4_setup/d1-bundle-rows.mjs) names every row.

**Cohort.** A named set of squads that a mission step waits on until all are dead (`!G:I,I|N`).

**CUSA00219.** The PS4 title ID of the Destiny release this project targets.

**Eboot.** The game's executable (`eboot.bin`). Tests that need it read it from `D1_EBOOT`.

**Fixture.** A file the servers serve or the tests read: characters, mission lists, vendor data
([`ps4/fixtures/56578`](../ps4/fixtures/56578/README.md)).

**Generation.** A counter in a squad's state; a new value makes the squad run again.

**Group.** The first field of a sensor-list row: the hash of the bundle the sensor belongs to (for
example `0xFC0E45A3` for the Breach).

**Incident.** An event the client reports to the activity host (udt 18): a cinematic started or
stopped, a skip requested, a trigger crossed (3403). Mission steps can wait on them.

**Investment.** Bungie's name for the inventory and economy services (BAP message ids 4xx, 6xx and 9xx):
equipping, loot, vendor purchases, refunds.

**Mission step.** One line of a mission schedule: when to fire and which sensor list to serve.

**Oracle.** `d1-oracle`, which runs the client's own encoders offline so our payloads match the game's
byte for byte.

**Profile push.** A server-sent update of the character's profile (gear, currencies, flags), so the
client sees a change at once.

**QueueZ.** How the services deliver RSAT objects in families (roster, character, account and
profile, inventory), first as a baseline and then as deltas.

**RSAT.** Bungie's bit-packed object format, used on the wire and in saved state.
[`notes/rsat-blam-crosscheck.md`](../notes/rsat-blam-crosscheck.md) checks our codec against an
independent one.

**Run (sense97, sense104...).** A named lab run. Its evidence lives with the run, and the notes cite
it by name.

**Sensor.** Anything in a world the host and client keep in sync: squads, triggers, props, doors,
objectives, performances.

**Sensor list.** The host's full description of a world's sensors and their states, one row per
sensor: `GROUP KIND INDEX has_a has_b [TAG HEX]`.

**shadNet.** shadPS4's replacement for the PlayStation Network, based on RPCS3's RPCN. Our server
for it is `shadnet-server`.

**shadPS4.** The PS4 emulator the game runs on. The project's fork is
[d1reimagined-shadps4](https://github.com/p3n1cillin/d1reimagined-shadps4) (branch `d1`).

**Slice set.** A loaded part of a world. The client reports which one it is in (udt 21); in A
Guardian Rises, 24 is the Breach and 96 the Divide.

**SOID.** A server object id: the 64-bit id of an account, character or item.

**Squad.** A group of enemies (sensor kind `0x1`). It spawns members when its state asks for more than
are alive.

**Tag.** A typed object in the game's content packages, named by a 32-bit hash such as `0x8080071E`.
Sensor states are tagged structures.

**Tiger.** The engine and its package format. See [`notes/tiger-pkg-56578.md`](../notes/tiger-pkg-56578.md)
and [`notes/charm-56578.md`](../notes/charm-56578.md).

**Trigger.** A volume the player can cross (sensor kind `0x1D`). Once armed, crossing it is
reported to the host (incident 3403), and a mission step can wait for it (`^G/0x1D/I`).

**udt (user_defined_type).** The message types of the activity-host protocol: 49 of them, such as
udt 5 (the server's sensor frame), 6 (squad sense updates), 18 (incidents) and 21 (the current slice
set).

**Unlock flag.** A server-held account or character flag the game checks (e.g. what the Tower
allows).

**World table.** The host's map from activity index to world, with how each world starts.
