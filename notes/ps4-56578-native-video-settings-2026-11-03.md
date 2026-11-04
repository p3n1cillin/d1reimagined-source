# Native video settings: content preparation (3 November 2026)

The original settings tab has no video controls. New Frame Rate and Resolution entries need
definitions, UI bindings, labels and native getters/setters. The fork's F10 panel is usable
separately; it does not establish that these native entries work.

## What is established from owned 56578 content

- The existing `d1-scenarios.py` already decodes the shifted English string libraries. The
  settings library identifies Helmet, Brightness, Screen Bounds and the other original entries.
  A new string decoder is not needed.
- The options-definition class is `0x80802036`: two root arrays, 40-byte group and setting
  records, 8-byte self-relative setting references and 16-byte choices. Groups sort by authored
  hash; settings sort by id. An authored group hash cannot be recovered by simply hashing its
  displayed English label.
- The native single-setting component uses byte ids. Its current getter handles the original
  ids through 20. Appending definitions alone cannot make new ids functional.
- The options-data update creates the groups referenced by UI graph properties and updates up
  to twelve groups. The input has eleven groups. A new Video group with two rows fits that
  observed limit; graph bindings still need to be added and checked.

## Candidate generator

`tools/ps4_setup/d1-cui-video.py` takes the user's decoded options tag and its identified Helmet
group as a template. It appends relocated arrays, preserves every old row and choice, derives
two spare ids from the highest original id, and prepares a Video group with six rate choices
and three resolutions. Its labels are project-authored. Output contains game data and stays
outside the checkout. It refuses to replace an existing output directory.

    python tools/ps4_setup/d1-cui-video.py <decoded-options-tag> --template-group <authored-hash> --out <private-new-directory>

The owned-input check produces twelve groups and twenty-three settings, including new ids 21
and 22. The generated manifest says `installed: false` and `native_ui_verified: false`.
Synthetic tests cover relocated group references, old choice preservation and corrupt bounds.

With `--graph <decoded-settings-graph>`, it also prepares a graph candidate. The existing Helmet
binding identifies its source component, named-property path and destination list. A new named
property and a binding to the next slot in that same list are appended, preserving node ids,
imports and all original bindings. On the owned input this adds binding 117 from options-data
node 236 to list node 231, slot 4. The existing and appended paths re-read successfully; the
layout and list expansion are still unverified in-game.

Optional `--string-index <decoded-index> --string-index-tag <authored-id> --english <decoded-English>`
adds the project-authored labels. Their keys follow the original maximum, preserving every old
hash's ordinal. The part, blob and entry arrays are relocated together; short control strings
outside the main text blob are preserved too. The existing `d1-scenarios.py` decoder verifies all
882 original English labels unchanged and all 15 new labels correctly decoded on the owned input.
Four synthetic tests pass, including a control string outside the main blob. This prepares English
only; other languages need equivalent content work.

The output retains exact original tags as well as candidates for runtime byte comparison. The
fork's opt-in native-video branch uses those comparisons before redirecting in-memory root arrays,
and adds getters/setters for ids 21/22 through the same config writer as the F10 panel. Original
packages remain unchanged. Native installation and interaction still require a live test.

## Native validation checkpoint

On 56578 with committed fork integration `1a58ced`, the character screen opens after fixing a
separate self-relative formatting pointer in each relocated string part. Switching to another
character tab still crashes while the native formatter reads an invalid string-part pointer.
A read-only execution trace confirms that the failing lookup returns text bytes rather than a
string-entry record. The argument and array relationship remains under investigation; this is
an observed failure, not evidence that the new settings rows work. No graph or settings-array
redirection was observed before that failure. The source and fork changes remain draft.

The later bounded trace isolated a cached language header: the failing array-head displacement
equalled the difference between that header and the one originally redirected. Fork integration
`fe52433` instead selects complete immutable candidate string roots at the library's lookup sites,
leaving resident descriptors unchanged. In fresh run `native-video-j`, both Inventory and Settings
opened with readable labels and without the earlier invalid-entry fault. This is an isolated
native pass for that transition, not completion of the Video controls.

