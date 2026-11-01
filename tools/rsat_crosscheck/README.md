# rsat_crosscheck

This checks our RSAT codec (`ps4/d1-server/crates/d1-bap`) against Blam Network's independent one,
[`@blamnetwork/rsat`](https://github.com/Blam-Network/tiger/tree/main/packages/rsat) (MIT). Results
are in [`notes/rsat-blam-crosscheck.md`](../../notes/rsat-blam-crosscheck.md).

1. `cargo run -q > vectors.txt` prints 23 of our encodings, one `name hex` per line. The
   checked-in `vectors.txt` is that output.
2. `node crosscheck.cjs [vectors.txt]` decodes each one with Blam schemas that mirror ours,
   re-encodes it with Blam, and compares the bytes. Set `D1_TIGER` to a built checkout of Blam's
   tiger (`npm ci && npm run build`).

The expected result is `26 OK, 2 DIFF`. Both differences are known, and both are on Blam's side:
- its `u32` decodes values ≥ 2³¹ as negative;
- its 2-bit leaf defaults to bias 2, where D1 uses 1.
