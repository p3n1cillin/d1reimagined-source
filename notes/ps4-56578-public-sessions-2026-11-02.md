# PS4 56578: what a PUBLIC start region needs (2 November 2026)

Story missions whose start region is PUBLIC stall: The Dark Within (168) at rung 34 ("Region
'PUB00.00' is PUBLIC but not yet connected"; [blockers 14 Oct](ps4-56578-blockers-2026-10-14.md)),
and Restoration (166) starts in bubble 7, which is public ([blockers 15 Oct](ps4-56578-blockers-2026-10-15.md)).
A Guardian Rises works because all 22 highway bubbles are private. This note maps the client's
public-session path from the 56578 eboot (static analysis with `d1-eboot`; addresses are vaddrs)
and plans serving it. [V] = read in the binary or our code, [I] = inferred.

## The client's state

- [V] One writer sets a region's session state: `0x77CB60(sstm, which, state, reason)` writes
  `[region+0x11]`, region = `which ? sstm+0x130 : sstm+0x90`, so `sstm+0x141` is the pending
  region's state. The 15 Oct note's "only compared, never written" missed this indirect write.
- [V] State 2 ("public AH instance ready") is set only at `0x77E6BF`, in the "starting public AH /
  joining public AH" task `0x77E1D0`, when `0x75CBE0(kind 1 PUBLIC, which 1 TARGET)` returns status 6
  (`[ac + slot*0x1F0 + 0x4F0]`). State 1 comes from ambassador matchmaking (`0x77C832`) or a
  citizen join (`0x77DA4A`). The gate is `0x77BD47` (`+0x13C == 1`, public) and `0x77BD55`
  (`+0x141 == 2`).
- [V] The path:
  - The mission's activity host (PAH) tabulates regions in the membership and assigns
    "ambassadorship" for a region (`0x7791C7`: region record `+0x0A` == local peer and `+0x09` == 1).
  - The ambassador matchmakes, or spins up a new public activity session
    (`create_public_activity_session`, `0x772BD0`). It starts a public activity-host instance (a
    second activity-host connection) and reports the session and its AH id (`0x779E30`).
  - The PAH advertises the public session and activity host (`0x778D60`).
  - Other players citizen-join (`0x77CD20`).
- [V] Region record fields: `ps4/capture-rig/surfaces/registry.toml` 442-449 (`+0x04` advertised
  public, `+0x09/+0x0A` ambassador, `+0x2C` session lock, `+0x4C` session, `+0xD0` AH).
- [V] What we serve: the activity host's region record carries only the region id and our peer
  token (`d1-activity-host/src/messages.rs` 779-806). The ambassador and advertised fields are 0.
  [I] So the client most likely takes the citizen-join path and waits for a session that never
  comes.

## Plan, smallest first

**5 November: the live values.** A live-server capture of Restoration (`d1-live-captures-2026-11-05.md`, "Restoration's public start") shows what the original PAH served. The region record has `+0x04 = 1` for the public region, and `+0x09 = 1`, `+0x0A = 0` (set for private regions too). The public session's host fills `+0x4C` (session) and `+0xD0` (its own host id). The client matchmakes with services messages 0x0a/0x2a between the two sessions.


1. **Observe.** Launch a mission at a public region and read the client's
   `slice_set_transition_manager` lines for the task it sits in. Restoration at bubble 7 (region 56)
   is being tried in sense121.
2. **Ambassadorship.** Set the region record's `+0x09 = 1` and `+0x0A` = the local peer. Expect "PAH
   assigned ambassadorship to us" and state 1.
3. **Matchmaking.** Answer the ambassador's search with nothing found, leading to
   `create_public_activity_session`. `/v1/sessions` is already served.
   - **6 November (runs fg5b, Ghidra):** "nothing found" alone does not end the search. With
     ambassadorship, the client ran `initiate_search [PUBLIC activity]` (matchmaking type 1, set
     `0x80010038`) 43 times in 3.5 min. Its seeker's index stopped at 15, and the last search kept
     repeating. The ambassador's task `FUN_0077C3D0` [V] ends it in one of two ways:
     - the TARGET region record's **`+0x19` is non-zero** (`sstm+0x4A9`). The client logs "A player
       has reached the matchmaking threshold for region '%s', aborting and spinning up a new
       public activity session";
     - the ambassadorship `+0x09 == 2` (revoked), after a time.

     The live PAH set `+0x19 = 1` 10.2 s after publishing region 56, and the client asked for its
     new host 0.2 s later. `d1-activity-host` now sets it 10 s after the record first names a
     public region with no session reported (`D1_AH_PUBLIC_THRESHOLD_MS`). That is a stand-in
     trigger: in region 48 it went to 1 as the player arrived. It also takes the public regions
     from the world's `slice-sets.tsv`.
4. **The public instance.** Serve a second activity-host instance per public session: its own
   membership, a disjoint entity-index range, the world's PUBLIC checksum and bubble count, the public
   activity index, the public bubble as its start region, and that bubble's sensor clients, all built
   with d1-oracle codecs. Its ready state (status 6) should set `+0x141 = 2`.
   - **Live (5 Nov):** the public instance is a second activity-host connection beside the PAH,
     which stays connected. It presents its own host id, and its join request names the PAH's.
     It serves its own sensor groups (in Restoration `0xEAAF16E2`, the 16 player objects, and
     four others) and no region records. Each connection presenting its own id is
     `D1_AH_DISTINCT_HOST_IDS=1` in `d1-activity-host`.
5. **Advertise.** Echo the reported session and AH id back in the PAH's region record (`+0x4C`,
   `+0xD0`, `+0x04 = 1`). Otherwise expect `public_activity_host_mismatch` or "has no AH ID".
   - **Live (5 Nov):** the client reports both in its udt 21 slice-set record (session at
     +0x14, host id at +0x98, state +0x11 = 2 once ready), and the PAH copies them unchanged.
     `d1-activity-host` does the same (`track_public_session`).

## Risks

- ~~Two hosts handing out the same entity indices.~~ Not a risk: live, each host has its own index space and granted the client 0-499 by its peer slot (`d1-live-captures-2026-11-05.md`).
- Shared state in one activity-host process.
- The ambassador's search may need a matchmaking service we don't run.
- The client's timeouts (minimum matchmaking time, the citizen-join timeout, a 15 s re-tick at
  `0x77BD6D`).
- New sensor lists for the public bubbles.
- Status 6's meaning is inferred from the private path.
