import datetime
import hashlib
import json
import subprocess
from pathlib import Path

rt = Path(r"F:\d1\<pc>-import\d1xenia\local")
repo = Path(r"F:\d1\d1xenia")
progress = rt / "PRIVATE_ENGINEERING" / "2026-10-08-progress.md"
old = progress.read_text(encoding="utf-8-sig")
heading = "## Safe engineering checkpoint — source 578ad60, JOIN217 (2026-10-08 22:05 UTC)"
if heading in old:
    raise RuntimeError("Preserve previous checkpoint")
assert subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip() == "578ad601ad6773bc35ce2294139aff8798a0a284"
assert not subprocess.check_output(["git", "status", "--porcelain"], cwd=repo, text=True).strip()
block = """## Safe engineering checkpoint — source 578ad60, JOIN217 (2026-10-08 22:05 UTC)

Continued from Claude source28455ed. Source578ad601ad6773bc35ce2294139aff8798a0a284 committed and pushed, remote master verified exactly, checkout clean. Data512c9e436c2c80cd2d88e5bc2be2dd02f00c4ca4 remains clean/unchanged. Full Destiny goal remains unfinished. No source/renderer permission or external reply is needed for continued authorized engineering.

Durable backend repairs: retry keys now enforce original account, character (where known), reason and operation; mismatches raiseValueError rather than return someone else's result. Item moves validate a live destination character owned by the item account inside the transaction. New four database methods initially produced7failures/3errors against old code, then pass after repair. Valid reconnect replay stays exactly once; cross-account/deleted/missing destinations leave item unchanged; vault->equip version1->2->3 passes. No migration and native loot/equipment requests still not wired to item grants. Final previously completed validations202serverPASS13.843s,171scriptsPASS1.274s,12DBPASS,five loot probe checksPASS, actual final-schema preflight1timed/58reactivePASS, overlay schema roundtripPASS and staged whitespacePASS. No raw standalone counterfactual test log was saved; its failures were observed in tool output.

Static loot correction: 276b08/276c44 are LOCAL deferred callback arenas, not host wire messages. 1853168 ->2b2f74 inserts client nonce/position cache. 1886af8 ->cbb5d4 ->472518 selects BAPservice601. Wire nonce is opaque, not an established item-hash/instance pair. Self80801714+3bd0 is pending80801701, firstslot+3bd8,64records stride38, source3/5 world-spawn paths. Manager1ab649c has unit pairs, ring header200/204/208,64cache records+210 stride30, fallback+e10. Current worldID is controller*(1a1dc80)+110c handle bits3..8, not manager+1a10 signature. 2b3240 requires newest matching nonce's currentworld position before art/bauble creation. 2b279c/2b2a60 record action data; direct callers5aea30/5aede4. Correct native action creation/replicated event resource remains open. Raw offline ELF disassembly in RESEARCH/loot-static-20261008 stays private; do not import auto-attaching tiger tool live. Entry39 resource18eee3c read value80804c4c is unresolved handle, not an established guest pointer; attempted dereference failed read, no mutation. Private raw values preserved JOIN217-script-type-values.json; don't invent schema fields from it.

JOIN217: bounded owned native C5 run, no mission/renderer change and no manual control-file commands or guest writes. Opening Mars skip normal input as already authorized, later urgent dialog dismissed only after image inspection. Private overlay JOIN217-loot-overlay.json216encodedbytes SHA256078d73fd00be0031f4facffbc590b936d6bc51d1c8910d87199590116bccc1ee adds TWO SYNTHETIC pending rifle records(item3598,sources3/5); not equipped, no authentic reward nonce, no position cache. Native2b36f8/2b3240 each8hits read both records, observed firstslot600ee780 ->self600eaba8, manager600b8f20. Read-only public helper requires exact manifest/observed selfID and rejects changed inputs; three private samples at revival, Wall and afterrifle alltwo records, cachecount0, absent matching slot data and unitrefsFFFFFFFF/FFFFFFFF. Cache-record callbacks2b279c,2b2a60,2b2f74 and serviceentry472518 zero watch hits; backend no service601. These observations validate the diagnostic and missing prerequisite for this synthetic fixture, not authentic reward identity/pickup or a backend grant.

Ordinary guarded routes: canonical Wall drive stopped correctly at slice transition. Breach stairs complete22:02:12.658852Z, platform incident starts production automatic light-up rules22:02:07.383Z; prop channel activations received22:02:32.241..611Z. Rifle approach completes22:03:04.975251Z at[410.579376,-794.043396,19.219030], through automatic barrier. No pickup accepted. Final inspected F12 image23_05_18UK remains dark with LevelUp coach and no first-person rifle. A proximity-driven objective handover elsewhere does not establish a rifle grant. No new clip/video, RSX capture/replay, audio-content acceptance, damage, combat, lighting or combined graphics repair claimed.

JOIN217 closed cooperatively by finish marker22:05:38.9464982Z, wrapperexit0. Independent RESEARCH/JOIN217-independent-closure-verified.json verifies nine exact owned identities closed and all three baseline title/global/watch hashes restored; XInput,RPCN preserved, no active controls/errors. Main immutable derived receipt RESEARCH/JOIN217-checkpoint-receipt.json hashes traces/fixture/route/cleanup; source/prepare evidence JOIN217-tested-source.diff and JOIN217-prepare.json. Renderer checkout stays53dirtyentries(48tracked5untracked), untouched, no build/edit. No helper/watch lease remains. JOIN216 independent closure receipt likewise verifies nine closed and baseline hashes; correct JOIN216 acceptance is pre-equipped rifle R1 firing12->11, R2 doesn't fire, reference25/184, damage untested.

Correction to this continuation's late JOIN210 action: at21:34:53Z a phase5 command was appended to old FINAL124_JOIN210-control.jsonl and JOIN210-activation-5.json created after that run had already closed. It DID NOT EXECUTE. Subsequent probe returnedWinError87 for absent process. Do not cite phase5 as a reopening success. Earlier valid phase3 opened and phase4 reblocked; ClaudeJOIN211 independently proves automatic barrier. Preserve all old records, including original JOIN210-prepare.json; Claude's overwritten prepare.ps1 error remains separately documented below.

Reporting: October8 daily package delivered16:25UK is immutable; JOIN205+ late evidence belongs in NEXT consolidated report, preserving actual October8 dates. Existing JOIN209 original segments retained/hash/decode checked as recorded below; don't resend earlier media. No new NAS delivery claimed by this continuation; no policy file or production inputs in source/packages, no final video edits/publishing.

Next: recover the native action pairing authentic rifle nonce and worldposition, observe native service601 then persist and replicate the idempotent grant. Accept only actual unarmed->Khvostov, referenceammo/HUD/grimoire and proper FightThroughWall order. Lighting/black companion Ghost/first-person darkness/flicker/distortion/Nav null-component crash/extraTakenKingbanner/earlyLevelUp remain open. Continue combat/mission/completion/rewards/Tower plus full backend/progression/multiplayer afterward. No goal completion or pause is implied by this safe checkpoint.

---

"""
progress.write_text(block + old, encoding="utf-8")
source_files = ["server/player_db.py", "server/test_player_db.py",
                "scripts/rpcs3_final124_loot.py", "scripts/test_rpcs3_final124_loot.py"]
record = {"recorded_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
          "source_commit": "578ad601ad6773bc35ce2294139aff8798a0a284", "remote_verified": True,
          "data_commit": "512c9e436c2c80cd2d88e5bc2be2dd02f00c4ca4", "source_clean": True,
          "renderer_preserved": {"tracked": 48, "untracked": 5},
          "progress_path": str(progress),
          "source": [{"path": p, "sha256": hashlib.sha256((repo / p).read_bytes()).hexdigest()}
                     for p in source_files]}
with (rt / "RESEARCH" / "JOIN217-pushed-checkpoint.json").open("x", encoding="utf-8") as output:
    json.dump(record, output, indent=2)
    output.write("\n")
print("Private continuation record saved; source push already independently verified.")
