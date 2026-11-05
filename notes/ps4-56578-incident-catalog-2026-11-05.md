# PS4 56578: the complete client incident catalogue (5 November 2026)

The owned 56578 content declares **3,902 client incidents**, indexed 0–3901.
[incidents-56578.tsv](../ps4/fixtures/56578/incidents-56578.tsv) records every index,
name hash, known name, payload selector and four auxiliary u16 values. **213 names**
have verified FNV-1 hashes; the other 3,689 names are blank. An unknown name does
not prevent lookup or translation by hash.

This is an **offline content export**. No lab, emulator, native process or eboot
decoder ran. The table supplies identities for the activity host and mission-script
bridge; it does not establish incident-body layouts, delivery filters, acknowledgements,
or permission to grant rewards. No host or bridge behaviour changes in this work.

## Source and layout

[d1_incidents.py](../tools/ps4_setup/d1_incidents.py) reads packages through the existing
MIT gibbon/poodle packages in [Blam Network's tiger](https://github.com/Blam-Network/tiger).
The local checkout used here is commit `82c57cb482767e0ba07030e988b7bfaa6b1a4bd9`.
Its `v56578_17_04_12_1251_evil` module selects the package reader. The tool finds the
named `client_startup_globals` root, then requires exactly one referenced entry of
class `0x80800659`. It does not select the first registry in the estate: there are two.

For the supplied owned content, root `0x80b1a000` references client registry
`0x81656598` at root +0x3c. These are observations and regression targets, not exporter
constants. The root-reference scan discovers them again on each export. The
56578 record layout was checked directly in that registry, rather than applying
01.24 or 01.29 executable offsets:

| Location | 56578 layout |
|---|---|
| Registry +0 | u64 declared tag length |
| Registry +0x20 | Array descriptor: u64 count, signed i64 field-relative link at +0x28 |
| Array header | Marker `0x80800184` immediately before it; repeated u64 count and u32 element class `0x808003d3`; records at header +0x10 |
| Record stride | 32 bytes |
| Record +0 | u32 incident name hash |
| Record +8 | Signed i64 definition link, relative to that pointer field |
| Record +0x10 | Four u16 auxiliary fields, kept in order |
| Record +0x18 | u8 payload selector |

All of these fields are little-endian. The exporter extends the existing
[catalogue parser](../scripts/d1_incident_catalog.py) and
[array reader](../scripts/d1_pkg.py); it does not add another package decoder.
It checks declared size, array marker/count/class, bounds, definition links,
contiguous indices and duplicate hashes before producing the fact tables.
Definition bytes and offsets are excluded from the committed TSVs.

Names come from locally reading tiger's `packages/dictionary/destiny1` CSVs and
`packages/bullshcript/INCIDENTS.md`. Only candidates whose computed FNV-1 equals a
catalogued hash survive. Conflicting candidate names for a hash fail explicitly.
The dictionary has no licence: none of its files, source/confidence columns or
descriptive text is copied. The committed names are derived hash → name facts.
Hash agreement is a name-identification check, not proof of a name's gameplay
semantics or absence of every possible 32-bit collision.

## 51539 / 01.24 to 56578

[incidents-51539-to-56578.tsv](../ps4/fixtures/56578/incidents-51539-to-56578.tsv)
joins every record of the retained
[01.24 report](../lab/history/ps3-era/research/JOIN218-incident-catalog-report.json)
to 56578 by equal name hash. The 51539 labels use the supplied premise that its
Xbox 360 incident IDs match 01.24's; this work does not independently extract a
51539 executable or catalogue. The existing live-capture anchors below agree.

**3,082 / 3,083 source records map (99.97%)** to distinct target entries.
The sole missing source entry is index **2114**, hash **`0xae820ae1`**; its target
index is blank. There are **820 target hashes absent from the legacy table**.
No index offset, interpolation, name guess or selector match is used to fill gaps.
Matched entries retain hash order, but index differences vary; translate by the
hash join.

| Capture check | 51539 / 01.24 | 56578 | Name hash | Verified name |
|---|---:|---:|---|---|
| Cinematic stopped | 734 | 891 | `0x3c03f059` | `cinematic_finished` |
| Cinematic started | 2120 | 2681 | `0xaeca3822` | `cinematic_started` |
| Player crossed a trigger | 2665 | 3403 | `0xdd6d986f` | `player_trigger` |

These are the checks from [What decodes](d1-live-captures-2026-11-05.md#what-decodes).
The independently supplied `activity_complete = 965` target is also confirmed:
legacy **789** has the same hash `0x42660690`.

The five host incidents listed in the [live captures](d1-live-captures-2026-11-05.md)
are now identified:

| 51539 host ID | 56578 ID | Name hash | Verified name |
|---:|---:|---|---|
| 2426 | **3092** | `0xc9102241` | `activity_join` |
| 2938 | **3732** | `0xf41ef26b` | `insertion_point` |
| 808 | **990** | `0x43ce9f66` | `tutorial_weapon` |
| 1170 | **1440** | `0x62a2eea4` | `tutorial_character_level` |
| 789 | **965** | `0x42660690` | `activity_complete` |

A multi-type incident chain needs each of its indices translated separately.
The name `insertion_point` identifies the incident accompanying the observed
23/24 exchanges; it does not, by itself, prove that every occurrence means an
authority handover. Payload selector and auxiliary-field meanings remain open.

## Reproduce and test

Build tiger locally (`npm ci` and `npm run build` in that checkout). Set
`D1_TIGER` to it, `D1_PACKAGES` to the owned 56578 packages directory, and put Node
and Python on PATH. The exporter reads raw tags into memory through subprocess
pipes; it writes only the two text fact tables:

```powershell
python tools/ps4_setup/d1_incidents.py --packages "$env:D1_PACKAGES"
```

This reuses the committed catalogue's hash-checked name facts. To rederive the
names from the optional local sources, add:

```powershell
python tools/ps4_setup/d1_incidents.py --packages "$env:D1_PACKAGES" `
  --dictionary "$env:D1_TIGER/packages/dictionary/destiny1" `
  --incident-notes "$env:D1_TIGER/packages/bullshcript/INCIDENTS.md"
```

`--names` selects a different TSV of name facts; a missing file starts with no
names. `--out` and `--mapping-out` select alternative text outputs. `aux` is a
comma-separated sequence of four decimal u16 values; hashes use `0x` plus eight
hexadecimal digits. A blank mapping target means absent, never index zero.

Validation performed:

```powershell
python -m unittest discover -s scripts -p test_d1_incident_catalog.py -v
python -m unittest discover -s scripts -p test_d1_pkg.py -v
python -m unittest discover -s tests -p test_incidents_56578.py -v
python .github/scripts/check_pr.py --title "ps4: catalogue 56578 incident identities and legacy mappings"
```

The parser's **8 tests** and shared package reader's **17 tests** pass. The new
table suite has **6 tests**, including a full owned-content export compared with
both committed tables. That private-input test runs when `D1_REPO` names the
owned repository containing `dumps/later-56578-CUSA00219/CUSA00219/packages`;
`D1_PACKAGES` can override that directory. Set `D1_TIGER` and, if Node is not on
PATH, `D1_NODE`. With inputs present, missing prerequisites or any full-table
disagreement fail. Without the inputs, it explicitly skips; the other five tests
still check all identities, names, mapping rows and capture anchors offline.
