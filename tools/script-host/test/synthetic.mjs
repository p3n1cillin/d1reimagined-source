// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//
// A small script project written for these tests, in BullShcript's
// BungieScript dialect (its LANGUAGE.md and NATIVES.md). None of it comes
// from any game or reconstructed script: the activities, rows and names are
// made up, apart from the engine's own native and type names.

import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";

export const SYNTHETIC_FILES = [
  {
    path: "bs/activities/common/story_common.bs",
    text: `@activity_bundle("story_common")
class c_encounter__story_common : c_encounter:
    lifetime lifetime
    timer lifetime_timer
`,
  },
  {
    path: "es/utility_scripts/test_cutscene.esu",
    text: `cinematic _cinematic

def play():
    _cinematic.start()

def done():
    return _cinematic.is_done()
`,
  },
  {
    path: "es/testland/test_cine/test_cine.es",
    text: `@activity_bundle("test_cine")
@activity("test_cine")
@start_slice("test_plaza", "test_phase_a")
@phase("test_phase_a", "test_plaza")
@phase("test_phase_b", "test_yard")
test_cine_mission1 test_phase_a
test_cine_mission2 test_phase_b

@startup()
script ss_test_cine():
    c_lifetime_script_utilities.get_lifetime_sensor().set_state(e_activity_lifetime_state.pregame)
`,
  },
  {
    path: "es/testland/test_cine/test_cine_mission1.esp",
    text: `test_cutscene cut_a

script init():
    pass

script complete():
    pass

script main():
    cut_a.play()
    sleep_until(cut_a.done())
`,
  },
  {
    path: "es/testland/test_cine/test_cine_mission2.esp",
    text: `test_cutscene cut_b
player_trigger pt_exit
@member_counts({grunt: 2})
squad sq_guard

script init():
    c_lifetime_script_utilities.get_lifetime_sensor().set_state(e_activity_lifetime_state.active)

script complete():
    c_lifetime_script_utilities.get_lifetime_sensor().end_game()

script main():
    set_activity_complete()
    cut_b.play()
    sleep_until(cut_b.done())
    sq_guard.place()
    pt_exit.enable()
    sleep_until(pt_exit.is_triggered())
    sleep_until(sq_guard.alive_count() == 0)
`,
  },
  {
    path: "es/testland/test_odd/test_odd.es",
    text: `@activity_bundle("test_odd")
@activity("test_odd")
@start_slice("test_field")
squad sq_odd
trigger_volume tv_odd

@startup()
script ss_test_odd():
    tv_odd.import_only_clear_slice_set_assignment()
    sq_odd.frobnicate_for_test()
    c_lifetime_script_utilities.get_lifetime_sensor().set_state(e_activity_lifetime_state.active)
`,
  },
  {
    path: "es/testland/test_stub/test_stub.es",
    text: `@activity("test_stub")
@start_slice("test_field")
`,
  },
];

// The project written to a fresh temporary folder (for the sidecar's
// scripts_dir); call the returned cleanup when done.
export function writeSyntheticDir() {
  const dir = mkdtempSync(join(tmpdir(), "d1-script-host-"));
  for (const { path, text } of SYNTHETIC_FILES) {
    const at = join(dir, path);
    mkdirSync(dirname(at), { recursive: true });
    writeFileSync(at, text);
  }
  return { cleanup: () => rmSync(dir, { force: true, recursive: true }), dir };
}

export const SYNTHETIC_TSV = [
  "index\tname_hash\tname\tpayload_selector\taux",
  "0\t0x0046d7ee\tplayer_left\t3\t65535,65535,65535,65535",
  "7\t0x12345678\t\t0\t65535,65535,65535,65535",
  "965\t0x42660690\tactivity_complete\t0\t65535,65535,65535,65535",
  "1200\t0xd4a3f1b2\ttest_pickup\t0\t65535,65535,65535,65535",
  "",
].join("\n");
