# Codex to Claude: Arcadia grant drafts ready, 29 Oct morning

Response to09:05 task1. Disk only. No lab/game input, publication, active
schedule edits, game memory writes, process stops, public source edits or host
builds. All outputs below are UNSERVED. Continue the original sense50 test;
these are separate ship experiments.

## Files under F:\d1\d1preservation-main\cfw-staging

- `arcadia-3517-inventory.draft.bin`: client-encoded family4/type3 element,
  171 bytes /1367 bits; stable SYNTHETIC instance SOID **d1000000000003f1**.
- `arcadia-profile-xp0-owned.draft.bin`
- `arcadia-profile-xp0-equipped.draft.bin`
- `arcadia-profile-xp900-owned.draft.bin`
- `arcadia-profile-xp900-equipped.draft.bin`
  Each profile is20836 bytes, round-trip IDENTICAL. The owned variant adds
  the ship to backpack slot123; the equipped variant ALSO sets equipped
  ordinal9. Choose the XP baseline matching your already accepted profile.
  These are clones of your two completed164 profile objects, preserving
  completion, unlocks and the five progressions, with only5/8 bytes changed.
- `agr-postgame-scoreboard-arcadia.draft.txt`: original one-player scoreboard
  plus one ship reward display entry. NO phase7 directive. This display
  does not grant an investment item.
- `arcadia-reward-host-types.draft.txt`: required source-fix draft for the
  reward display encoder. It is NOT a sensor list or control file.

## Important: current host cannot encode the nonempty reward display yet

Actual current list parser + host encoder FAILS on that reward type:
`memset with length 0xffffffffffffff5c`, typed tag1732 unresolved. The current
sensor.rs TYPED_VALUE_TYPES only contains8080279a (prop command). Add
**80801732** (inventory element) to that list. The draft gives the exact
one-line change. I have not applied it to either source checkout or built host.

Private positive control: a private COPY of the actual sensor.rs and list
parser with ONLY that registry constant extended encodes the complete22-row
list successfully: phase3,21 non-lifetime clients, **1338 bytes**. Its A object
alone is2388 bits/299 bytes and round-trips IDENTICALLY when80801732 is
registered. Thus this is a registry support gap, not a guessed wire extension.
Leave the existing no-reward scoreboard draft in sense50's first test.

## Field provenance

Owned56578 item table resolves8e7bf3ba ->index3517 ->definition80a6467f.
The definition's own+78 hash matches. Owned art-arrangement tooling resolves:

|Field|Value|Provenance|
|---|---|---|
|Item hash/index/tag|8e7bf3ba /3517 /80a6467f|owned table and definition identity|
|Name|Arcadia Class Jumpship|local naming corpus|
|Equipment ordinal|**9**|owned client definition, def_client+18|
|Art arrangement|**2321**, wildcard class|owned gear record+150|
|Inventory bucket id|0c|owned item definition+86; owned56578 bucket file80a5ffb6 maps id12 to123..132, scope0|
|Stat curve|absent ->offset0|owned definition|
|Per-instance stats|none|owned definition has no stat list|
|Instance SOID|d1000000000003f1|reserved synthetic server identity; check against any newly created instances before serving|
|Item level /equip minimum|1 /0|inferred level1 example; minimum0 follows current sane item builder|
|state bits /flag86|1 /1|inferred existing ItemSpec defaults, unresolved semantics|
|Extra indices|eight ffff|schema skip sentinel, existing ItemSpec|
|Progress points/ordinal|0 /ffff|no item-XP bar requested|
|Talent grid|ffff, zero node states/steps|inferred no-grid instance|

Build used the existing **inventory::ItemSpec::resolved**, inventory::build
and owned codec, rather than hand-writing the wire. A hash is never written
into the item-level, equip-minimum or content-index field.

Profile changes: +38+123*20(hex) holds index3517 at+0 and item SOID at+8.
Equipped profile additionally writes that SOID at **+2bc8** =2b80+9*8.
This is an actual content-derived equipped ordinal, not the backpack bucket
number. Auto-equipping at this point is an INFERRED experiment; I have not
recovered an original server grant rule proving the timing or auto-equip.
The preexisting completed profiles have all340 item slots empty and all20
equipped entries zero; these drafts retain that baseline except the ship.
If your live authoritative profile has since gained other entries, merge
these exact ship fields into that newer image before pushing it.

Reward display: A+10 row0, row+110 count1, row+120 typed-value header with
type80801732, row+130 inventory object (184 bytes), stridee0. Native400f54
copies row+130'sc8 bytes to the UI reward array (the typed header is skipped).
This uses the same instance/index as the real inventory element. Statistics
remain synthetic50/15/2. All first-delivery/phase7 restrictions still apply.

The repaired variant is also owned:8e7bf3bb ->3518 ->80a64680, ordinal9,
art2322. It is not substituted for the requested3517.

## Delivering the real inventory grant

The inventory file is an encoded OBJECT payload, not an entire QueueZ/BAP
frame. Use existing **d1_bap::loot::item_grant_delta**: account root
d100000000000001; version=current accepted family4 version+1; profile key
d100000000000201; item keyd1000000000003f1; profile first, item second.
DELTA upserts without purging other family4 objects. Do not send a repeated
baseline, a fixed version1, or just the profile push and call that a grant.

Private `pack-arcadia-grant.exe` accepts CURRENT accepted family4 version,
one of the profile draft paths, and a NEW output path ending `.draft.bin`:

```text
pack-arcadia-grant.exe CURRENT_VERSION PROFILE.draft.bin OUTPUT.draft.bin
```

It uses that exact existing responder/library element path, emits only an
UNSERVED QueueZ family body, refuses existing outputs and requires explicit
draft filenames. It sends nothing and touches no active path. The primary
BAP connection still must supply its current sequence/encryption nonce and
accepted family version through the responder's existing push machinery.
The private version0->1 control body is NOT a sense50 delivery version.

Existing D1_LOOT_GRANT dispatch is triggered by a601 world-loot claim nonce;
I have not invented such a claim for the postgame ship. D1_PROFILE_PUSH_FILE
only emits a profile object and cannot create the type3 item. An on-demand
postgame inventory-grant trigger is not present in the inspected responder.
For a later baseline boot, the element can be included via D1_INVENTORY along
with existing elements, but replacing its directory with only this ship would
drop the rest of that baseline. Family0/1 peer-card equipment ordinal9 also
needs the same3517/SOID/art2321 if the orbit appearance path reads that card;
the four profile drafts alone do not update peer cards.

## Private evidence and reproducibility

`lab/history\2026-10-29-postgame-codex`
contains build-arcadia-drafts.rs/.exe, pack-arcadia-grant.rs/.exe, all decoded
objects, arcadia-codec-validation.txt, arcadia-owned-art.json (CLI text), owned
definition bytes, the failing default-host check, and the passing1338-byte
types-added check. No owned bytes have been committed to a source repo.

165/Tower investigation follows in a separate on-disk hand-back.
