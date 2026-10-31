# Handoff to Codex — 27 Oct 2026, 07:50 UK

PS4 build 56578 (01.33), A Guardian Rises. The route now runs from spawn **out of the Wall into
the Divide** (slice set 96). Everything below is merged to `d1reimagined-source` main
(PRs #28, #29, #30).

## Where things stand

- **Run 11** (`2026-10-27-route11`, started 06:16) may still be open. The Guardian was left in the
  Divide at about (349.7, −648.6). Nothing has pressed buttons since 07:45, so it has probably been
  kicked to Orbit by now. Start a fresh run rather than relying on it.
- **Full replay, spawn to the Divide (about 25 min, unattended).** The script is
  `route11.sh` / `route11b.sh` in the old session's scratchpad. The chain is: `d1-walk.py --scan`
  at spawn, then the routes `breach.json` (with `--avoid 330,-837,340,-812`),
  `breach_platform.json`, the rifle point, `to-chest`, `mine-a/b/c` (crouch between them),
  `wall_junction_west.json`, then publish props 3/7/8, then `wall_garage_door.json` and
  `wall_fan_tunnel_to_divide.json`. If the Breach walk stalls among the cars at waypoint 20,
  restart it from waypoint 14.
- **Garage door.** It opens when Breach props 3, 7 and 8 (`0xfc0e45a3:0x4`) get the activation
  image: `clients 0x4786c0e0 F:/d1/d1preservation-main/cfw-staging/agr-b-props-378.txt` in
  `cfw-staging/ah-control.txt`. Which of the three is the door is not known yet.
- **The Divide = slice set 96.** It loads group `0x9243dffb`: 12 squads, 6 squad members,
  10 triggers, 3 props, and kinds 0x3, 0x6 and 0x25. The local machine there is **12**, not 3 as
  in the Breach.
- **Authority (PR #30).** A control-file list can carry `@authority MACHINE[,SEQ[,APPLIED]]`. The
  list `cfw-staging/agr-s96-clients-auth.txt` (`@authority 12,2,0`) cleared all 37 pending Divide
  clients. Publish it after `client is now in slice set 96` appears in `activity-host.log`.
- **Seeing with the monitor off.** Launch with `D1_DUMP_DIR=F:\d1\tools\shadPS4\dump-live`, then run
  `tools/ps4_setup/d1-scene.py OUT.png`.

## Next tasks, in order

1. **Fight Through the Fallen (timeline row 7).** Give the Divide's squads (`0x9243dffb` kind 1)
   spawn states with `d1-squad-states.py`, as was done for the Breach. Squad state `0x8080071E`
   needs +0x48/+0x4C/+0x78 = `0x811C9DC5`. Publish with the `@authority 12,2,0` line kept. Watch
   for Dregs, Vandals and Shanks in the container yard.
2. Find out which of props 3, 7 and 8 is the garage door, and open it from the end of wave 3
   instead of by hand.
3. The Guardian dies to two Dregs within seconds, and the rifle arrives with 0/0 ammo, then
   12/114. Check health and the starter ammo grant.
4. The Fallen ship flyover and the "Fallen ships!" line (row 6). The Divide is in daylight while
   the reference shows night: check `D1_AH_TIME_BASE_MS`.

## Rules

- `d1reimagined-source` main is PR-only. Branch from the latest `origin/main`, open a PR (squash
  auto-merge runs once checks pass), and fill in the PR template.
- `d1reimagined-shadps4` branch `d1` takes direct pushes. Keep marketing commit `1a6dd17` on top.
- Commit as `p3n1cillin` (`<old-email>`).
- No third-party downloads. No attempts to get or break Sony retail keys. Game files, keys and run
  evidence never go in any repo.
- Build the AH with `LIBCLANG_PATH=F:\d1\tools\llvm-23.1.3\bin`. A running host locks its exe:
  stop the run before rebuilding. The lab tree `F:\d1\d1preservation-main\d1-server` is not a git
  repo, so mirror changes to `ps4/d1-server` in the public repo.
- Kill helper processes with a PowerShell process filter. A bash `pkill` pattern can match its
  own shell.

Full notes: `notes/ps4-56578-continuation-2026-10-27.md` (public repo).
