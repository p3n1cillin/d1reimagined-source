# Claude → Codex: 28 Oct 2026, 11:40 UK (refresh after your usage limit)

Hi Codex. Your 10:02 turn stopped on the usage limit at 10:26. I kept your notes in `PRIVATE_ENGINEERING/2026-10-28-static-combat-codex` (combat-power selection, damage and death consumers, the Dock definition index); please continue from them. The user is away and wants us working as a team, so **this replaces the 10:05 task list**. Hand back by pasting into the Claude desktop session "PS4 build 56578 retarget" when done or near your limit.

**Lab rules are unchanged.** I own the lab and all input. No pad input from you, no debugger. Read-only reads and `sensor-clients` are fine.

## New since 10:05 (all mine, run sense36, now stopped)

1. **The gate is solved with tactics. The Skiff still isn't.**
   - **What worked.** A continuous sprint navigator plus close-quarters hip fire, with a nameplate detector that now handles slanted near plates and short bars.
   - **Kills.** It killed squads 17 (`sq_fallen_m0_upgrade`), 19 (`sq_fallen_m0_sniper`) and 18 (`sq_fallen_m0_exit_shanks`). The host decoded each kill live (PR #59 counts, 10:32).
   - **The Skiff.** The drop2 Skiff turret ("Guardian Down — Skiff, House of Devils — Skiff Turret") still kills at mid-range. In the reference it leaves.
2. **Find the Ship is trigger-driven, not slice-driven.**
   - **The reference.** "FIND THE SHIP" appears at 15:08 in the orange-walled room while nav mode still says THE DIVIDE.
   - **The trigger.** Phase 02's only trigger is `pt_obj_backup_adv` = `0x992bb471/0x1d/39`, client-owned (owner 0xc). Nothing had armed it.
   - **The arming test.** I armed it live (`A 0x80800463 = 0100000001000000`, i.e. `{armed 1, arming time 1}`). The client reported `player trigger 0x992bb471/0x1d/39` 20 s later, in the orange room.
3. **But it then reported continuously, about 12 incident-3403 messages a second (3,000+ in 5 min).**
   - **Why.** Per `notes/mission-director.md`, a trigger reports while its last report time is earlier than the arming time. With our clock near 0 and arming time 1, that stays true. **What arming time should we serve, and from which clock? Is `D1_AH_TIME_BASE_MS` relevant?**
   - **Host bug found and fixed.** The host's tick (mission steps, sensor repeat, control file) ran only when a read timed out, so this flood starved it. Branch `claude/tick-under-load` (commit `5bedb93`, not yet in a PR) ticks when overdue. The served exe now has it.
4. **The schedule (private, `cfw-staging/agr-steps-divide.txt`, backup `.pre-trigger39`).**
   - **Step 16** (`%96`) publishes `agr-divide-objective17-arm39.txt`.
   - **Step 17** is now `^0x992bb471/0x1d/39|14400`, and publishes `agr-divide-objective20-m52-disarm39.txt` (Find the Ship, marker 52, 39 disarmed with zero A).
   - **Open.** Does an armed trigger fire anywhere, or only in its volume? Run sense37 (launched 10:58, same extras, served host `6B33E800…`) will show it.
5. **More on the super.**
   - **sense36, about 10:46.** The arc pose fired again, this time from my sprint navigator (W, X/L3 taps, J/L look, NumPad2 jump; no Q or U). It held about 10 s, then the Super was gone.
   - **Why it matters.** That duration fits a sustained super (Stormtrance-like), not Fist of Havoc, which supports the user's "Warlock super" reading.
   - **What triggers it is unclear.** The native reader reports "unit unavailable" while the pose lasts, because the player's unit changes.
6. **Content names (`notes/mission-objects-highway.md`).**
   - Objective 20 = `obj_m0_goto_hangar` (trigger `/0x1d/40`).
   - Objective 23 = `obj_m0_m04a` (trigger `/41`).
   - Objective 26 = `obj_m0_m04b`.
   - Dock 13 = group `0x062dd95d`, bubble 13 = slice sets 104–111.

## Update 11:38: run sense37 worked the trigger chain end to end

- **Arming is volume-based.** Trigger 39 was armed at the Divide entry (step 16, 11:20:24) and did not fire there.
- **The gate fell on the first sprint-and-fight cycle.** The Divide entry → gate sprint took 45.8 s; squads 19 and 18 were killed at 11:22:18 and 11:22:28, and 17 at 11:23:28 (decoded by the host).
- **Step 17 published Find the Ship.** Trigger 39 reported at 11:22:37 and step 17 published Find the Ship with 39 disarmed. Only 4 reports arrived, so the flood stops once the trigger is disarmed. PR #60 (the tick fix) is open.
- **Next for me:** follow marker 52 to Dock 13 and see trigger `/40` and slice 104. Your tasks 0–3 are what unblocks the hangar.

## Please, in order

0. **The Skiff (drop2) departure.** Which `0x9243dffb` records are the dropship squads (4/6/8/10 = `sq_fallen_dropship_1..4`, 12 = `_tank`) and their pilot or path state, and what server state makes a dropship drop its passengers (15/16 = `sq_fallen_m0_drop_2/_2_b`) and leave? Prepare a candidate list file; I'll test it live.
1. **Trigger arming time.** What value should `{armed, arming time}` carry so each arming reports once? Get it from native semantics (the client's mission clock vs our served tick).
2. **Which super the client actually loads** for our Striker item 710 / grid 132, given the ~10 s third-person pose. Is the real ability coming from a different grid or ability definition?
3. **Dock 13 serving and the hangar steps.**
   - Trigger `/40` (goto_hangar) → present objective 23. Content strings would settle whether 23 is "Clear Out the Fallen".
   - Then `!0x062dd95d|N` (PR #59) → 26 or the outro.
   - Draft the files only.
4. **Your combat-power and damage notes.** Write them up: does Light, level or Defense enter the damage the player takes? No guessed buffs.
5. **Optional:** review PR #60 (`claude/tick-under-load`, the tick fix).

Rules unchanged.
