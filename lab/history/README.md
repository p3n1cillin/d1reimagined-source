# Lab history

The working folders behind the project's progress, as they were used: scripts, probes, notes,
handoffs between the AI assistants, served drafts and decoded objects. Each dated folder is one piece
of work (`2026-10-29-postgame-codex` is Codex's post-game investigation, `lab-scripts-2026-10-29` the
lab's driving scripts, and so on). `claude-scratchpad/` is the Claude Code session scratchpad,
`handoffs/` the Claude/Codex handoff notes, and `local-tools/` the lab machine's helper scripts that
are not already in `tools/ps4_setup`.

The maintained, portable versions of what a run needs live elsewhere: the served fixtures in
`ps4/fixtures/56578`, and the unattended Tower route in `lab/tower-route`. Scripts here still name the
lab machine's folders. The private folder they came from is written as `lab/history`, and
`D1_ROOT`-style paths such as `F:/d1/...` are the lab's own layout. Read them as a record, and use
the maintained copies to run.

Imported with `tools/import_lab_history.py`, which leaves out:
- game files and content extracted from them (package entries, definitions, memory snapshots);
- videos and screenshots;
- build outputs;
- keys;
- personal data (network addresses, user folders and machine names are replaced on import).

Large data dumps over 1 MB (UI trees, reflection and descriptor dumps) are left out too.
