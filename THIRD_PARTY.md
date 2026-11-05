# Third-party research tools

The native-settings investigation used locally installed copies of these projects to inspect
the user's owned content. Their tools, extracted outputs and game material are not redistributed
in this repository. The settings generator is project code derived from the native layout.

- [tiger-pkg](https://github.com/v4nguard/tiger-pkg): package inspection and extraction.
- [Blam Network tiger](https://github.com/Blam-Network/tiger): package-format reference. Its MIT
  `packages/rsat` codec was an independent cross-check of our RSAT primitives
  ([notes/rsat-blam-crosscheck.md](notes/rsat-blam-crosscheck.md)). The native codec
  `ps4/d1-server/crates/d1-rsat` adapts no code from it: it is written from the owned eboot's own
  descriptors and walkers, and tested against `d1-oracle`.
- [quicktag](https://github.com/v4nguard/quicktag): tag-format research reference.
- [Ghidra](https://github.com/NationalSecurityAgency/ghidra) (Apache-2.0), run headless on a
  [Temurin](https://adoptium.net/) JDK 25: decompiling the owned eboot (`tools/ps4_setup/d1-ghidra.ps1`);
  its project, which holds the game's code, stays outside the repository.
- [Destiny Item Manager](https://github.com/DestinyItemManager)'s archive of Bungie's final D1 API
  manifest: a local input to `tools/ps4_setup/d1-progression-tables.py`. The manifest is Bungie's
  data and is not committed; only the derived XP, rank, reset and refund tables in
  `ps4/fixtures/56578/progression/` are, each citing its definition. The same archive feeds
  `tools/rewards_tables/gen_rewards_tables.py`, whose derived tables (item tiers and buckets,
  progressions, talent grids, activity rewards, records, objectives) are in
  `ps4/d1-server/crates/d1-responder/src/rewards/tables/`, each citing its input's SHA-256.
- [nruble/cryptarch-guide](https://github.com/nruble/cryptarch-guide) (GPL-3.0): its copy of
  Bungie's Grimoire definitions (the Grimoire card points and stat ranks), and its community-collated
  engram contents and per-activity Legendary Mark limits, read locally by
  `tools/rewards_tables/gen_rewards_tables.py`. Only hashes and numbers are kept, in
  `grimoire.tsv`, `engram_pools.tsv` and `mark_limits.tsv`; the latter two are labelled as
  community stand-ins. None of the guide's text is copied.

## The mission-script sidecar (`tools/script-host/`)

- [BullShcript](https://github.com/Blam-Network/tiger/tree/main/packages/bullshcript) (MIT, Blam
  Network; `"license": "MIT"` in its `package.json`), the BungieScript compiler and VM by Codie.
  The sidecar imports a local, built checkout in place (`D1_TIGER_DIR` or `D1_BULLSHCRIPT_DIR`;
  checked at tiger `82c57cb`). None of its code is copied into this repository. The sidecar's
  native-type to row-type table was derived from BullShcript's MIT TU0 test tables, and its
  command-script order names from BullShcript's `command_script.ts`.
- [Fukugen Daihon](https://github.com/Blam-Network/tiger/tree/main/packages/fukugen-daihon)
  (Blam Network; **no licence**): the restored mission scripts the sidecar runs, read from a
  local checkout through `D1_SCRIPTS_DIR` under the owner's exception in
  `ps4/design/licensing.md` (rule 2). Never copied, quoted or redistributed here; the repository
  holds only activity names and counts derived from running them.

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
