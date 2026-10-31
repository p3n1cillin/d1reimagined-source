import collections, datetime, json, math, re
from pathlib import Path
rt = Path(r"F:\d1\<pc>-import\d1xenia\local")
logs = rt / "SIGNON_LOCAL/logs"
rows = [json.loads(line) for line in (rt / "RESEARCH/JOIN179-session-timeline.jsonl").read_text().splitlines() if line.strip()]
def idle(row, zone):
    return row["native_current"][0] == zone and row["flags_88_8b"][1] == 0 and row["active_loading"] == [zone, -1]
players = [json.loads(line) for line in (rt / "RESEARCH/JOIN179-player-handover/samples.jsonl").read_text().splitlines()]
changes = []
for row in players:
    if "xyz" in row and (not changes or changes[-1]["entity"] != row["entity"]):
        changes.append({key: row[key] for key in ("utc", "entity", "xyz", "state")})
native = (logs / "JOIN179-pre-stop-native.log").read_text(errors="replace")
record = {
    "at": datetime.datetime.now(datetime.timezone.utc).isoformat(), "run": "JOIN179",
    "session_samples": len(rows),
    "session_states": dict(collections.Counter(str(row["sessions"][0]["state"]) for row in rows)),
    "idle112": sum(idle(row, 112) for row in rows),
    "active24": sum(row["native_current"][0] == 24 for row in rows),
    "idle24": sum(idle(row, 24) for row in rows),
    "sample_first_utc": rows[0]["utc"], "sample_last_utc": rows[-1]["utc"],
    "pointer_recheck_failures": sum(not row["pointers_unchanged"] for row in rows),
    "context_lookup_failures": sum(not session.get("context", {}).get("lookup", {}).get("unchanged_during_lookup", True) for row in rows for session in row["sessions"]),
    "context_state_recheck_failures": sum(not session.get("context", {}).get("unchanged_handle_state", True) for row in rows for session in row["sessions"]),
    "player_changes": changes,
    "native_VUID_count": len(re.findall(r"VUID-", native)),
    "native_device_loss_count": len(re.findall(r"VK_ERROR_DEVICE_LOST|device.?lost", native, re.I)),
    "native_fatal_lines": len(re.findall(r"^\u00b7F[ \t]", native, re.M)),
    "scope": "Native non-atomic reads, guarded pad input and explicit host experiment. Automatic handoff and combined graphics fail acceptance. No full-game acceptance."
}
for name, path in {
    "route": rt / "RESEARCH/JOIN179-guardian-route/result.json",
    "continuation": rt / "RESEARCH/JOIN179-new-player-continuation/result.json",
    "doorway": rt / "RESEARCH/JOIN179-doorway-crossing/result.json",
    "cleanup": logs / "JOIN179-cleanup-result.json",
    "rsx": logs / "JOIN179-rsx-capture-result.json",
}.items():
    record[name] = json.loads(path.read_text(encoding="utf-8-sig"))
requested = json.loads((rt / "RESEARCH/JOIN179-manual-breach-request.json").read_text())["at"]
before = [row for row in players if "xyz" in row and row["entity"] == changes[0]["entity"] and
          row["utc"] < requested and row["state"]["engine"] == 3][-1]
after = next(row for row in players if "xyz" in row and row["entity"] == changes[1]["entity"])
record["last_old_player_before_request"] = {key: before[key] for key in ("utc", "entity", "xyz", "state")}
record["replacement_displacement"] = math.dist(before["xyz"], after["xyz"])
record["old_address_during_transition"] = [{key: row[key] for key in ("utc", "entity", "xyz", "state", "hook_age")}
    for row in players if "xyz" in row and row["entity"] == changes[0]["entity"] and row["state"]["engine"] not in (0, 3)]
record["old_address_transition_scope"] = "Old-address reads during phase6 are not accepted as the current player's position. The observer is non-atomic and a recent hook cannot prove unit lifetime through respawn."
(rt / "RESEARCH/JOIN179-final-acceptance.json").write_text(json.dumps(record, indent=2) + "\n")
print(json.dumps({k: v for k, v in record.items() if k not in ("route", "continuation", "doorway", "cleanup", "rsx")}, indent=2))
