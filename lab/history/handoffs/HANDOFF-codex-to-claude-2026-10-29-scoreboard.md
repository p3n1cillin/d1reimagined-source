# Codex to Claude: kind-15 scoreboard draft ready, 29 Oct morning

On-disk hand-back for sense50 / "PS4 build 56578 retarget". The human clarified
that Claude reads these files directly: no desktop paste is needed, and no
Computer Use or window switching is allowed while the lab runs. All further
delivery is through files. CONTINUE analysis is in
`HANDOFF-codex-to-claude-2026-10-29-continue.md` in this same directory.

## Ready: UNSERVED draft

`F:\d1\d1preservation-main\cfw-staging\agr-postgame-scoreboard.draft.txt`

Uses the familiar 22-client outro stop list, replacing only kind15/sub0's
A image with schema **808008d0**, size **0x4d50 / 19792 bytes**. Contains
`@authority 13,2,0` and NO phase7 directive. Synthetic result: 50 kills,
15 precision kills, 2 deaths. All provenance/unknown fields are commented in
the list. No control publication, lab input, host build, process stop or active
schedule edits from me. No public source changes or commits needed.

### Owner test sequence

1. Arrive in outro105 and complete your profile push, while lifetime remains3.
2. Reconcile fresh authority, generation, delivered/pending clients and native
   player identity; the draft carries prior-run common rows and cine29 stop3.
3. Your control: `clients 0x4786c0e0 F:\d1\d1preservation-main\cfw-staging\agr-postgame-scoreboard.draft.txt`
4. Wait at least one resend (~10 seconds). Require native kind15 component
   +0x1b0=1 AND UI scoreboard `[base+0x26adb30]+0xcc8` count=1. Check the UI
   row identity resolved (row+0x18 and player handle row+0) before phase7.
5. THEN publish phase7 separately. Keeping the same scoreboard image in this
   second list avoids redefaulting the source. Do not combine phase7 with the
   FIRST scoreboard delivery: list encoder emits lifetime first, and the UI
   updater deliberately excludes phases7/8/9. An arbitrary 10s sleep alone is
   weaker evidence than the two row-count reads.

The source definition handle is NOT a field in this A image. It is initialized
locally at component+0x4f00, and was valid **80aa3ee6** in sense49. The setter
11dae0 copies 0x4d50 bytes to component+0x1b0, marks +0x4f04 and increments the
copy timestamp/revision at +0x4f08 itself. UI updater3ff9b0 requires +0x4f00 != -1
and detects that revision. Do not invent a field for these trailing runtime data.

### Exact field layout / provenance

All offsets below are decoded A image offsets unless called row-relative.

|Field|Value|Evidence|
|---|---|---|
|A+0 u32 player count|1|schema-derived layout, synthetic one-player choice|
|A+0x10 row0, stride0x4a0|one row|schema8080410c/80804377/808006e0|
|row+0 eight-byte identity|oracle image `d100000000000201`|same kind123 as cine A+a0; prior native identity `01020000000000d1`, recheck fresh run|
|row+8 bool|0|inferred default; meaning unresolved|
|row+c stat count|23|owned scoreboard config80aa3ee6 array+40|
|row+10 stat floats|index0=50, index18=15, index22=2; others0|indices content-derived, numbers explicitly synthetic|
|row+110 reward count|0|schema-derived layout, no invented grant|
|row+120 rewards, stridee0|empty|kind22 item-valued array; copied by400f54 onward|
|A+4a10 team array|count0|inferred/default for one-player story|
|A+4cb4 aggregate float array|count0|inferred/default|
|A+4d18 extra array|count0|inferred/default|
|A+4d3c category hash|811c9dc5|content-derived empty category in authored config/UI|
|A+4d40 time pair|zeros|inferred diagnostic default|

Owned config80aa3ee6 (class8080099c) has 23 player stat definitions, stride20,
with hashes kills232708fc at index0, precision_kills0ea6079d at18, deaths
cfc7f8d0 at22. Their names independently FNV1-match. The configured display
category has exactly three indices **0,18,22**, matching the visible headings.
Do not use the separate 20-entry array at config+30: precision/deaths there
have DIFFERENT indices16/19. Player rows are indexed using array+40 by11cff0
and400900. The draft uses23 entries and18/22.

303410 resolves this identity by comparing to native player table entry+18.
400900 then fills account/character/name fields from that resolved player;
the scoreboard row is not an entire session participant0xc0 record. Account
SOID is not the lookup key. The prior player's native identity happens to be
the character SOID numerically; retain its correct codec representation.

### Validation

- Actual owned object codec: 988 bits /124 bytes, round trip object IDENTICAL.
- Actual current sensor_list parser + linked oracle frame encoder: PASS,
  phase3, 21 non-lifetime clients (lifetime separately generated), **1163 bytes**.
- PRIVATE artifacts/scripts under
  `lab/history\2026-10-29-postgame-codex`:
  build-scoreboard-draft.py, scoreboard-draft-manifest.json,
  scoreboard-object-codec-check.txt, check-scoreboard-list.rs/.exe,
  scoreboard-full-udt5.bin. These payloads are UNSERVED.

Live acceptance and whether CONTINUE enables remain untested. An empty reward
array is deliberate: a scoreboard reward display is not an investment item grant.

### Fresh sense50 identity check, 08:52:58 UK

Bounded READ + QUERY-only process handle, pid11068/base05180000; no input or
memory write. Native player-table entry0 identity is again
`01020000000000d1`, matching the draft's codec-image identity. Its player handle
is ec700000. UI scoreboard count is0 before publication, as expected. Saved
`sense50-identity-readonly.json` in the private folder. This confirms the draft
identity on the fresh run; authority/generation and final outro state remain
owner-controlled checks at publication time.

For precise UI verification, let S=`[base+26adb30]+cc8`: count is at S+0;
first row starts S+8 (stride458), resolved player handle is S+8, replicated
lookup identity is S+58, Guardian-derived identity is S+18. These S-relative
offsets avoid confusing the UI row's base with the store's base.

END EARLY SCOREBOARD HAND-BACK
