"""Private, immutable derived run receipt; no gameplay mutations."""
import datetime
import hashlib
import json
import re
from pathlib import Path

runtime = Path(r"F:\d1\<pc>-import\d1xenia\local")
research = runtime / "RESEARCH"
logs = runtime / "SIGNON_LOCAL" / "logs"
native = logs / "JOIN217-pre-stop-native.log"
text = native.read_text(encoding="utf-8", errors="replace")
counts = {f"0x{pc:x}": len(re.findall(rf"D1P .*?pc=0x{pc:x}\b", text))
          for pc in (0x2b36f8, 0x2b3240, 0x2b279c, 0x2b2a60, 0x2b2f74, 0x472518)}
backend = (logs / "FINAL124_JOIN217-bap.log").read_text(encoding="utf-8", errors="replace")
closure = json.loads((research / "JOIN217-independent-closure-verified.json").read_text(encoding="utf-8-sig"))
cleanup = json.loads((logs / "JOIN217-cleanup-result.json").read_text(encoding="utf-8-sig"))
snapshots = [json.loads((research / name).read_text(encoding="utf-8")) for name in (
    "JOIN217-loot-at-revival.json", "JOIN217-loot-during-wall-route.json", "JOIN217-loot-after-rifle.json")]
assert counts["0x2b36f8"] and counts["0x2b3240"]
assert not any(counts[pc] for pc in ("0x2b279c", "0x2b2a60", "0x2b2f74", "0x472518"))
assert closure["all_exact_owned_closed"] and closure["all_hashes_match"]
assert cleanup["all_owned_closed"] and not cleanup["errors"] and not cleanup["active_controls"]
assert not re.search(r"\bservice 601\b", backend)
assert all(len(s["records"]) == 2 and s["cache_count"] == 0 for s in snapshots)
assert all(not r["position_matches_current_world"] and r["unit_reference"] == ["0xffffffff"] * 2
           for s in snapshots for r in s["records"])
evidence = [native, logs / "FINAL124_JOIN217-bap.log", logs / "FINAL124_JOIN217-ahl.log",
            research / "JOIN217-independent-closure-verified.json", logs / "JOIN217-cleanup-result.json",
            research / "JOIN217-loot-overlay.json", research / "JOIN217-overlay-preflight.json",
            research / "JOIN217-rifle-approach" / "result.json"]
receipt = {
    "recorded_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "source_base": "28455edb058faf0566a2f52cdb83e683b39a62bb",
    "tested_source": "dirty source; exact diff/status in JOIN217-prepare.json and tested-source.diff",
    "native_trace_hits": counts, "backend_service_601_observed": False,
    "samples": [{"utc": s["captured_utc"], "world": s["current_world_id"],
                 "pending_records": len(s["records"]), "cached_nonces": s["cache_count"]} for s in snapshots],
    "scope": "Synthetic pending rifle records, unarmed starter, ordinary guarded movement, read-only guest memory. No authentic reward nonce, native claim, grant, combat, graphics fix or RSX capture accepted.",
    "cleanup_utc": cleanup["at"], "independent_closed_count": len(closure["checks"]),
    "configuration_restored": True,
    "validation_previously_completed_this_continuation": {
        "server": {"tests": 202, "pass": True, "seconds": 13.843},
        "scripts": {"tests": 171, "pass": True, "seconds": 1.274},
        "player_db": {"tests": 12, "pass": True},
        "loot_probe": {"tests": 5, "pass": True},
        "mission_final_schema": {"timed": 1, "reactive": 58, "pass": True},
        "database_counterfactual": "Four new methods initially produced seven failures and three errors against old implementation; repaired suite passes. No separate raw test log was saved."},
    "evidence": [{"path": str(p), "bytes": p.stat().st_size,
                  "sha256": hashlib.sha256(p.read_bytes()).hexdigest()} for p in evidence],
    "late_evidence": "Next consolidated report; October 8 report delivered at 16:25 UK stays immutable. No new NAS delivery claimed."
}
destination = research / "JOIN217-checkpoint-receipt.json"
with destination.open("x", encoding="utf-8") as output:
    json.dump(receipt, output, indent=2)
    output.write("\n")
print(json.dumps({"receipt": str(destination), "native_trace_hits": counts,
                  "closed": len(closure["checks"]), "restored": True}))
