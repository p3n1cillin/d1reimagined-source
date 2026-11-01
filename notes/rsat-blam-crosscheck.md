# Our RSAT codec against Blam Network's (1 November 2026)

[Blam Network's tiger](https://github.com/Blam-Network/tiger) includes `packages/rsat`, an
independent TypeScript codec for RSAT, the bit-packed format Destiny's services speak (MIT). It
holds no game schemas. [`tools/rsat_crosscheck`](../tools/rsat_crosscheck/README.md) took 23 of our
`d1-bap` encodings, decoded each with Blam schemas written to mirror ours, re-encoded it with Blam,
and compared the bytes.

**Result:** 26 of 28 checks match byte for byte. The two differences are on Blam's side, so neither
points to a fault in our encoding.

## What matched

- **Bit order:** MSB-first and zero-padded to a byte, the same as our `pack_msb_first`.
- **Character:** race, gender and class are Blam `i8` with its default bias `0x80`. Our `+0x80` is
  the standard signed bias.
- **Roster:** with 0, 1 and 3 characters, as `array(u64, {lengthBits: 4})`.
- **Account:**
  - plain;
  - `serial` 7;
  - no profile;
  - fields 6 and 20 as nested blobs.
- **Profile:** plain, and with its destination (field 2, sub-field 10 a `u32`).
- **Small bodies:**
  - the 503 SOID body;
  - the 501 create-character response;
  - `RESULT_OK_BODY` (a 4-bit leaf with bias 1, plus an `i32` with bias `0x80000000`).
- **206:** `fetch_family_response_body` is Blam's `encodeServerMessage` minus its leading message id
  (`u16 id | args | 1 bit | u16 len | bytes`), with and without a span.
- **QueueZ:** the family-3 roster and character, the family-4 baseline and the delta ping all have
  the layout of Blam's `encodeQueuezFamily`. Blam also notes that an object's checksum is not always
  its TypeDef hash, which matches our instance checksums.
- **Unlock flags:** the array at bit 141 of the 205 body decodes as
  `array(nested{optional(i16), optional(i32{size:2, bias:1})}, {lengthBits:6})`. Our `^0x8000` on
  the index is the standard `i16` bias.

## The two differences

1. Blam's `u32` decodes values of 2³¹ and above as negative numbers (`(wire - bias) | 0`); Old
   Russia's hash `2468353467` comes back as `-1826613829`. Encoding is unaffected.
2. Blam's `encodeQueuezFamily` defaults to `version = 1`, while our baselines send 0. Code that
   mixes the two must pass the version explicitly, because our deltas expect the accepted version
   plus 1.

## What it changed here

- **The 206 span length is 16 bits.** Given a longer span, both codecs wrapped the length silently.
  `fetch_family_response_body` now refuses a span over 65,535 bytes, with tests for the longest
  span that fits and for one that doesn't.
- **Blam's default bias for narrow signed fields is `1 << (size - 1)`.** D1's narrow leaves (the
  4-bit result and family fields, the 2-bit flag value) use bias 1. Blam's own README states
  `bias: 1` for the result header, so the bias must be given explicitly wherever these are modelled.

## Names from Blam's dictionary (build 41375)

`packages/dictionary/destiny1/v41375_14_07_03_1501_live/network-ids.csv` names every message id of
the 2014 live build with its request and response schema tags. They agree with ours wherever both
name the same id:
- 206 (`80801774`/`80801869`);
- 503 `login_account`;
- 501, 205 `fetch_universe`, 901 and 903;
- the wire types 10–303;
- the activity host's ids 0–42.

Ids we don't use yet, which are candidates for the open vendor and inventory work:

| Id | Name |
| --- | --- |
| 104 | `director_enter_orbit` |
| 207 | `fetch_account_family_versions` |
| 402–405 | inventory (403 `inventory_equip_item`) |
| 801–803 | talent grid |
| 902 | `vendor_buyback_item` |
| 1201–1227 | `cheat_*` (1202 `cheat_character_add_item`, 1207 `cheat_character_set_unlock_flag`) |
| 1301–1304 | activity host |
| 1401 | `update_offers` |

Wire ids 14/15 (Unsubscribe) and 250/251 (Echo) are also listed.

QueueZ families in Blam's `schemas.csv`:

| Family | Blam's name | Our use |
| --- | --- | --- |
| 0 | peer | peer contact and peer view |
| 2 | roster | contact and card |
| 3 | select-character | character card |
| 4 | self | account, profile and inventory items |

One naming difference to keep in mind: Blam calls `0x80801743` "SelfInventory", where we use it as
56578's profile wrapper around `0x80801714`. Our profile also carries the 340 item slots, and our
tag comes from the eboot's own descriptors, so the names may simply describe the same object.
