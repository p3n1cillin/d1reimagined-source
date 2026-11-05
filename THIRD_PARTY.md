# Third-party research tools

The native-settings investigation used locally installed copies of these projects to inspect
the user's owned content. Their tools, extracted outputs and game material are not redistributed
in this repository. The settings generator is project code derived from the native layout.

- [tiger-pkg](https://github.com/v4nguard/tiger-pkg): package inspection and extraction.
- [Blam Network tiger](https://github.com/Blam-Network/tiger): package-format reference.
- [quicktag](https://github.com/v4nguard/quicktag): tag-format research reference.
- [Ghidra](https://github.com/NationalSecurityAgency/ghidra) (Apache-2.0), run headless on a
  [Temurin](https://adoptium.net/) JDK 25: decompiling the owned eboot (`tools/ps4_setup/d1-ghidra.ps1`);
  its project, which holds the game's code, stays outside the repository.

## Activity framework sources (strikes, raids, arenas, the Crucible)

`d1-activity-host`'s `encounter_framework.rs`, `pvp.rs` and the data under
`ps4/fixtures/56578/activities/` (notes/ps4-56578-strikes-raids-crucible-2026-11-05.md) drew on:

- [BullShcript](https://github.com/Blam-Network/tiger/tree/main/packages/bullshcript) (MIT, Blam
  Network): the scoreboard row's semantics (`scoreboard_natives.ts`, `state/items.ts`), reimplemented
  in Rust in `pvp.rs`, and the hard-wipe rules (`LANGUAGE.md`). No code was copied.
- Fukugen Daihon (Blam Network's restored 56578 mission scripts; no licence): read locally only.
  The definitions carry identifiers from it (activity, bundle and phase names, regions,
  `@hard_wipe` flags), never script text.
- [The Cryptarch guide](https://github.com/nruble/cryptarch-guide) (GPL-3.0): its raid loot tables
  (encounter titles and item hashes), labelled `community` wherever used.
- Bungie's Destiny 1 manifest, as [DIM](https://github.com/DestinyItemManager/DIM) distributes it,
  and the mobile world database: a local input to `tools/ps4_setup/d1-activities-export.py`.

Each upstream project retains its own licence. Consult that project's licence before copying
or redistributing its implementation; these links do not grant additional rights.
