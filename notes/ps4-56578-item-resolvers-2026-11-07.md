# Granted-item grid indices for build 56578

`ExportedIndex` now resolves the item element's `+0x58` from
[`talent-grid-index.tsv`](../ps4/fixtures/56578/items/talent-grid-index.tsv).
The generator is `d1-oracle content-tables --out <fixture-directory>`, using the existing package,
Oodle and self-relative list readers. It requires the pinned owned 56578 executable. The grid
table is tag `0x80A5E227`, class `0x808018C2`: 1,405 rows of class `0x808017CB`, 24 bytes each.
The index is the row position; the hash is the row's first u32. Duplicate hashes, truncated rows
and ordinals reaching the absent sentinel are rejected. The export records the source tag and
SHA-256; no definition payload or binary fixture is added.

The generated table pins Khvostov's grid hash `0xFD2AFFAD` to ordinal **279**, and
Preacher Mk. 20's grid hash `1363610267` to ordinal **286**. Both the lookup regression and
the item writer regression check these content-derived targets; runtime values come from the TSV.

## Template comparison

The existing served
[`khvostov-3598.bin`](../ps4/fixtures/56578/characters/grants/khvostov-3598.bin)
decodes with the 56578 oracle to 184 bytes (1,367 wire bits): content index 3598, level 40,
progression ordinal `0xFFFF`, talent grid `0xFFFF`. Its grid differs from
the content table's Khvostov ordinal 279. The private-input regression decodes this fixture with
`d1-rsat`, checks that discrepancy, and verifies that `write_element` replaces the sentinel with
the exported ordinal. Tests without `D1_EBOOT` or `D1_REPO` print a skip reason; with inputs,
missing fixtures or decode failures fail the test.

The lab handoff reports that the served Khvostov renders with this same absent-grid sentinel.
That report weakens the grid explanation for F55; no native reproduction was performed here.
The export supplies content-derived ordinals for faithful item construction, without establishing
that the sentinel causes the black frame or that substituting an ordinal resolves it.

## Still open

- **Level at `+0x18`:** `level_for_primary` still returns `None`. The
  [global curves](../ps4/fixtures/56578/items/stat-curves.json) describe the forward base-stat
  mapping, not a one-to-one per-item inverse. Definition offsets, explicit base rows, progression
  rank and talent contributions are additional inputs; integer display conversion also loses
  information. The existing [curve investigation](ps4-56578-rewards-and-growth-2026-11-05.md)
  explains these gaps. Assigning level 1 solely because a manifest says Attack 5 would guess.
- **Progression at `+0x44`:** the existing resolver answers hashes in the committed progression
  table, but both starter weapons' manifest grid rows name progression hash `0`, absent from that
  table. Their grants still report this field as unresolved and retain the template value.
  This does not establish that the owned item needs no progression or that an arbitrary shared
  progression should be substituted.
- **Node states:** `+0x5A` and `+0x7E` retain the template's bytes. Grid lookup does not establish
  the correct per-instance node-state encoding.

All results here are offline content exports or codec/unit checks. No native run was performed,
and this change does not establish that F55's black frame on equip is fixed.
