<!-- SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project -->
<!-- SPDX-License-Identifier: GPL-2.0-or-later -->
# Licensing - the position, and the rules that follow from it

**Status: RATIFIED 2026-08-06 by the operator. `d1-server` is GPL-2.0-or-later**, and
`d1-server/LICENSE` carries the GPL-2.0 text explicitly rather than only a Cargo field.

## 1. The decision

`d1-server` is **GPL-2.0-or-later**, deliberately, matching shadPS4. MIT is off the table for this
component, and that is a choice rather than an accident.

**Scope:** the decision covers **`d1-server/`**, which is where the entanglement lives. The
repository's docs and research text are a separate matter; much of that text discusses copyrighted
assets we cannot redistribute regardless of any code licence. Do not add a repo-root LICENSE
implying otherwise without a separate decision.

**Why GPL was already true before it was chosen** - two real entanglements, both recorded in
`d1-server/ATTRIBUTION.md` with files and upstream commits:

| what | provenance | effect |
|---|---|---|
| `d1-server/proto/shadnet.proto` | vendored verbatim from shadPS4; its header reads `SPDX-License-Identifier: GPL-2.0-or-later` | `shadnet-proto` generates code from a GPL-2.0-or-later file |
| `crates/shadnet-wire/src/lib.rs` | written by reading shadPS4's `src/shadnet/client.{h,cpp}` (GPL-2.0); the file cites that source four times | constants and framing mirrored from GPL source |

shadPS4 is itself GPL-2.0, so the position is coherent: we are a companion to a GPL emulator,
licensed the same way. Rebuilding the ShadNet layer from a written spec to free it for a permissive
licence was considered and rejected - the layer talks exclusively to a GPL emulator, so there is no
plausible consumer who needs it permissively.

## 2. The "or later" is load-bearing

It arrived as a Cargo default and happened to land well. **Keep it.** It is what lets us take
GPL-3.0 material; GPL-2.0-**only** could not.

| licence of a would-be source | usable? | note |
|---|---|---|
| GPL-2.0, GPL-2.0-or-later | yes | our own licence; shadPS4 sits here |
| GPL-3.0, LGPL, MPL-2.0 | yes | **only because we said "or later"** |
| Apache-2.0, MIT, BSD, ISC, Zlib | yes | permissive, absorbs into GPL cleanly |
| **AGPL-3.0** | no | project decision, and network-copyleft we do not want |
| **no licence at all** | no | see below |

⛔ **An unlicensed repository is MORE restrictive than AGPL, not less.** No licence means all
rights reserved: no permission to use, copy, modify or derive. The common assumption that "no
LICENSE file means public domain" is exactly backwards, and it is easy to get wrong.

## 3. The rules for prior art

1. **No AGPL-3.0 material enters this project in any form** - not linked, not ported from, and
   **not read**, including as a source of facts extracted by a human or an agent. This is stronger
   than a reference-only posture on purpose: "read the code, write your own" is a practice
   reasonable people disagree about; not reading it is not arguable.
2. **No unlicensed material either**, on the stricter grounds in section 2. **One exception, decided by
   the project owner on 5 November 2026:** Blam Network's (Codie's) unlicensed Destiny work, namely
   tiger's `fukugen-daihon` restored mission scripts and the `tiger_test_harness` server, may be
   read, run and loaded as an **external input from a local checkout**. It is never copied into this
   repository, never redistributed, and only described here in our own words, with credit. A
   licence from Blam Network is still being sought. The alpha scripts decompiled from a leaked
   Bungie build are not quoted. Blam Network's MIT packages (`bullshcript`, `gibbon`, `rsat`,
   `datamine`, `poodle`) are ordinary permissive code under rule 3.
3. **GPL and permissive prior art is in bounds** - fork it, crib from it, port from it, with
   credit. Every derivation gets an entry in `d1-server/ATTRIBUTION.md` naming the specific files
   and the upstream commit, not a vague nod, and every source file carries an SPDX header.
4. **Running an external tool is not a derivative.** Ghidra (Apache-2.0) lives outside the repo and
   nothing from it is linked into `d1-server`; vendoring Apache-2.0 code *into* `d1-server` would
   cost the "or later" option, so do not. ⇒ `design/ghidra-bringup.md`.

⭐ **There is also a technical argument, independent of licence, and it is the stronger one.** Prior
art from other Demonware-era titles implements a transport we measured our client not to use: those
projects are TCP with 3DES-EDE3-CBC and HMAC-SHA1, while our 01.29 client speaks an SCTP-shaped
association over UDP with DTLS (`design/peer-and-platform.md`). Everything that would transfer from
such a project is a constant table, a taxonomy or a handshake order - **facts about a protocol,
which we can and do recover ourselves** - and everything that is genuinely engineered code is the
part that does not match. Anchoring on a 2010-2012 implementation would have pulled our model
toward the wrong transport. **Declining is not merely safe; it is more accurate.**

## 4. ⭐⭐ The source that outranks all prior art: our own binary

The best specification source we have is first-party evidence about our exact build, and it is
unencumbered:

* **103 distinct `bd*` symbols** and **72 Demonware build paths** with module structure
* the **SCTP-shaped chunk names** and their DTLS variants - the actual transport, not a guessed one
* `bdBitBuffer` / `bdByteBuffer`, `BD_NAT_OPEN|MODERATE|STRICT`, `BD_MAX_MESSAGE_SIZE`
* **1,291 `networking:*` diagnostic strings**, which narrate the state machine in the developers'
  own words
* and the attempt ledger: **making the client tell us**, which is the whole methodology

We extracted more from 30 seconds of `strings` than any transfer table predicted would carry over
from prior art. That is the argument for "we can do better anyway", stated as evidence rather than
preference.

## 5. Open, low urgency

**Does a permissive licence matter for any future component?** The only plausible candidate is a
standalone Demonware-layer server, which could have consumers beyond this project. If that ever
matters, keep it clean from the start: spec-then-implement, its own `LICENSE`, and no
copyleft-incompatible source read by its implementer. Cheap to preserve now, expensive to
retrofit - **decide before that code exists, not after**, which is the same trap the vendored proto
walked us into.

---

**Not legal advice.** The line between "facts about a protocol" and "protected expression" is a
judgment call this project cannot settle, and the Bnetd precedent is a reminder that these
questions have teeth. The framing that keeps us on the right side of it is what we are actually
doing: **preservation of an owned copy against services we run ourselves.** If publication becomes
a real plan rather than a possibility, it deserves a human with actual expertise.