The Video group was still absent. Read-only checks confirmed that the extra graph binding and
settings rows were resident. Runs `native-video-k` and `native-video-l` redirected the graph during
property application at `a8b2de` and left the Settings buttons blank. That site follows component
allocation. Integration `940fb22` moved the early hook to the allocator's first tag read at
`a8b073`. Run `native-video-n` reached Settings and confirmed the English lookup, but the buttons
were still blank and the log contained no graph-redirection marker.

The generator now also accepts `--existing-group` in place of `--graph`. In that mode the
candidate appends the two rows to an authored group and keeps its original title, member order
and native graph binding. An owned-input candidate using Accessibility retained all 11 groups
and appended ids 21 and 22 after its three original rows. Audio already has five rows. This is a
separate test of the controls through the existing native group. The companion fork can load
this form without installing either graph detour.

In run `native-video-o` on 4 November (fork integration `1f8d7aa`), the Settings tab showed
its original buttons and the Accessibility panel instantiated the new frame-rate choices.
Their titles and labels were blank. Selecting the third choice wrote 100 to the shared config
and the live engine clock showed a 10 ms period and about 100 ticks/s while in the menu.
Selecting the second restored the prior 60 target and the clock returned to 60 ticks/s.
The native panel's panning stopped before the appended resolution choices came into view.
The next test uses the existing Helmet group, which has only one original row, and logs which
native string library receives the new label hashes.

The compact placement exposes all six frame-rate and all three resolution choices. The text
trace found a missing library on the two new headings. Run `native-video-q` on `1900d5d` showed
both headings after the fork supplied the verified library. The individual choice labels were
still blank. Selecting the second and third resolution choices saved 2560×1440 and 3840×2160
while the running renderer stayed at 1920×1080. Fresh run `native-video-r-4k` on `683dd78`
registered 3840×2160 buffers and rendered the full gameplay scene using the saved native
selection. The screenshot was captured at the window size.

An optional `--group-title Display` changes the compact group's label while preserving its
authored graph identifier and existing Helmet row. Six synthetic tests pass, including that
preservation. Pair `--existing-group` with `--group-title Display` and supply the English
inputs described above to reproduce this placement.

The choice labels use a different path from the headings. A bounded native trace found that
the choice widget reaches its string-library registry lookup with the new hash and receives
the missing-library sentinel. The fork now supplies the verified English library only for
the new hashes when that lookup misses. Original labels still use their native lookup.

Fresh run `native-video-u-display` on 4 November (fork integration `874fc4d`) opened the
native Settings → Display panel. Helmet and its original labels remained available. All six
Frame Rate labels and all three Resolution labels were readable. Each frame-rate preset was
selected and saved. The engine clock measured approximately 30, 60, 100, 144 and 240 ticks/s
at the corresponding capped settings while the menu was open. Uncapped saved the automatic
target value of zero. These are menu engine-clock samples. The running render resolution
stayed at 1920×1080 during the preset test, and the test ended with the prior 60/1080p config.

The fork also checks `d1-native-video` under its user directory when D1 Video is enabled and
`D1_NATIVE_VIDEO_DIR` is unset. Install the generated directory there to use the controls on
ordinary launches. Keep all original and generated tags private. On integration `98542a7`,
run `native-video-v-installed` opened the native panel with only that installation present,
switched 30 → 60 live, and saved 1440p while running at 1080p. Fresh run
`native-video-w-1440p` registered 2560×1440 buffers and drew the complete gameplay scene.
The saved 1440p selection appeared in Display after the reboot. Selecting 1080p restored the
prior saved 60/1080p config through the native panel.

## Remaining work

The validated placement uses the authored Helmet group. The optional graph path for a separate
Video group remains experimental, and new labels have only been prepared for English.

The owned input was inspected using the existing project decoder and tiger-pkg tools. See
[the third-party attribution](../THIRD_PARTY.md); no third-party code or extracted tags are
distributed with this change.
