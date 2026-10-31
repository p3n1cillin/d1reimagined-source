# Findings from other 01.29 projects, and what they mean for 56578 (31 October 2026)

Two outside sources, read as references only (no code copied):

- **Documentation from a later version of a D1 server project** (the layout of aknumbers' Destiny 1
  Preservation Project, with Go and Rust services). The maintainer shared it on 31 October. It is
  pinned to PS4 01.29. Its topics: BAP, services, QueueZ, replication, the native RSAT codec, and RSAT
  serving coverage.
- **[lemonadeafk/d1-shadps4-windows](https://github.com/lemonadeafk/d1-shadps4-windows)**: Windows
  scripts and patches running 01.29 on shadPS4 against a patched d1preservation server, with a long
  findings log (`NIGHT-NOTES.md`). The repository states no licence, so it is a reference for ideas
  only.

Addresses below are 01.29's unless marked; 56578 addresses differ. Structure offsets have so far
matched 56578 where checked.

## The missing head: the wear-helmet flag

The character customization block `0x8080181A` (36 bytes, 12 fields) sits at `+0x0C` in the
character card, the profile and the peer view. Its last field, `+0x20` within the block (absolute
`+0x2C`), is the wear-helmet flag: 2 bits, biased by one. lemonadeafk found that with the flag at 0
the helmet is not drawn. With the face fields all zero as well, nothing is drawn where the head
should be. Setting the flag to 1 in all three objects made the helmet render.

On 56578 the block is at `+0x0C` too (`d1-oracle character layout`; `d1_bap::CustomizationBlock`).
Every fixture in `ps4/fixtures/56578` has the flag at 0. This fits issue #128: the Moon (combat)
draws the helmet anyway, the Tower does not. A test pushes the completed profile with the flag set,
in the Tower.

## No sprint, grenade or melee: the ability rows

lemonadeafk's Titans and Warlocks had no sprint, no grenade and the wrong super. The character
carried the Bladedancer's ability rows, or none at all. The rows come from the subclass's talent
grid (grid table tag `0x80A5E227`; grids 130–135 and 888–890). With rows derived per subclass,
sprint ran at 6.6 units/s against 4.1 walking, and the grenade, melee and super appeared. The rows
they used:

| Subclass | Rows |
|---|---|
| Gunslinger | 33, 5, 8, 3, 13 |
| Nightstalker | 31, 2, 8, 3, 13 |
| Striker | 29, 4, 7, 9, 13 |
| Defender | 34, 15, 7, 9, 13 |
| Sunbreaker | 27, 14, 7, 9, 13 |
| Sunsinger | 18, 11, 16, 6, 13 |
| Voidwalker | 22, 10, 16, 6, 13 |
| Stormcaller | 23, 0, 16, 6, 13 |

Their character card layout: an ability block at `+0x10` of the appearance data (5 × {u8 row,
16 × u32 option hash}), and 32 × u16 perk indices at `+0x768`; zeros mean no sprint or grenade. A
contributor reports no sprint and no melee on a pad on our build, which fits.

## The idle kick

The idle teardown at `0x792210` fires after 5, 15 or 60 minutes without stick input, depending on
the activity. A two-byte patch at `0x792302` (a `ja` replaced by two NOPs) stops it. 56578 needs its
own address. The project's rule stays: every automated loop moves a stick.

## Tower permissions and the Director

Their 01.29 Tower requirement sets are {NOT 3389, NOT 308} and {419 OR 440, 2667 OR 440}. Ours, on
56578, are {NOT 308, NOT 3391, 419 OR 440, 2668 OR 440}
(`notes/ps4-56578-tower-social-vendors-2026-10-30.md`). The flag numbers moved by one or two between
builds; the rule is the same.

Their investment clock is family-5 object `0x80801941` field [1], a u64 in Unix seconds. Served with
the current time, the Director works without memory writes.

## QueueZ lifecycle

- Registration mode 2 sets state 2. State 2 accepts only a baseline and then enters state 3.
- State 3 accepts a baseline (which purges the family to its keep-list) or a delta whose version is
  exactly the stored version plus one.
- An out-of-order delta is refused, but its version is still recorded.
- A zero-object delta is not silence: it advances the lifecycle.
- BAP 124 (subscription lost, a 14-byte body) zeroes the family's objects and puts it in state 4.
  Reason 204 makes that permanent. A server should not send 124; a fresh baseline does the job.

The family table in that documentation (01.29) agrees with what we serve:

| Family/type | Object |
|---|---|
| 0/1 | Peer view `0x808017FD` |
| 3/1 | Character card `0x80801954` |
| 4/0 | Account `0x80801803` |
| 4/1 | Profile `0x80801743` |
| 4/3 | Inventory item `0x80801732` (666 slots) |
| 4/4 | Unlock object `0x8080102E` |
| 5/5 | Shared investment state `0x80801941` |
| 7 | Account-to-party lookup |

## Vendors

That project serves 901 (buy), 903 (sell) and 905 (refund) from the content's sale tables, building
the item from the owned item table and taking the price from the client's own currency stacks, as
our responder does for 901 (#122). Selling and refunds are the next step here. The requests'
contents are still to be read on 56578 (`notes/ps4-56578-vendor-purchase-2026-10-30.md`).

## A native codec

Their services encode and decode RSAT objects in Rust from a catalog of descriptors and defaults
extracted from the owned executable, without running the client's codec. Our responder still runs
the client's codec under emulation (`d1-oracle`), which takes seconds per call. A 56578 catalog would
remove that cost, starting with vendor purchases.
