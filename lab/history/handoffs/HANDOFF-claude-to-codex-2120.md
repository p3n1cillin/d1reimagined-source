# Claude → Codex: 29 Oct, 21:20 UK — hangar diagnostic result (sense59)

For when you're back (23:29). All evidence is in `cfw-staging\sense59\` and your probe outputs.

- **Advancement probe** ran with ADVANCEMENT selected at about 21:05 (output beside your script; `upgrade_count` 0). Screenshot: `sense59\capture\advancement-selected.png`. Please interpret the body tag and visibility path.
- **CONTINUE → 165 → hangar:** every milestone fired (countdown 21:05:10.9, handler31/goal27 21:05:14.1, BAP42 hash165 21:05:17.2, follow, AH publish, `ah_world_row165`, join 21:05:19.0). The departure monitor shows the world as **0x80cb802b / act_cosmo_m0_5_hangar / 31429b32 / e354f1ea / 22 bubbles**.
- **Your cue probe** (`hangar-cues-sense59-1793307906339523100.summary.json`): `saw_expected_hangar` true, `saw_hangar_world_state36` false, no accepted cinematics. Its last sample has world_state **34** and a sensor pool with **0 clients, 0 pending, no lifetime/cinematic names**, and no component rows.
- `d1-winguest sensor-clients` at 21:08: "local id -1", 0 pending. **Nothing to publish.** Unlike the Tower stall (100 pending), the hangar never registered a single sensor client.
- Host: join, udt 17/21/36/20, udt-5 every 10 s, and no udt 6 from the client. **The client went silent around 21:10:30 and the connection aborted at 21:11:36 (10053)**; the screen then showed "Removed from the game world… baboon". That's the same signature as sense58 at 20:12.
- Reading: as served (region 0, no cues), the hangar world loads its scenario but never instantiates its sensor world. Whether that means "not 165's world" or "needs a host-side start/region/cue we don't send" is open. I'm leaving the hangar there; **165 → Tower stays the stand-in.**

Next: sense60 runs 0 and 165 → Tower with your **authored-slot list** (a fresh join), plus `social-keepalive.sh` after arrival, for a populated-Tower capture.

Open for you: the baboon signature (client silent, then a host-side 10053 about 65 s later; seen twice tonight); the Advancement probe output; and whether the hangar needs a different start region or cue.

## 21:40 — Advancement settled from the live tree (sense59, your probe, ADVANCEMENT selected)
- Live root: a screen widget with definition **0x80aa2f14** (node 0). Its post_game_data is node **1232** (identity d1…01/d1…0201/4, version 5, level 1→2, gained 900). The wrapper 0x80aa2ed1 still holds node 120.
- I extracted 0x80aa2f14/0x80aa2f15 (and 0x80aa93c3) with your `extract-owned-entries.exe` (needs D1_EBOOT, D1_BUILDS and D1_PKGDIR=…\CUSA00219\packages). In graph **0x80aa2f15**, node 1232 has exactly **one** output: `0x18 → 1186.0xa07` (equipment-upgrade rows → list item source), the same as your default-body result. **No character-XP binding in the selected body either.**
- The only visible progress widget outside the body is wrapper node **54** (`c_progress_indicator_widget`, a8 0x406). Its chain: import 0x80aa39b5 → 0x80aa93c3, where bar node 2 ← decision makers 33/34 (`0x402→0x600/0x602`) ← node 25 props 0x206/0x208. In the wrapper that is node **77 = `c_orbit_recall_handler`**. **So the visible bar is the CONTINUE hold ring, not an XP bar.**
- Conclusion: in this build's post-game layout nothing displays character XP. The 900 gain is recorded and correct, and "Upgrades: NO RESULTS" is the empty equipped-item list. I consider the Advancement question closed unless you know of another body that shows a character bar.
- The progress-widget table with the ancestors' a8 bit 1 is in `sense59-advancement-progress-widgets.txt`. My "bit 1 = visible" reading there is a hypothesis only.

## 22:35 — sense60: your authored slots on a fresh join
- CONTINUE 22:11:10 → Tower (80c7a005). Within 12 s the host logged "live members 0 -> 1" lines and a SquadReader snapshot matched your vector exactly: 2:1, 4:1, 6:1, 8:2, 11:2, 15:1, 17:3, 21:1, 23:2, 27:1, 30:1, 34:1, 36:1, 38:1, 41:1 = **20 members**.
- On screen while walking: civilians on the walkway and at the railing, **Patches 99-40** by the stairs, and **"Xûr, Agent of the Nine"** at the round door (the black_market vendor_door slot), with an Approach prompt. Recording: `sense60\capture\tower-walk.mp4`; stills in `capture\walk\`.
- I delivered a late NAS package with the caveat that the counts and selection are reconstructed (Xûr being present is an artifact of enabling that slot).
- sense60 is parked in the Tower with `social-keepalive.sh` running, so you can read it read-only when you're back.
- 22:30: still **0 pending** after following the quest marker for 3 min (it got stuck in a corner, marker 66 m). The plaza (8cc31436) and hangar (19d43443) groups haven't registered; they need the Guardian to actually reach those regions. Next session: a route into the plaza, then read and merge that region's pending rows into the 40aebf90 list.
- **Correction, 22:45:** `social-keepalive.sh` does NOT prevent the Tower → orbit kick. sense60 was returned to orbit at **22:26:15** (peer_leave_request from the client), about 2 min after the keep-alive restarted and right after ~3 min of real walking. Arrival was 22:11 (15 min); sense57 was kicked 35 min after arrival. Likely `error.booted_from_private_activity_host_inactivity_timeout`: the client deems the *activity host* inactive. What does the client expect from a private AH in a social space (a periodic udt, incidents, a lifetime phase)? Notes corrected in PR #91.
- Public "Live from the lab" notes are now live (owner-approved): tower-walk and orbit **working**, tower-vendors and tower-cinematic **testing**.
- **Correction to the correction, 22:55:** the peer_leave payload's last word is the reason hash. Both kicks carry **0x4ca36d9a = `error.inactivity_timed_out`** (the error-name table at 0x785650 stores that string with hash 0x4ca36d9a at 0x26cb060); the CONTINUE departures carry **0x03cfbb27**. So it is the client's player-inactivity timer, NOT the private-AH inactivity error. Our d1-pad input (walking, camera) evidently does not reset it. Which input or state resets that timer? PR #91 is updated.
