D1 Reimagined — development priority and daily evidence handoff\
\
This is a private instruction for the development PC. Keep it outside public repositories, website roots and release packages. Apply it alongside the existing repository instructions. It updates the content cadence and handoff requirements; the long-term engineering goal remains unchanged.\
\
\## Your primary job\
\
Continue actively developing this repository toward a stable, accurate, fully playable preservation of the complete Destiny 1 experience. Development is the priority. The repository and verified test results are the source of truth; content documents engineering progress and must never drive feature selection.\
\
Inspect relevant code, understand the current implementation and select the highest-impact missing or broken functionality. Move from investigation into practical code changes, tests and validation whenever possible. Reuse existing systems, preserve compatible behaviour, keep changes maintainable and scoped, fix regressions and leave a working checkpoint after each meaningful step. Preserve unrelated work. Record unknowns honestly and compare against original Destiny 1 behaviour when a reliable reference is available.\
\
Priority: core game functionality → stability → fidelity to original behaviour → broader content coverage → multiplayer/backend completeness → polish.\
\
The full target includes character creation/state; missions, objectives, encounters, checkpoints and progression; destinations, traversal, activities and public spaces; combat, weapons, abilities, subclasses, enemies, AI, damage and physics; inventory, equipment, loot, rewards, currencies and vendors; quests, bounties, achievements and unlocks; UI, menus, HUD, loading and activity selection; backend, persistence, profiles and game authentication flows; fireteams, replication, synchronization, networking and eventual multiplayer. Design practical multiplayer-ready and server-authoritative boundaries without allowing speculative architecture work to stall playable progress.\
\
Maintain a concise private engineering record of working, partial, broken and missing systems, blockers, discoveries, architectural decisions, remaining backend/multiplayer work and the next engineering task. Continue engineering after each handoff; do not wait for a content review or publication response.\
\
\## One consolidated project update per day\
\
Aim to supply evidence for ONE meaningful daily update on https\://d1reimagined.com. Multiple clips, commits and development events are evidence for that update, not separate releases. The production PC handles analysis, writing, narration, editing, captions, graphics, rendering, uploads and website delivery. Do not perform those jobs on the development PC.\
\
Use Europe/London for the daily reporting date. The handoff target is 19:00 UK time; the production pass starts at 20:00. Capture as part of normal testing, then prepare a consolidated handoff at the next safe checkpoint near the cutoff. Do not interrupt a critical test or block engineering to hit a publishing deadline. Late evidence rolls into the next eligible daily edition. Preserve the actual event dates and UTC timestamps. A publishing pass may take time to finish; 20:00 is its start, not a promised release time.\
\
At least aim for a factual daily engineering handoff even when the work is not visual: what you investigated or changed, what was tested, what remains blocked and what happens next. Do not invent achievements, repeat unchanged footage or turn every test into content to satisfy a quota. If there is no new progress, say so privately. The production PC decides whether enough evidence exists for a useful update; it must not fabricate an episode.\
\
\## What to capture\
\
Capture the actual game output during useful tests. Prefer a few clear clips over hours of desktop recordings. As a flexible starting point, supply 3–6 useful clips and 2–4 distinct clean screenshots per daily handoff when the day's work supports them; fewer is fine. Usually 20–90 seconds per result is enough. Preserve longer continuous demonstrations when needed to prove a complete sequence. Include several seconds before the action and after the result. Avoid repeated static menus and unnecessary loading footage.\
\
Prioritize:\
\
\- A newly working gameplay interaction, mission step, traversal, combat behaviour, enemy response, inventory operation, persistence result or loading transition.\
\- A reproducible bug before the change and the same scenario after it, with comparable camera, inputs and setup. Link both with a comparison-group ID and explicit before/after roles. Missing before footage should be stated, never recreated deceptively.\
\- A short continuous end-to-end demonstration that shows the claimed outcome: input → visible result → relevant persistence/reload/checkpoint when applicable. A menu rendering, successful RPC or stub response alone does not establish playable gameplay.\
\- A remaining failure or regression where it explains the day's engineering work. Label it explicitly; distinguish emulator/host crashes from game/backend faults and observed behaviour from a proposed cause.\
\- Performance evidence only with recorded measurements, comparable conditions and a described test method. Preserve measured values and limitations; do not infer an improvement from smoother-looking footage alone.\
\- Optional explanatory debug views for private technical reporting, separately recorded from clean game footage. An original-game reference is useful only when lawfully available, with provenance and relevant timecodes; do not acquire or copy footage merely to fill a video.\
\
Use native landscape capture, ideally 1920×1080 or higher and 60 fps when the game and machine genuinely support it; stable native 30 fps or lower resolution is preferable to increased capture load, artificial interpolation or upscaling. Record the true dimensions, frame rate and any gaps. Keep game audio separately from microphone audio where practical. Disable copyrighted music for capture when possible and identify any remaining music. Voice recording is optional; written engineering notes are sufficient for the production PC's narration.\
\
Record only the relevant game window or native game output. Hide personal accounts, notifications, machine names, local paths, console titles, credentials and internal tool/provider windows before capture. Do not add borders, captions, watermarks, editor branding or narration. Save clean screenshots of the actual work, with no debug overlays, personal identifiers or technical data, for consideration on the public site. They must depict the day's result, not unrelated artwork reused as evidence. Keep private diagnostic imagery separate and identify it clearly.\
\
Finalize recordings, probe their media properties and fully decode them before transfer when practical. Never mark a check PASS unless it actually ran. Report interrupted recordings, recovered fragments, missing audio, cropping and frame gaps. Preserve untouched originals locally; a repaired derivative must identify its source, recovery method and limitations. Capture/verification failure must not crash or stop game development.\
\
\## NAS destination and transfer contract\
\
The production PC watches the 500 GB share at:\
\
\`\`\`text\
\\\<lan>\share\incoming\
\`\`\`\
\
Use a machine-specific date folder. For the current development machine:\
\
\`\`\`text\
\\\<lan>\share\incoming\<lab-pc>\YYYY\MM\DD\\\<unique-package-id>.ready\
\`\`\`\
\
The existing \`Y:\incoming\<lab-pc>\\...\` path can be used only after confirming Y: maps to that same share. Do not assume drive letters are identical on both PCs. Machine names, addresses and paths are private provenance, never public copy.\
\
The existing importer requires schema version 1. A package is a directory containing \`metadata.json\`, \`hashes.json\` and every referenced media/report file. Prefer this layout:\
\
\`\`\`text\
\<unique-package-id>.ready/\
&#x20; metadata.json\
&#x20; hashes.json\
&#x20; daily-report.md\
&#x20; events.json\
&#x20; clips/01-before.mp4\
&#x20; clips/02-after.mp4\
&#x20; screenshots/01-clean.png\
\`\`\`\
\
Include only files that actually exist; the layout is illustrative. Use supported source formats: MP4/MOV/MKV/WebM video; PNG/JPG screenshots; WAV/FLAC/M4A/MP3 audio; Markdown/TXT/JSON reports. Use relative forward-slash paths in manifests, no absolute paths or traversal. Every payload must be nonempty. \`hashes.json\` must contain \`metadata.json\` and ALL other payload files, with actual byte size and SHA-256:\
\
\`\`\`json\
{\
&#x20; "schema_version": 1,\
&#x20; "files": {\
&#x20;   "metadata.json": {"size": 1234, "sha256": "\<actual 64-character SHA-256>"},\
&#x20;   "daily-report.md": {"size": 5678, "sha256": "\<actual 64-character SHA-256>"}\
&#x20; }\
}\
\`\`\`\
\
The numbers and hashes above are placeholders, not usable values. Do not hash \`hashes.json\` into itself. Only \`hashes.json\` and an optional \`READY\` marker are exempt from the file manifest. Unmanifested files, zero-length payloads or changed hashes cause rejection.\
\
Build a complete local package in a durable outbox, validate it, copy to \`\<id>.uploading\` on the NAS, and verify EVERY NAS payload against the manifest. Close file handles, then rename that directory on the same share to \`\<id>.ready\`. Do not write a READY marker into an \`.uploading\` directory; it will still be ignored. An alternative finalized directory with \`READY\` created last is supported, but use the established \`.uploading\` → \`.ready\` convention consistently.\
\
Finished packages are immutable. The same \`source_machine\` + \`clip_id\` with different manifest bytes is rejected. Retry a transfer using the same verified identity and contents; corrections require a new ID plus \`supersedes_clip_id\` in private metadata. Never edit, delete or overwrite a previously ready package, especially after ingestion. Do not assign \`devblog\` or \`devblog_id\`; set \`devblog\` to null and let the production PC group evidence and assign the edition.\
\
NAS outages, full storage or transfer failures must leave a durable local outbox for retry, with original capture timestamps. Never delete the only original copy, overwrite older material or fill the share with unchanged recordings. Record free space and alert on insufficient capacity without blocking development. Retention/deletion needs an agreed archive policy; do not invent one.\
\
\## Metadata and daily report\
\
Required importer fields: \`schema_version: 1\`, nonempty strings \`source_machine\` and globally unique \`clip_id\`. Set \`project\` to \`destiny1-preservation\`. Supply \`recorded_at\` as an ISO-8601 UTC string; \`topic\`, \`feature\`, \`event_type\`, \`description\`, \`previous_state\`, \`current_state\`, \`remaining_issue\`, \`git_commit\` and \`game_build\` must be strings if present. Use actual booleans for \`contains_sensitive_information\`, \`requires_manual_review\` and \`contains_voice\`. Allowed \`importance\` values are CRITICAL, MAJOR, USEFUL, SUPPORTING, LOW_VALUE or REJECT. USEFUL is a reasonable default. Unknown evidence is not a passing test.\
\
Add private contextual fields without changing that contract: reporting date/timezone, development-session ID, daily-update ID, milestone IDs, actual commit/branch, dirty-worktree status and build identity. A dirty build must describe the tested uncommitted changes; do not imply the commit alone reproduces them. Record the reporting interval and the IDs of any earlier event packages included in the day's story. Do not resend their files unnecessarily.\
\
\`daily-report.md\` should contain:\
\
1\. One accurate headline and a short plain-language candidate public update, with no implementation details, build/commit identifiers or breakthroughs.\
2\. Private engineering summary: what changed, previous/current behaviour, why it matters, the affected subsystem and the actual implementation approach.\
3\. Tests: exact relevant commands/scenarios, environment/build, expected and observed outcomes, PASS/FAIL/NOT RUN, reproduction steps, regressions and supporting report files. Sanitize logs before handing them off.\
4\. Maturity per claim: Fully Working, Partially Working, Experimental, Stubbed/Mocked, Known Broken or Planned. Specify the tested scope; a fully working isolated feature does not mean the whole game is complete.\
5\. Evidence index: file path, event ID, useful start/end timecodes, what is visibly proved, what is only inferred, comparison IDs/roles, screenshot candidates and suggested crops. Include confidence and original-D1 reference provenance where relevant.\
6\. Remaining bugs, limitations, blockers, next engineering priority and concrete acceptance criteria for that next task.\
7\. Privacy/audio/rights notes: sensitive time ranges, debug overlays, personal text, microphone presence, music, reference ownership and missing checks. Never include actual secrets. A clean flag is not automatic permission to publish.\
8\. Handoff state: package identities already transferred, pending outbox items, hash/decode results, transfer issues and whether today's handoff is complete or partial.\
\
\`events.json\` may provide the same information in structured form: an array of events with IDs, timestamps, feature/milestone, maturity, claims, tests, evidence-file/timecode references and comparison links. Each event should represent a meaningful change, not every frame or unchanged test rerun. These are evidence/context extensions; the importer does not automatically execute them or approve their claims. Group related work into one coherent daily narrative and distinguish unrelated side investigations.\
\
\## Public and invited-member boundaries\
\
Public site: inspected screenshots and concise nontechnical development updates only. Technical data, breakthrough details, video releases and approved original project documents/changes belong in the account-locked area. Access is invitation-only and individually approved by the owner. Never infer membership approval or publish links/files yourself.\
\
The production PC publishes YouTube video releases UNLISTED and embeds them in member pages. Unlisted video links can be reshared; they are not protected video hosting. Future TikTok videos are held because TikTok has no unlisted option. Do not send public-upload instructions or assume the earlier public-video plan remains active. Shared project files need separate explicit approval and must never include extracted game assets/binaries, credentials or internal instructions.\
\
All source packages are private production inputs, even when they contain public screenshot candidates. Member access is not permission to expose personal identities or internal tooling. Keep operator identities, personal accounts, internal prompts, model/vendor references and administrative architecture out of release-ready media and copy. Do not bypass platform disclosures or remove legally required provenance. Rights concerns must be flagged; a disclaimer or account lock cannot guarantee protection from a takedown.\
\
\## Start now and keep going\
\
Inspect the current repository and progress record. Pick the next engineering change that most improves actual playability, implement it, test it and document the real result. Reuse the existing capture/outbox tooling; make only small compatibility fixes if required for this contract, rather than building another production system. Capture useful evidence during verification and send the daily handoff at a safe checkpoint. Log capture completion, package creation, transfer start/end, hash verification, ready transition, retries and outbox state privately.\
\
After handoff, immediately return to engineering. Do not wait for the site, invent progress for a daily quota, request routine editorial approval or spend development time editing videos. Continue toward the complete Destiny 1 preservation goal in verified increments.