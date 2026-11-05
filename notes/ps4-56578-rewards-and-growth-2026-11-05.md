# PS4 56578: rewards and character growth services (5 November 2026)

Stream M of the full-game operation. This adds the server's reward and growth services to
d1-responder:
- loot, engrams and Light;
- item instances and talent grids;
- the Grimoire, Record Books and quests;
- currencies.

They are offline services, built and unit-tested only. Nothing here was run against the game.
Every rule they apply is in the table below, with its source and status. **No reward behaviour
here has been seen in a native run.**

The code is `ps4/d1-server/crates/d1-responder/src/rewards/`. It is off unless `D1_REWARDS=1`.

## What other streams call

| Caller | Function | What it does |
|---|---|---|
| J (strikes, raids, Crucible), A2 (script bridge) | `rewards::grant_activity_rewards(&mut store, character, activity_hash, &ActivityOutcome)` | One player's end-of-activity rewards. Call it once per fireteam member at `activity_complete` (incident 965), with the manifest hash the client launched. It returns a `RewardGrant`: currencies granted and lost, new item instances, progressions, item XP, the weekly bonus left, and the rule ids applied. |
| I (inventory, Cryptarch) | `rewards::engrams::decrypt(&mut store, character, engram_instance, &mut rng)` | Decrypt one engram the character holds. |
| I | `rewards::engrams::on_pickup`, `rewards::currencies::{grant, spend, dismantle_yield}` | The engram pickup rule, wallets and their caps, and dismantle yields. |
| I, C | `rewards::talent_grid::GridState::{activate, add_xp, available}`, `share_xp` | Node activation and item/subclass XP. |
| any | `rewards::grimoire::{unlock_card, add_stat, score}`, `rewards::records::{advance_objective, book_progress, claim_book_rewards, advance_quest}` | The Grimoire, Record Books and quests. |
| responder | `rewards::item_instance::write_element(template, &instance, &dyn ClientIndex)` | Writes an instance into a `0x80801732` element. |

`ActivityOutcome` has four fields:
- `completed`;
- `community_page`: the cryptarch-guide page for the activity, which selects the community Mark limits;
- `roll_engrams`: the placeholder engram drops;
- `seed`: else `D1_REWARDS_SEED`, else the time.

Every roll goes through a seeded SplitMix64, and the grant records the seed, so a reviewer can
replay a grant exactly.

### State

The services read and change state through `rewards::store::RewardStore`. `MemoryStore` is the
in-memory version, for tests. `rewards::player_store_adapter::Adapter` runs the services on stream
C's persistent store (`player_store::PlayerStore`, #307). It routes these through C's store:
- XP: `add_xp`, through C's curves;
- currencies: `currency` and `adjust_currency`, by 56578 content index;
- the Grimoire score: `add_grimoire_card`;
- Light: `set_light`;
- instance ids: `mint_item_soid`.

It keeps what C's record has no field for yet in its own overlay:
- item instances with their grids;
- counters;
- objectives;
- record-book claims.

Saving that overlay into C's record (its `attributes` maps) is the next step.

### Tables

`tools/rewards_tables/gen_rewards_tables.py` reads the local manifest and writes the compact
tables under `rewards/tables/`. Each table's header names its input and the input's SHA-256. The
tables keep only hashes, tiers, buckets, thresholds and amounts, with no names or text. To
regenerate them:

```sh
python3 tools/rewards_tables/gen_rewards_tables.py --manifest <dim-d1-manifest> --cryptarch <cryptarch-guide>/public/data
```

