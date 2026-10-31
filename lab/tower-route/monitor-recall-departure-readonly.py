"""Owned 56578 recall/departure evidence monitor. READ+QUERY only; never sends input.

Only private JSON/JSONL output is written. Follow files, logs, game/control files,
process memory and schedules are read only. No debugger, guest calls or UI use.
"""
import argparse
import ctypes
import datetime as dt
import json
import re
import runpy
import struct
import sys
import time
from pathlib import Path

P = Path(__file__).resolve().parent
sys.path.insert(0, str(P)); import routepaths  # noqa: E402
STAGING = routepaths.staging()
BUILD_TABLE = routepaths.fixtures() / "characters" / "activity-table.tsv"
EXPECTED_165_HASH = 0x6EAF0959
ANSI = re.compile(r"\x1b\[[0-9;]*m")
LOG_MATCH = re.compile(
    r"0x002[ab]\b|MATCHMAKING CORE REQUEST|matchmaking_core_response|activity follow:"
    r"|CAPTURED udt [34] \(|JoinRequest|JoinResult|world table:|spawn bundle team"
    r"|publishing activity (?:INDEX|name|CHECKSUM)|Disconnect|join.*(?:fail|error)"
    r"|world.*(?:fail|error)|signout_character|0x01f9\b", re.I)


def utc():
    return dt.datetime.now(dt.timezone.utc).isoformat()


def parse_follow(raw, activity_map):
    text = raw.decode("utf-8", "replace").strip()
    try:
        value = int(text, 0)
        if not 0 <= value <= 0xFFFFFFFF:
            raise ValueError("outside u32")
        return {"text": text, "hash": f"0x{value:08x}", "index": activity_map.get(value)}
    except ValueError:
        return {"text": text[:256], "error": "not one u32 activity hash"}


def classify_log(line):
    flags = {}
    if "MATCHMAKING CORE REQUEST" in line and re.search(r"kind=Some\(4\)", line):
        flags["bap42_kind4_after_start"] = True
        m = re.search(r"activity_hash:\s*(\d+)", line)
        if m and int(m[1]) == EXPECTED_165_HASH:
            flags["bap42_kind4_hash165_after_start"] = True
    if "CAPTURED udt 3 (join_request)" in line or "JoinRequest" in line:
        flags["ah_join_after_start"] = True
    if re.search(r"activity follow:.*index\s+165\b|spawn bundle team.*activity index 165\b", line):
        flags["ah_publish165_after_start"] = True
    if re.search(r"world table: activity index 165 -> world", line):
        flags["ah_world_row165_after_start"] = True
    return flags


class Tail:
    """Bounded incremental reads; startup history is never classified as new traffic."""
    def __init__(self, path):
        self.path = path
        try:
            self.offset = path.stat().st_size
        except FileNotFoundError:
            self.offset = 0
        self.pending = b""
        self.pending_start = self.offset

    def poll(self):
        notices, lines = [], []
        try:
            size = self.path.stat().st_size
            if size < self.offset:
                notices.append({"event": "log_truncated", "path": str(self.path)})
                self.offset, self.pending, self.pending_start = 0, b"", 0
            with self.path.open("rb") as f:
                f.seek(self.offset)
                block = f.read(65536)
            self.offset += len(block)
            self.pending += block
            while b"\n" in self.pending:
                raw, self.pending = self.pending.split(b"\n", 1)
                at = self.pending_start
                self.pending_start += len(raw) + 1
                line = ANSI.sub("", raw.decode("utf-8", "replace").rstrip("\r"))
                if LOG_MATCH.search(line):
                    if "CAPTURED udt" in line and " payload:" in line:
                        line = line.split(" payload:", 1)[0] + " payload:[omitted; original remains in log]"
                    lines.append({"path": str(self.path), "offset": at, "line": line[:4096]})
            if len(self.pending) > 65536:
                notices.append({"event": "oversize_log_line_skipped", "path": str(self.path)})
                self.pending, self.pending_start = b"", self.offset
        except OSError as e:
            notices.append({"event": "log_unavailable", "path": str(self.path), "error": str(e)})
        return notices, lines

    def baseline(self):
        if not self.offset:
            return []
        try:
            with self.path.open("rb") as f:
                raw = f.read(min(self.offset, 65536))
            return [ANSI.sub("", s)[:4096] for s in raw.decode("utf-8", "replace").splitlines()
                    if LOG_MATCH.search(ANSI.sub("", s))][-40:]
        except OSError:
            return []


