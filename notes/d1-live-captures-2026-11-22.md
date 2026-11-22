# Live captures, third batch (14 to 20 November 2026): coverage

A Discord member who shared the earlier batches recorded 19 more captures of Destiny 1 on Bungie's live
servers, on an Xbox 360 (build 51539, the same plugin). They stay private; this note records what they
cover and what they can check in the build 56578 work. Values read from them are test targets for 56578,
not constants.

## Coverage

| Kind | Activities |
|---|---|
| Strikes (7 captures) | Fallen S.A.B.E.R. (two runs: one failed when the timer ran out, one complete), Shield Brothers, The Sunless Cell, Blighted Chalice, Echo Chamber (incomplete: stopped before the boss died) |
| Taken War quest missions and one-off missions (9) | Kings of Decay, Paradox (one complete run, one partial), Entropy's Pinnacle, Tenebrous Tunnels, Fear's Embrace, Outbound Signal, Summoner's Circle, Pretender to the Throne |
| Patrols (3) | Venus (a Taken Vex quest step), Mars (Taken Champion bounty and a public instance shared with another player), the Dreadnaught (the step that unlocks Blighted Chalice) |
| Tower (1) | a 100-second visit during the Sparrow Racing League event |

Notes on the set:
- The file named "Blighted Coven" is a complete Kings of Decay run, and the file named "Kings of Decay" is the first five minutes of a Paradox run.
  There is no Blighted Coven capture.
- Every strike, quest and mission capture carries the host's activity-complete event except the failed S.A.B.E.R. run, the partial Paradox run and Echo Chamber. Patrols and the Tower have no such event.

## What they check, and the result

- **World identity.** For every activity that has a world-specific record group, our 56578 world table's group for that world is among the groups the live host served.
- **Records.** Over a whole mission, every record the live mission host served is present in our generated world's bundle table, in 18 of 19 captures. The exception is the Tower plaza, where ten
  records (the civilians' index choices) differ from ours.
- **Start slice sets.** 17 of the activities in this batch had a stand-in start region in our table (the first private slice set). The live client's first slice set is now known for each.
  The two patrols already cross-checked (Venus, the Dreadnaught) agree. Every slice set the live client reported exists in our table, and every region the live host
  advertised as public is public in ours.
- **First full list.** Where the start is a private slice of a self-contained mission (Fear's Embrace, Entropy's Pinnacle) our generated list matches the live one exactly or nearly so.
  In the strikes and longer missions the live host's first list also contains records from other slice sets of the same world; the grouping, not the record set, is what differs.
- **Public instances.** 27 of the 30 public starts in regions we already have a fixture for contain the whole fixture. Two regions of the Dreadnaught (152 and 72) have no fixture yet and
  can now be built.
- **Ambassador release timing.** The initial release of a region's matchmaking flag comes about 6 seconds after the first publish when the client joins another player's public
  instance, and about 10 seconds when it starts its own. This refines the earlier note's observation (confirmed against six earlier captures).
- **Multiplayer.** Two joined-instance runs (Shield Brothers, Patrol Mars) and a third (Patrol Venus) show the second peer's entity range, the occupancy and simulation masks, and that the
  client sends its incidents to the host of the bubble it is in.
- **Strikes.** The failed run shows the host's wipe incident followed by a full list sent twice; the complete run shows the objective records stepping through the mission, and the
  reward items arriving seconds before the activity-complete event.
- **Tower.** The Sparrow Racing League event adds one record group to the Tower's first list that our table serves only in another slice set.

## Open after this batch

- Which extra slice sets the 56578 client asks for in a strike's first list (needs a lab run).
- The timer and Warsat-failure state of S.A.B.E.R., and which reward item is the Sleeper part (item numbers do not carry over between the builds).
- A real Blighted Coven capture.
