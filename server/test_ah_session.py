"""End-to-end tests of the activity-host stand-in: a fake client performs the shello handshake,
joins the activity host over the GCM-encrypted stream and checks the host messages that come
back (join response, simulation packets, heartbeat timing, --ah-object-at and --ah-on steps, an
--ah-mission script with --ah-sim-clock).
Uses a cut-down schema, so it runs without the private exported one."""

import json
import socket
import struct
import tempfile
import threading
import time
import unittest
from pathlib import Path

import bap_server as b
import investment
import signon_proto
from test_activity_join import ID_TYPE, SCHEMA as JOIN_SCHEMA, nested, opt_nested, prim

IDS_TYPE, STATE_TYPE = 0x80809001, 0x80809002
PEER = {i: 0x10 + i for i in range(8)}


def schema():
    s = dict(ID_TYPE)
    s.update(JOIN_SCHEMA)
    s[f"{b.JOIN_REQUEST_TYPE:#x}"] = {"size": 0x20, "fields": [
        prim(0, 9, 32), prim(8, 10, 64), nested(0x10, IDS_TYPE)]}
    s[f"{IDS_TYPE:#x}"] = {"size": 8, "fields": [nested(0, 0x8080405E)]}
    s["0x80800b83"] = {"size": 0xC, "fields": [opt_nested(0, 0x8080405E), prim(8, 4, 16, 0, 1), prim(0xA, 2, 1)]}
    s["0x8080540e"] = {"size": 0x30C, "fields": [nested(0xC * i, 0x80800B83) for i in range(65)]}
    s[f"{b.ROSTER_TYPE:#x}"] = {"size": 0x310, "fields": [prim(0, 5, 32, 0x80000000, 1), nested(4, 0x8080540E)]}
    s[f"{b.OBJECT_ID_TYPE:#x}"] = {"size": 8, "fields": [prim(0, 9, 32), prim(4, 3, 6, 1), prim(6, 4, 16, 0x8000)]}
    s[f"{STATE_TYPE:#x}"] = {"size": 4, "fields": [prim(0, 3, 7)]}  # a u7, like one objective state
    return s


def read_frame(sock):
    head = b.exact(sock, 6)
    _, kind, length = struct.unpack(">BBI", head)
    return kind, b.exact(sock, length)


class FakeClient:
    def __init__(self, sock):
        self.sock = sock
        inner = struct.pack(">HI", b.SHELLO_REQ, 1)
        sock.sendall(struct.pack(">BBI", 1, 2, len(inner)) + inner)
        kind, body = read_frame(sock)
        assert kind == 2 and struct.unpack_from(">H", body)[0] == b.SHELLO_RSP
        plain = b.unseal(signon_proto.TEST_SESSION_KEYS[1], signon_proto.TEST_SESSION_KEYS[2], body[8:])
        nonce, key = plain[:12], plain[12:28]
        self.tx = b.GcmChannel(key, nonce[:11] + bytes([nonce[11] ^ 1]))
        self.rx = b.GcmChannel(key, nonce)
        self.msg_id = 1

    def notify(self, ah_type, body, host=1):
        self.msg_id += 1
        payload = struct.pack(">QBIII", host, 1, ah_type, len(body), 0) + body  # u32 0 before the body
        sealed = self.tx.seal(struct.pack(">HI", b.AH_NOT, self.msg_id) + payload)
        self.sock.sendall(struct.pack(">BBI", 1, 1, len(sealed)) + sealed)

    def pushes(self, timeout=0.05):
        """Host notifications received so far: (arrival time, subtype, data)."""
        out = []
        self.sock.settimeout(timeout)
        try:
            while True:
                kind, body = read_frame(self.sock)
                plain = self.rx.open(body)
                msg_type = struct.unpack_from(">H", plain)[0]
                if msg_type == b.BC_NOT:
                    _, _, subtype, length = struct.unpack_from(">BQII", plain, 6)
                    out.append((time.time(), subtype, plain[6 + 17:6 + 17 + length]))
        except (socket.timeout, TimeoutError):
            pass
        return out


def objects_in(s, data):
    """{(group, kind, index): state or None} from a simulation packet."""
    r = investment.BitReader(data)
    r.get(8 + 1 + 65 + 1)
    investment.decode_from(r, s, b.ROSTER_TYPE)
    r.get(32)
    found = {}
    if r.get(1):
        while r.get(1):
            ident = investment.decode_from(r, s, b.OBJECT_ID_TYPE)
            length = r.get(32)
            end = r.position + length
            r.get(1)
            found[(ident[0], ident[4], ident[6])] = investment.decode_from(r, s, STATE_TYPE) if r.get(1) else None
            r.position = end
    return found


def host_tick(s, data):
    """The u32 host tick of a simulation packet."""
    r = investment.BitReader(data)
    r.get(8 + 1 + 65 + 1)
    investment.decode_from(r, s, b.ROSTER_TYPE)
    return r.get(32)


