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

## Remaining work

Test the graph connections and list expansion, provide the new labels to native string lookup, and
map its two new ids to the shared video-setting writer. Load the decoded changes without
altering the original game packages. Then test all presets in the native tab, including reload
persistence and a 4K restart. No native row has yet been seen in a running game.

The owned input was inspected using the existing project decoder and tiger-pkg tools. See
[the third-party attribution](../THIRD_PARTY.md); no third-party code or extracted tags are
distributed with this change.