| Table | From | Rows |
|---|---|---|
| `items.tsv` | Items in the gear, engram, material and currency buckets: tier, bucket, class, grid, stack, primary-stat range, armour-stat ranges | 6,376 |
| `progressions.tsv` | All progressions' step costs | 252 |
| `talent_grids.tsv` | The grids those items use, with each node's level, flags, prerequisites and exclusions | 1,167 |
| `activities.tsv` | Every activity's level and reward list | 653 |
| `reward_display.tsv`, `weekly_bonus.tsv` | Reward display items and the stackables they stand for; the `(Weekly Bonus Remaining: N)` markers | 7, 5 |
| `record_books.tsv`, `records.tsv`, `objectives.tsv`, `quests.tsv` | Books, records, objectives, quest steps and their questline order | 5, 168, 1,388, 642 |
| `grimoire.tsv` | Cards, points and stat ranks (Bungie's Grimoire tables, from cryptarch-guide's copy; DIM's archive has none) | 834 |
| `engram_pools.tsv` | COMMUNITY STAND-IN: slot engram contents, and the Silver Dust engrams' contents | 38 |
| `mark_limits.tsv` | COMMUNITY STAND-IN: per-activity Legendary Mark grants and limits | 7 |

## Every rule

Status:
- **verified**: from Bungie's data (the manifest), Bungie's patch notes or the client;
- **community**: from community collation;
- **placeholder**: not public; an isolated, labelled stand-in that needs evidence.

The placeholders' values are all in `rewards/placeholders.rs`. A test checks this table against
`rewards::RULES`.

| Rule | What it says | Source | Status |
|---|---|---|---|
| `light.cap` | Light cap 400 from 2.4.1 | Patch notes 2.4.1; Year 3 Legendary primary max 400 in the manifest | verified |
| `light.cap.2_4_0` | 385 in 2.4.0 | Patch notes 2.4.0 | verified |
| `light.item_bounds` | An instance's Attack/Defense is within its item's primary-stat min and max | Manifest | verified |
| `light.decode_cap.rare` | Rare engrams up to 365 | Patch notes 2.4.1 | verified |
| `light.decode_cap.legendary` | Legendary engrams up to 390 | cryptarch-guide (final state); 2.4.1 said 385 | community |
| `light.decode_cap.exotic` | Exotic engrams up to 400 | Patch notes 2.4.1 | verified |
| `light.at_or_above` | Rare and Legendary drops try to land at or above the player's Light | Patch notes 2.4.0 | verified |
| `light.slow_zone` | In the last 15 below a cap, higher only half the time | 2.4.0 said this for 350–365; extending it to every cap needs evidence | placeholder |
| `light.spread` | A drop lands 0–4 above the basis | Needs evidence | placeholder |
| `light.common_roll` | Common and Uncommon drops are uniform between floor and cap | Needs evidence | placeholder |
| `light.basis` | The basis is the highest Light the character could equip (one Exotic weapon, one Exotic armour piece) | Rise of Iron player reports | community |
| `light.weights` | Character Light: weapons 6, armour 5, Ghost/Artifact/class item 4 of 50, floored | DIM's D1 code; needs checking against the client's `0x265EF0` | community |
| `light.infusion` | The target rises to the fuel's Light, capped by its own maximum | Rise of Iron, community | community |
| `light.infusion_cost` | No cost | Needs evidence | placeholder |
| `engram.bucket` | An engram decrypts into its own bucket | Manifest (engrams carry their slot's bucket) | verified |
| `engram.class` | Armour is for the decrypting character's class | | community |
| `engram.min_tier` | Never below the engram's tier | Patch notes 2.0.0 (no materials from Legendary engrams), community | community |
| `engram.upgrade` | One tier higher: 0‰ | Needs evidence | placeholder |
| `engram.pool` | Slot pools from the engram pages | cryptarch-guide `rewardBoxData` | community |
| `engram.pool.fallback` | Otherwise, every matching item that reaches the cap | Needs evidence | placeholder |
| `engram.uncommon_convert` | At level 40, Uncommon engrams become weapon/armour materials on pickup | Patch notes 2.5.0 | verified |
| `engram.uncommon_convert.amount` | One material each | Needs evidence | placeholder |
| `loot.activity_rewards` | Completion grants the activity's own reward list | Manifest `DestinyActivityDefinition.rewards` | verified |
| `loot.display_map` | A display item stands for the same-named stackable | Manifest names | verified |
| `loot.weekly_bonus` | The weekly-bonus marker limits the list to N a week, per account | Manifest marker; account scope from cryptarch-guide | community |
| `loot.mark_limits` | Per-activity Mark grants and daily/weekly limits | cryptarch-guide `activityPageData` | community |
| `loot.drop_chance` | End-of-activity engrams: Rare 400‰, Legendary 100‰, Exotic 10‰ | Needs evidence | placeholder |
| `loot.per_player` | Each fireteam member rolls separately | D1's personal loot; `loot-and-rewards.md` design | community |
| `currency.caps` | Wallet caps = `maxStackSize`: Glimmer 25,000, Legendary Marks 200, Silver Dust 250, planetary materials 200 | Manifest | verified |
| `currency.overflow` | Amounts over a cap are lost | | community |
| `currency.marks_weekly_cap` | 100 Legendary Marks a week from activities; dismantling isn't counted | The Year 2 figure; needs evidence for 2.6 | placeholder |
| `currency.scope.glimmer` | Glimmer is account-wide | Client bucket table (slot 308, scope 1) | verified |
| `currency.scope` | Marks and Silver Dust are account-wide; materials are per character | | community |
| `currency.dismantle.silver` | Silver Dust engram items give 1 Silver Dust, Icy armour 5, Dusty Iron armour 1 | Patch notes 2.6.0, 2.4.1.1 (item sets from cryptarch-guide) | verified |
| `currency.dismantle.marks` | Year 2/3 Legendaries give Marks: 3 for a weapon, 2 for armour | The source is patch notes 2.0.0; the amounts need evidence | placeholder |
| `currency.dismantle.other` | Rare and lower give nothing | Needs evidence | placeholder |
| `grid.level` | Item XP runs through the progression's step costs (the last repeating) | Manifest; client `0x284630` (01.29 reading) | verified |
| `grid.node_level` | A node needs its `gridLevel` | Manifest | verified |
| `grid.prerequisites` | Prerequisites first; exclusive nodes exclude | Manifest fields | community |
| `grid.auto_unlock` | `autoUnlocks` nodes activate when available | Manifest field | community |
| `grid.random_roll` | Random nodes pick a step uniformly | Needs evidence | placeholder |
| `grid.node_cost` | Nodes cost nothing | Needs evidence (the material sets aren't in DIM's manifest) | placeholder |
| `grid.xp_share` | Each equipped grid item gains the full XP | Community says they all gain XP; the split needs evidence | placeholder |
| `item.element_fields` | `0x80801732`: SOID `+0`, level `+0x18`, content index `+0x24`, XP `+0x38`, progression `+0x44`, grid `+0x58`, stats `+0x98` | Client ([items-and-gear §2](../ps4/design/items-and-gear.md)) | verified |
| `item.node_state` | Node states `+0x5A` and step selection `+0x7E` | Not decoded, not written | placeholder |
| `item.stat_roll` | Armour stats are uniform within the manifest range | The range is from the manifest; the distribution needs evidence | placeholder |
| `item.level_for_light` | The element level for an Attack/Defense | Needs the client's curves (tag `0x80A5E244`) | placeholder |
| `grimoire.score` | Unlocked cards' points plus stat ranks reached; 5,610 in all | Manifest (card totals agree with the cardBriefs) | verified |
| `grimoire.triggers` | What unlocks a card | Server-side; callers unlock explicitly | placeholder |
| `records.complete` | A record completes when every objective reaches its value | Manifest | verified |
| `records.book_points` | 1,000 book progression per record | Needs evidence | placeholder |
| `records.rewards` | Rank rewards from the manifest, each claimed once | Manifest | verified |
| `quests.advance` | A step completes with its objectives; the next in the questline follows | Manifest (questline order) | community |

Count:
- 19 verified;
- 15 community;
- 19 placeholder.

## Evidence needed, by rule

- **Drop chances, the Light spread, decrypt upgrades, dismantle amounts and the Marks weekly
  cap:** counts from recordings or live captures. The private Xbox 360 captures
  ([live captures](d1-live-captures-2026-11-05.md)) are Year 1, so they settle none of the 2.6 rules.
- **Character Light weighting:** read the client's own sum (`0x265EF0`) and the select card's
  stored Light against a known loadout.
- **Item elements:**
  - the level curve that gives an Attack/Defense (`d1-oracle character derived-stat` already runs it);
  - the node-state bytes: serve an instance with one node toggled and read it back;
  - the order of a definition's stat list.
- **The Grimoire, Record Books and quest objectives on the wire:** no 56578 structure for them is
  decoded. Look in the account object and the profile, from a capture of an account that has
  them.
- **Decrypt requests:** which investment message the Cryptarch sends in 56578 (901 or 402) is not
  seen yet.
- **Stream C's `activity-rewards.tsv` and this module both give XP:** C's per completion (by
  56578 activity index), this module's from the manifest's "Experience" line. An activity listed
  in both must take XP from only one of them. The adapter's `xp_from_manifest` switch decides,
  and is off by default, leaving XP to C.

## Tests

`cargo test -p d1-responder rewards` covers:
- table parsing;
- Light roll bounds for every tier, source cap and basis;
- talent-grid unlock order and XP over 400 grids;
- the Grimoire total matching the manifest (5,610);
- Glimmer and Marks caps, including the weekly cap and its reset;
- the decrypt tier rules over every Rare, Legendary and Exotic slot engram for all three classes;
- book ranks and rewards, and quest advance;
- the element writer's offsets;
- seeded, per-player drops;
- the adapter onto stream C's store.
