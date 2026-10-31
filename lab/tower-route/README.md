# The unattended Tower route (56578)

One command plays A Guardian Rises from boot to the Tower with nobody at the pad. It boots, replays
the opening, fights through the Divide, the gate, Dock 13 and the hangar, then publishes the
post-game screens. It records the post-game tour, holds CONTINUE and lands in Tower North, where
the vendors sell.

    pwsh lab/tower-route/ending-run.ps1 -Name sense74

This is how the lab's runs are made. The defaults recreate run sense73 (31 October 2026).

## What it needs

- The lab set up as in `tools/ps4_setup/README.md`:
  - the project's emulator build (`tools/ps4_setup/shadps4-0.19.0-*.patch`);
  - your own 56578 copy;
  - the servers built from `ps4/d1-server` in the workspace's `d1-server/target/release`;
  - `D1_ROOT` (or the other `D1_*` locations) set.
- The served fixtures in `ps4/fixtures/56578`.
- The probes that read the executable statically (`static56578.py`) need your own decrypted
  `eboot.elf` (`D1_EBOOT`).
- Python with `numpy`, `Pillow` and `capstone` (`requirements.txt`), Git Bash, and the emulator window
  kept in front: input only reaches the focused window.

## The pieces

| Script | Role |
|---|---|
| `ending-run.ps1` | Starts `tools/ps4_setup/d1-agr-run.ps1` with the route's server settings, then the jobs below. |
| `chain2.sh` | After the replay reaches the Divide: walks (`nav.py`) and fights (`combat2.py`) through the gate, Dock 13 and the hangar until the mission's last squad is down. |
| `postgame5.sh` | Pushes the profile at the right moments (XP, completion, the Arcadia), then the scoreboard and phase 7, which bring up the post-game screens. |
| `capture51.sh` | Records from Dock 13 through the post-game tour. |
| `continue-hold.sh` | Checks the recall gate, then holds CONTINUE and watches the departure to the Tower. |
| `combat2.py`, `fight.py`, `plates.py`, `health.py`, `read_vitals.py` | Find live enemies (nameplates, squad memory), aim and fire. |
| `nav.py`, `run.py`, `follow.py`, `seek.py`, `target.py`, `marker.py`, `release.py` | Walk the Guardian over positions it has walked before. |
| `alive.py` | Reads live squad counts from the activity host log. |
| `gamefocus.py` | Keeps the emulator window in front for input. |
| `abilities-check.py`, `tower-head-check.py` | Screen checks of a running lab: the ability icons (#134), and the Guardian's head in the Tower in the normal view and the wave emote (#128). |
| `probe-*.py`, `monitor-recall-departure-readonly.py`, `attach-readonly.py`, `static56578.py` | Read-only probes of the running client (and of the executable). Output goes to `STAGING/probes`. |
| `routepaths.py` | Where all of the above find the repository, the fixtures and the staging folder. |

Every loop that drives the game also moves a stick: an idle Guardian is sent back to orbit.
`tools/ps4_setup/d1-idle-guard.sh` covers the waits.
