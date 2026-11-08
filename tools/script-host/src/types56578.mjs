// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//
// What build 56578 adds to BullShcript's 01.29 (v52553) model, in our own
// words and numbers. Each entry says where its evidence comes from; an entry
// with none is a stand-in and is labelled so.

// 01.29's row type ids plus the one our 56578 sensor lists use that 01.29's
// table does not name. The name ref's i8 in our lists (1 squad, 2 actor,
// 4 object, 5 sequence, 6 cinematic, 12 player, 16 lifetime, 29 player
// trigger, 30 trigger group, 32 player objective, 33 filter, 34 hard-wipe
// globals, 40 machine) matches 01.29 one for one (gap analysis §0).
export function types56578(v52553Types) {
  return Object.freeze({
    ...v52553Types,
    // Seen in our 56578 lists' binder group; BullShcript's TU0 binder table
    // names that row `new_user_experience`. No native reads it.
    NewUserExperience: 39,
  });
}

// The lifetime's state as the runtime writes it (BullShcript
// native_support.ts) to the decoded value our 56578 lifetime record carries
// (`@phase` in a sensor list). Codex's mapping (#304); the 56578 cinematic
// timeline observed 0, 1, 4, 6, 8.
export const LIFETIME_WIRE_PHASE = Object.freeze({
  0: 1, // pregame
  1: 2, // countdown: the +1 shift only, never observed on 56578
  2: 3, // active
  3: 4, // cinematic hold
  5: 6, // results (end_game's first write)
  6: 8, // ended (end_game's second write): a cinematic's 8; the host sends 7 for gameplay (#318)
});

// Only the values above are evidence. Anything else goes out as null with
// the runtime's own number beside it, so the host can refuse it.
export function lifetimeWirePhase(runtimeState) {
  const wire = LIFETIME_WIRE_PHASE[runtimeState];
  return wire === undefined ? null : wire;
}

export const RUNTIME_LIFETIME_ENDED = 6;

// A script member's native type, as a script spells it, to the row type its
// rows have (by key into the type table). Derived by joining BullShcript's
// MIT TU0 row tables (test/tu0_missions/*_rows.ts) to the TU0 scripts'
// member types by name: every pairing below held there, with the counts in
// the note. Used only to build stand-in bundles when the host gives us no
// content export.
export const NATIVE_ROW_TYPE = Object.freeze({
  action_point: "ActionPoint",
  cinematic: "Cinematic",
  control_point_set: "ControlPoint",
  encounter_binding: "EncounterBinding",
  encounter_handle: "Encounter",
  fireteam: "Fireteam",
  hop_on: "HopOn",
  lifetime: "Lifetime",
  location: "Location",
  loot: "Loot",
  nav_point: "NavPoint",
  object: "Object",
  object_filter: "ObjectFilter",
  objective: "Objective",
  passenger: "Passenger",
  performance: "Machine",
  player_monitor: "PlayerMonitor",
  player_objective: "PlayerObjective",
  player_trigger: "PlayerTrigger",
  safe_zone: "SafeZone",
  sequence: "Sequence",
  squad: "Squad",
  squad_group: "SquadGroup",
  squad_member: "Actor",
  timer: "Timer",
  trigger_group: "TriggerGroup",
  trigger_volume: "TriggerVolume",
});

// Native types with no TU0 row to pair with. Stand-ins, each logged when a
// stand-in bundle uses it.
export const STAND_IN_ROW_TYPE = Object.freeze({
  // Component hosts on a placed object: their verbs (set_device_position,
  // enable, is_captured) write ObjectState's components. 01.29 also lists a
  // device row (22) that no native reads.
  capturable: "Object",
  device: "Object",
  interactable: "Object",
});

// Native types that are host-side handles, never rows: listeners, flags,
// managers, command scripts.
export const HANDLE_TYPES = Object.freeze(
  new Set([
    "activity_flag",
    "c_command_script",
    "command_script",
    "common_reference",
    "encounter_manager",
    "incident_listener",
    "interactable_incident_listener",
    "kill_incident_listener",
    "thread",
  ])
);

// Rows a binder (bs/activities/common/*_common.bs) carries that no script
// member declares, in the order BullShcript's TU0 binder table lists them.
// Stand-in: 56578's binder layout comes from content.
export const BINDER_EXTRA_ROWS = Object.freeze([
  ["scoreboard", "Scoreboard"],
  ["hard_wipe_globals", "HardWipeGlobals"],
  ["new_user_experience", "NewUserExperience"],
]);
export const BINDER_PLAYER_ROWS = 16;
