# Reading build 56578's packages with tiger-pkg (1 November 2026)

[tiger-pkg](https://github.com/v4nguard/tiger-pkg), by cohaereo and nblock (MIT), reads Tiger engine
packages. Its compatibility table lists Rise of Iron PS4 packages. It reads ours (build 56578, PS4
01.33) as `d1_roi`.

## Setup

- Clone it outside this repository, for example to `F:\d1\third-party\tiger-pkg`, and build its
  examples: `cargo build --release --examples`.
- It needs `oo2core_3_win64.dll` (Oodle, by RAD Game Tools) in the folder you run it from. Use your
  own copy; it is not in this repository.
- Commands:
  - `unpack -d -v d1_roi <dump>/CUSA00219/packages/ps4_ui_social_003b_0.pkg` lists a package's
    entries.
  - `unpack -s -v d1_roi -o <out> <pkg>` extracts them.
- Extracted content is game data. Keep it outside the repository.

## What it showed

- The dump has 2,105 packages (50 GB). The 52 `ps4_ui_*` packages, without the cinematics, extract
  to 49,026 files (8.2 GB).
- UI content is binary tag structures (for example type 16 with struct tags such as `0x80801AD7`),
  RIFF audio and `BKHD` sound banks, not Scaleform.
- Its readable strings are mostly audio and dialogue cue names (`vendor_hello`,
  `vendor_comment_purchase`, `vendor_screen_confirm_purchase`).
- **The refund list question** ([vendor notes](ps4-56578-vendor-purchase-2026-10-30.md)) stays
  open:
  - "refund" appears only as `mtx_refund_error` (the microtransaction store) and in the licence text.
  - The UI components' 64-bit registration hashes (the refund component's `0x7128FF278C066039` and
    `0xDC317725EE33B80A`, and the interaction display's `0x15A317405039990F` as a control) appear
    nowhere in the UI content, in either byte order or as 32-bit halves.
  - So screens don't name components that way, and how a screen picks its components is still to be
    read, now from the content side as well as the executable.