class Native:
    def __init__(self):
        # Audited OpenProcess(0x410): QUERY_INFORMATION | VM_READ only.
        g = runpy.run_path(str(P / "attach-readonly.py"))
        self.b, self.u, self.rd = g["base"], g["u"], g["rd"]
        self.pid, self.k, self.handle = g["ids"][0], g["k"], g["handle"]
        self.k.GetExitCodeProcess.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_ulong)]
        self.k.CloseHandle.argtypes = [ctypes.c_void_p]
        self.world_names = {}
        facts = P / "owned-world-facts.json"
        if facts.exists():
            for row in json.loads(facts.read_text()):
                self.world_names[int(row["tag"], 16)] = row["name"]

    def alive(self):
        code = ctypes.c_ulong()
        return bool(self.k.GetExitCodeProcess(self.handle, ctypes.byref(code))) and code.value == 259

    def close(self):
        self.k.CloseHandle(self.handle)

    def resolve(self, tag):
        assert tag not in (0, 0xFFFFFFFF), "absent content handle"
        signed = struct.unpack("<i", struct.pack("<I", tag))[0] >> 13
        slot = (((signed & 0xFFFFFFFF) >> 18) | 0x3FF) & signed & 0xFFFFFFFF
        desc = self.u(self.u(self.b + 0x1A238A0)) + slot * 64
        data, stride = self.u(desc + 16), self.u(desc + 48, "I")
        assert data and 0 < stride <= 0x100000, "invalid registry geometry"
        cell = data + (tag & 0x1FFF) * stride
        return (cell - (self.u(desc + 52, "i") & self.u(cell + 8))) & ((1 << 64) - 1)

    def fast(self):
        u, rd, b = self.u, self.rd, self.b
        out = {}
        def part(name, fn):
            try:
                out[name] = fn()
            except (AssertionError, ValueError, OSError, struct.error) as e:
                out[name] = {"unavailable": str(e)}
        def selection():
            a = u(b + 0x26BC068)
            assert a, "selection absent"
            return {"type": u(a, "B"), "activity": u(a + 2, "h")}
        def control():
            a = u(b + 0x26CA078)
            assert a, "controller absent"
            return {"state": u(a + 0x1B0, "i"), "cleanup_reason6c4": u(a + 0x6C4, "i"),
                    "cleanup_field6c8": u(a + 0x6C8, "i")}
        def session(index):
            root = u(b + 0x26CA4E0)
            assert root and u(root, "B"), "session manager absent"
            root += index * 0x131C8
            slot = u(root + 8, "i")
            assert slot in (0, 1), "invalid session buffer"
            s = root + slot * 0x98E0 + 0x10
            params = s + 0x6B28
            r = rd(params + 0xEB8 + 0xE0, 32)
            desired = rd(params + 0x9B8 + 0xE0, 24)
            goal = rd(params + 0xE0, 16)
            goal_valid = bool(goal[0] & 1 and goal[12])
            return {
                "state": u(s + 0x8C48, "i"),
                "leader": u(s + 0x628, "i") != -1 and u(s + 0x62C, "i") == u(s + 0x632C, "i"),
                "goal_valid": goal_valid,
                "goal": struct.unpack_from("<i", goal, 4)[0] if goal_valid else None,
                "goal_reason": struct.unpack_from("<i", goal, 8)[0] if goal_valid else None,
                "recall_valid": bool(r[0] & 1), "recall_active_fc": r[28],
                "recall_seconds_e8": struct.unpack_from("<i", r, 8)[0],
                "recall_api_active": bool(r[0] & 1 and r[28] and struct.unpack_from("<i", r, 8)[0] != -1),
                "desired_valid": bool(desired[0] & 1),
                "desired_type": desired[8], "desired_activity": struct.unpack_from("<h", desired, 10)[0],
            }
        def input_state():
            root = u(b + 0x26519D8)
            assert root, "input manager absent"
            controller = u(root + 0x790, "i")
            assert 0 <= controller < 4, "no primary controller"
            raw = rd(b + 0x273ED40 + 0x2A0 + controller * 0x300 + 0x100, 32)
            f = rd(b + 0x279CE58, 17)
            return {"controller": controller, "held106": raw[6],
                    "consumed108": struct.unpack_from("<I", raw, 8)[0],
                    "route110": struct.unpack_from("<I", raw, 16)[0],
                    "route114": struct.unpack_from("<I", raw, 20)[0],
                    "hold_elapsed5c": struct.unpack_from("<I", f, 4)[0],
                    "ordinary64": f[12], "alternate65": f[13],
                    "reward66": f[14], "rewardhold67": f[15], "edge68": f[16],
                    "screen_gate11b": u(b + 0x2757090 + 0x11B, "B"),
                    "ui_context_state": u(b + 0x279D158 + 0x104, "i")}
        part("selection", selection)
        part("world_controller", control)
        part("session0", lambda: session(0))
        part("session1", lambda: session(1))
        part("input", input_state)
        part("departure", lambda: {"handler31": u(b + 0x26CE420 + 0x31, "B"),
                                  "handler32": u(b + 0x26CE420 + 0x32, "B")})
        return out

    def slow(self):
        u, b = self.u, self.b
        out = {}
        try:
            profile = u(b + 0x26BC130 + 0x18)
            plane = u(b + 0x26BC130 + 0x38)
            assert profile and plane, "profile/plane absent"
            out["recall_gate"] = {"completion673": u(profile + 0x5661, "B"),
                                  "plane2838": u(plane + 12 + 2838, "B"),
                                  "plane_valid": u(plane + 8, "B")}
        except (AssertionError, ValueError, OSError) as e:
            out["recall_gate"] = {"unavailable": str(e)}
        try:
            c = u(b + 0x26BC040)
            assert c, "collector absent"
            out["collector"] = {"state": u(c, "B"), "target": u(c + 1, "B"),
                                "activity": u(c + 2, "h"),
                                "xp": [u(c + x, "I") for x in (0x10, 0x14, 0x18)]}
        except (AssertionError, ValueError, OSError) as e:
            out["collector"] = {"unavailable": str(e)}
        try:
            manager = u(b + 0x26CA088)
            assert manager, "world manager absent"
            tag = u(manager + 0x278, "I")
            world = {"scenario_tag": f"0x{tag:08x}", "world_name": self.world_names.get(tag)}
            if tag != 0xFFFFFFFF:
                a = self.resolve(tag)
                world.update({"world_name_hash": f"0x{u(a + 8, 'I'):08x}",
                              "private_checksum": f"0x{u(a + 12, 'I'):08x}",
                              "public_checksum": f"0x{u(a + 16, 'I'):08x}",
                              "bubble_count": u(a + 0x48)})
            out["world"] = world
        except (AssertionError, ValueError, OSError) as e:
            out["world"] = {"unavailable": str(e)}
        try:
            idx = u(b + 0x25D6FF8, "H")
            assert idx != 0xFFFF, "no current content"
            table = b + 0x1AD5000
            stride = u(table + 16)
            assert 0 < stride <= 0x100000
            content = u(table + u(table, "q") + idx * stride + 0x30)
            assert content
            start, count = struct.unpack("<hh", self.rd(content + 0x2BD1C + 16 * 4, 4))
            assert 0 <= count <= 8
            phases = []
            for j in range(count):
                slot = u(content + 0x2A51C + 2 * (start + j), "H")
                assert slot < 4096
                record = content + 0x510 + slot * 0x38
                cp = self.resolve(u(record + 16, "I")) + u(record + 24)
                phases.append(u(cp + 0x1AC, "b"))
            out["lifetime"] = {"content_index": idx, "phases": phases}
        except (AssertionError, ValueError, OSError, struct.error) as e:
            out["lifetime"] = {"unavailable": str(e)}
        return out