class ActivityHostSession(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)  # cleanups run last-in first-out: this one last
        self.schema = schema()
        path = Path(self.tmp.name) / "schema.json"
        path.write_text(json.dumps(self.schema), encoding="utf-8")
        self.path = path
        self.logs = []
        self.addCleanup(setattr, b, "log", b.log)  # runs after the server thread has ended
        b.log = self.logs.append

    def start(self, *extra):
        args = b.parse_args(["--bind", "127.0.0.1", "--activity-schema", str(self.path), "--capture-dir",
                             self.tmp.name, "--schema", str(Path(self.tmp.name) / "none.json"),
                             "--ah-join", "--ah-roster", "0", *extra])
        server, client = socket.socketpair()
        thread = threading.Thread(target=b.handle, args=(server, ("127.0.0.1", 0), args), daemon=True)
        thread.start()
        self.addCleanup(thread.join, 2)
        self.addCleanup(client.close)
        c = FakeClient(client)
        request = investment.encode(self.schema, b.JOIN_REQUEST_TYPE, {0: 7, 8: 9, 0x10: {0: PEER}})
        c.notify(b.AH_JOIN_REQUEST, request)
        return c

    def run_client(self, c, seconds, reports=()):
        """Send type 15 every 30 ms for `seconds` (body 00, or a scheduled body); collect pushes."""
        pushes, start, reports = [], time.time(), sorted(reports)
        while time.time() - start < seconds:
            body = b"\x00"
            if reports and time.time() - start >= reports[0][0]:
                body = reports.pop(0)[1]
            c.notify(15, body)
            pushes += c.pushes(0.03)
        return start, pushes

    def test_join_response_and_first_simulation_packet(self):
        c = self.start("--ah-baselines", "0x10/3/0")
        pushes = c.pushes(0.5)
        subtypes = [p[1] for p in pushes]
        self.assertEqual(subtypes, [b.AH_JOIN_RESPONSE, b.AH_SIMULATION])
        response = investment.decode(self.schema, b.JOIN_RESPONSE_TYPE, pushes[0][2])
        self.assertEqual(response[0][0], 7)
        self.assertEqual(objects_in(self.schema, pushes[1][2]), {(0x10, 3, 0): None})

    def test_default_heartbeat_without_world_state_keeps_empty_type_eleven(self):
        c = self.start("--ah-heartbeat", "0.01")
        # The join is processed on a separate thread. Wait for its response before
        # timing heartbeats, rather than assuming Windows schedules it within 100 ms.
        initial = c.pushes(0.5)
        self.assertIn(b.AH_JOIN_RESPONSE, [subtype for _, subtype, _ in initial])
        _, pushes = self.run_client(c, 0.4)
        heartbeats = [data for _, subtype, data in initial + pushes if subtype == b.AH_ESTABLISH]
        self.assertGreaterEqual(len(heartbeats), 2)
        self.assertTrue(all(data == b"" for data in heartbeats))

    def test_heartbeats_keep_their_interval_across_object_steps(self):
        # TEST361: an --ah-object-at step delayed the next heartbeat by the step's time (baboon)
        c = self.start("--ah-heartbeat", "0.2:-1", "--ah-baselines", "0x10/3/0",
                       "--ah-object-at", '0.5@0x10:3:0:0x80809002={"0": 5}')
        start, pushes = self.run_client(c, 1.6)
        sims = [(t - start, objects_in(self.schema, d)) for t, s, d in pushes if s == b.AH_SIMULATION]
        times = [t for t, _ in sims]
        self.assertGreaterEqual(len(times), 6)
        self.assertLess(max(y - x for x, y in zip(times, times[1:])), 0.45)
        states = [o[(0x10, 3, 0)] for _, o in sims]
        self.assertIsNone(states[0])
        self.assertEqual(states[-1], {0: 5})
        first_step = next(t for t, o in sims if o[(0x10, 3, 0)] == {0: 5})
        self.assertGreaterEqual(first_step, 0.45)

    def test_on_rule_steps_an_object_when_the_client_reports(self):
        c = self.start("--ah-heartbeat", "0.1:-1", "--ah-baselines", "0x10/3/0,0x10/3/1",
                       "--ah-on", '15/^41=>0@0x10:3:1:0x80809002={"0": 2}')
        start, pushes = self.run_client(c, 1.2, reports=[(0.6, b"\x41")])
        sims = [(t - start, objects_in(self.schema, d)) for t, s, d in pushes if s == b.AH_SIMULATION]
        before = [o[(0x10, 3, 1)] for t, o in sims if t < 0.55]
        after = [o[(0x10, 3, 1)] for t, o in sims if t > 0.9]
        self.assertTrue(before and all(state is None for state in before))
        self.assertTrue(after and all(state == {0: 2} for state in after))
        self.assertEqual(sum("--ah-on rule 0 matched" in line for line in self.logs), 1)

    def test_events_log_records_frames_and_host_messages(self):
        c = self.start("--ah-heartbeat", "0.1:-1", "--ah-baselines", "0x10/3/0")
        # 0.3 s was not always long enough for a slow runner to log the report (#191).
        self.run_client(c, 1.0, reports=[(0.1, b"\x41")])
        path = next(Path(self.tmp.name).glob("*/events.jsonl"))
        events = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]
        host = [e for e in events if e.get("ev") == "ah"]
        self.assertEqual(host[0]["ah_type"], b.AH_JOIN_REQUEST)
        self.assertIn({"ah_type": 15, "body": "41"}, [{k: e[k] for k in ("ah_type", "body")} for e in host])
        self.assertTrue(any(e.get("ev") == "frame" and e["type"] == b.SHELLO_REQ for e in events))

    def check_routine_notification_capture(self, verbose):
        extra = ("--verbose-wire",) if verbose else ()
        c = self.start("--ah-baselines", "0x10/3/0", "--ah-heartbeat", "0.05:-1", "--ah-on",
                       '15/^00=>0@0x10:3:0:0x80809002={"0":9}', *extra)
        for _ in range(64):
            c.notify(15, b"\x00")
            time.sleep(0.003)
        c.notify(15, b"\x41")
        c.notify(b.AH_CLIENT_STATE, b"\x00")
        pushes = c.pushes(0.3)
        self.assertTrue(any(subtype == b.AH_SIMULATION
                            and objects_in(self.schema, data).get((0x10, 3, 0)) == {0: 9}
                            for _, subtype, data in pushes))
        folder = next(Path(self.tmp.name).glob("*/events.jsonl")).parent
        events = [json.loads(line) for line in (folder / "events.jsonl").read_text().splitlines()]
        frames = [e for e in events if e.get("ev") == "frame"]
        self.assertEqual([e["n"] for e in frames], list(range(1, 69)))
        self.assertEqual(len(list(folder.glob("*-client-plain.bin"))), 67)
        self.assertEqual(len(list(folder.glob("*-client-kind*.bin"))), 68)
        routine = [e for e in events if e.get("ev") == "ah"
                   and e["ah_type"] == 15 and e["body"] == "00"]
        self.assertEqual(len(routine), 64)
        self.assertTrue(any(e.get("ev") == "ah" and e["ah_type"] == b.AH_CLIENT_STATE
                            for e in events))
        printed = [line for line in self.logs if "c->ah not host 0x1 type 15 len 1 body 00" in line]
        self.assertEqual(len(printed), 64 if verbose else 0)
        self.assertTrue(any("c->ah not host 0x1 type 15 len 1 body 41" in line for line in self.logs))
        self.assertTrue(any("c->ah not host 0x1 type 21" in line for line in self.logs))

    def test_routine_console_suppression_preserves_capture_and_reactions(self):
        self.check_routine_notification_capture(verbose=False)

    def test_verbose_wire_retains_routine_console_lines(self):
        self.check_routine_notification_capture(verbose=True)

    def test_mission_script_with_clock_and_trigger(self):
        mission = {"objects": {"timed": "0x10:3:0:0x80809002", "next": "0x10:3:1:0x80809002",
                               "trig": "0x10:3:2:0x80809002"},
                   "steps": [{"at": 0.3, "set": {"timed": {"0": "$now"}}},
                             {"on": {"type": "any", "object": "trig"}, "set": {"next": {"0": 9}}}]}
        path = Path(self.tmp.name) / "mission.json"
        path.write_text(json.dumps(mission), encoding="utf-8")
        c = self.start("--ah-heartbeat", "0.1:-1", "--ah-baselines", "0x10/3/0,0x10/3/1,0x10/3/2",
                       "--ah-sim-clock", "100", "--ah-mission", str(path))
        # the client reports the trigger's id (at an odd bit offset) in a type-17 message at 0.7 s
        w = investment.BitWriter()
        w.put(1, 3)
        investment.encode_into(w, self.schema, b.OBJECT_ID_TYPE, {0: 0x10, 4: 3, 6: 2})
        start, pushes = self.run_client(c, 1.3)  # type 15 only: must not fire the rule
        c.notify(17, w.data())
        more = [p for p in self.run_client(c, 0.5)[1] if p[1] == b.AH_SIMULATION]  # the loop runs on traffic
        sims = [(t - start, objects_in(self.schema, d), host_tick(self.schema, d))
                for t, st, d in pushes if st == b.AH_SIMULATION]
        ticks = [tick for _, _, tick in sims]
        self.assertEqual(ticks, sorted(ticks))
        self.assertGreater(ticks[-1], 60)  # about 100 ticks a second since the first packet
        stamped = [o[(0x10, 3, 0)] for t, o, _ in sims if t > 0.5]
        self.assertTrue(stamped and 15 <= stamped[-1][0] <= 60)  # "$now" at the 0.3 s step
        self.assertTrue(all(o[(0x10, 3, 1)] is None for _, o, _ in sims))
        self.assertTrue(more and objects_in(self.schema, more[-1][2])[(0x10, 3, 1)] == {0: 9})


if __name__ == "__main__":
    unittest.main()