def self_test():
    assert parse_follow(b"1846390409\n", {1846390409: 164})["index"] == 164
    assert parse_follow(b"0x6eaf0959\n", {EXPECTED_165_HASH: 165})["index"] == 165
    assert "error" in parse_follow(b"", {})
    assert classify_log("MATCHMAKING CORE REQUEST (msg 42): kind=Some(4) key=Some(MatchmakingKey { activity_hash: 1846390409 })") == {"bap42_kind4_after_start": True}
    line = f"MATCHMAKING CORE REQUEST (msg 42): kind=Some(4) activity_hash: {EXPECTED_165_HASH}"
    assert classify_log(line)["bap42_kind4_hash165_after_start"]
    assert classify_log("spawn bundle team -1 for activity index 165")["ah_publish165_after_start"]
    assert not classify_log("world table: activity index 164 -> world highway").get("ah_world_row165_after_start")
    # A log append split between polls must appear once; old history never reappears.
    import tempfile
    with tempfile.TemporaryDirectory(prefix="recall-monitor-test-", dir=P) as d:
        f = Path(d) / "log.txt"
        f.write_bytes(b"CAPTURED udt 3 (join_request) old\n")
        tail = Tail(f)
        assert not tail.poll()[1]
        with f.open("ab") as out:
            out.write(b"CAPTURED udt 3 (join_")
        assert not tail.poll()[1]
        with f.open("ab") as out:
            out.write(b"request) new\n")
        assert len(tail.poll()[1]) == 1
        assert not tail.poll()[1]
    print("PASS: follow parsing, milestone classification, incremental tail/partial-line controls")


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--run", default="sense55")
    ap.add_argument("--seconds", type=float, default=120)
    ap.add_argument("--log-dir", type=Path)
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        self_test()
        return
    assert re.fullmatch(r"[A-Za-z0-9_-]{1,64}", args.run), "invalid run name"
    assert 0 < args.seconds <= 180, "duration must be 0..180 seconds"
    candidates = list((STAGING / "contained-boot").glob("*-" + args.run))
    logdir = args.log_dir or (max(candidates, key=lambda x: x.stat().st_mtime) if candidates else None)
    assert logdir and logdir.is_dir(), "no matching log directory"
    follow = STAGING / (args.run + "-launched-activity.txt")
    activity_map = {}
    with BUILD_TABLE.open(encoding="utf-8") as f:
        for line in list(f)[:4097]:
            cells = line.split()
            if len(cells) == 2 and cells[0].isdigit():
                activity_map[int(cells[1], 0)] = int(cells[0])
    assert activity_map.get(EXPECTED_165_HASH) == 165
    tails = [Tail(logdir / name) for name in
             ("responder.log", "responder.err.log", "activity-host.log", "activity-host.err.log")]
    stem = routepaths.probes() / f"recall-departure-{args.run}-{time.time_ns()}"
    events_path, summary_path = Path(str(stem) + ".jsonl"), Path(str(stem) + ".summary.json")
    native = Native()
    milestones = {}
    summary = {"run": args.run, "pid": native.pid, "base": hex(native.b), "started_utc": utc(),
               "seconds_requested": args.seconds, "log_dir": str(logdir), "follow_file": str(follow),
               "events": str(events_path), "milestones_after_start": milestones,
               "limits": ["50ms memory samples can miss same-frame flags",
                          "world record indicates loaded scenario, not successful player spawn",
                          "log observations are timestamped when read; log timestamps remain in each line",
                          "startup log history is labelled baseline and never counts as new traffic",
                          "no reattachment or input if the process exits"]}
    started = time.monotonic()
    previous = {}
    events_count = 0
    with events_path.open("x", encoding="utf-8") as evidence:
        def emit(kind, value, console=True):
            nonlocal events_count
            row = {"utc": utc(), "elapsed_s": round(time.monotonic() - started, 3),
                   "kind": kind, "data": value}
            evidence.write(json.dumps(row) + "\n")
            evidence.flush()
            events_count += 1
            if console:
                print(json.dumps(row), flush=True)
        def changed(kind, value):
            if previous.get(kind) != value:
                previous[kind] = value
                emit(kind, value)
        def mark(name):
            if name not in milestones:
                milestones[name] = {"utc": utc(), "elapsed_s": round(time.monotonic() - started, 3)}
        try:
            emit("metadata", summary)
            for tail in tails:
                emit("log_baseline", {"path": str(tail.path), "start_offset": tail.offset,
                                      "head_matches": tail.baseline()}, console=False)
            print(f"READY: READ-ONLY {args.seconds:g}s monitor. Owner controls any tap. Evidence: {events_path}",
                  flush=True)
            next_slow, next_logs = 0.0, 0.0
            while time.monotonic() - started < args.seconds:
                tick = time.monotonic()
                if not native.alive():
                    emit("process_exited", {"pid": native.pid})
                    summary["stop_reason"] = "process_exited"
                    break
                fast = native.fast()
                changed("native_fast", fast)
                s0 = fast.get("session0", {})
                if s0.get("recall_api_active"):
                    mark("native_countdown_active")
                if fast.get("departure", {}).get("handler31") == 1:
                    mark("native_handler31")
                if fast.get("departure", {}).get("handler32") == 1:
                    mark("native_explicit_orbit_handler32")
                if s0.get("goal") == 27:
                    mark("native_goal27")
                if s0.get("desired_valid") and s0.get("desired_activity") == 165:
                    mark("native_desired165")
                if tick >= next_slow:
                    changed("native_slow", native.slow())
                    next_slow = tick + .25
                if tick >= next_logs:
                    try:
                        with follow.open("rb") as f:
                            raw = f.read(2049)
                        value = parse_follow(raw, activity_map) if len(raw) <= 2048 else {"error": "follow file >2048 bytes"}
                        changed("follow_file", value)
                        if value.get("index") == 165:
                            mark("follow_file165_seen")
                    except OSError as e:
                        changed("follow_file", {"error": str(e)})
                    for tail in tails:
                        notices, lines = tail.poll()
                        for note in notices:
                            changed("log_notice:" + str(tail.path), note)
                        for line in lines:
                            emit("log_new", line)
                            for name in classify_log(line["line"]):
                                mark(name)
                    next_logs = tick + .2
                time.sleep(max(0, .05 - (time.monotonic() - tick)))
            else:
                summary["stop_reason"] = "duration_elapsed"
        except KeyboardInterrupt:
            summary["stop_reason"] = "owner_interrupt"
        except Exception as e:
            summary["stop_reason"], summary["error"] = "monitor_error", repr(e)
            emit("monitor_error", {"error": repr(e)})
        finally:
            native.close()
            summary.update({"ended_utc": utc(), "elapsed_s": round(time.monotonic() - started, 3),
                            "event_count_before_summary": events_count, "last_observed": previous})
            emit("summary", summary, console=False)
            summary_path.write_text(json.dumps(summary, indent=2), encoding="utf-8")
    print(f"DONE: {summary_path}", flush=True)
    print(json.dumps(milestones, indent=2), flush=True)
    if summary.get("stop_reason") == "monitor_error":
        raise SystemExit(1)


if __name__ == "__main__":
    main()
