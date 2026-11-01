"""Encrypted host teleports with retained targets and fresh arrival deltas (JOIN88/92)."""

import json
import socket
import tempfile
import threading
import time
import unittest
from pathlib import Path
from unittest.mock import patch

import bap_server as bap
from test_ah_session import FakeClient, PEER, schema


class TeleportDeltaSession(unittest.TestCase):
    target = [24, 0x68935472]

    def setUp(self):
        folder = tempfile.TemporaryDirectory()
        self.addCleanup(folder.cleanup)
        self.folder = Path(folder.name)
        self.control = self.folder / "control.jsonl"
        self.control.write_text("", encoding="utf-8")
        self.activity_schema = schema()
        path = self.folder / "schema.json"
        path.write_text(json.dumps(self.activity_schema), encoding="utf-8")
        self.args = bap.parse_args([
            "--capture-dir", str(self.folder), "--schema", str(self.folder / "absent.json"),
            "--activity-schema", str(path), "--ah-join", "--ah-state", "--ah-roster", "0",
            "--ah-world-state", "112", "--ah-group-switch", "--ah-heartbeat", "0.005:world",
            "--ah-control", str(self.control), "--ah-teleport-finish-delay", "0.01",
        ])

        def world_data(_schema, _request, zone, *unused, **fields):
            return json.dumps({"zone": zone, **fields}).encode()

        for mocking in (
            patch.object(bap, "world_state_delta", side_effect=world_data),
            patch.object(bap, "activity_state", return_value=b"activity"),
            patch.object(bap, "client_slice_record", side_effect=lambda _s, body: json.loads(body)),
            patch.object(bap, "client_slice_state", return_value="test delta"),
            patch.object(bap, "TELEPORT_SETTLE_SECONDS", 0.04),
            patch.object(bap, "log"),
        ):
            mocking.start()
            self.addCleanup(mocking.stop)

    def start(self):
        server, sock = socket.socketpair()
        sock.settimeout(2)
        thread = threading.Thread(target=bap.handle, args=(server, ("127.0.0.1", 0), self.args), daemon=True)
        thread.start()
        self.addCleanup(thread.join, 2)
        self.addCleanup(sock.close)
        client = FakeClient(sock)
        client.notify(bap.AH_JOIN_REQUEST, bap.investment.encode(
            self.activity_schema, bap.JOIN_REQUEST_TYPE, {0: 7, 8: 9, 0x10: {0: PEER}}))
        self.collect(client, 0.2)
        return client

    def report(self, client, **fields):
        client.notify(bap.AH_CLIENT_STATE, json.dumps(fields).encode())

    # Windows that wait for a reply use a long duration with `until`, so they return as soon as it
    # comes; 0.02-0.15 s windows failed on slow Windows runners (#192, #200). Windows that assert
    # nothing finishes stay short where a delayed finish is part of the test.
    def collect(self, client, duration, until=None):
        worlds = []
        deadline = time.monotonic() + duration
        while time.monotonic() < deadline:
            client.notify(15, b"\0")
            for _, subtype, data in client.pushes(0.002):
                if subtype == bap.AH_ESTABLISH:
                    worlds.append(json.loads(data) if data else {"empty": True})
            if until and any(until(world) for world in worlds):
                break
        return worlds

    def queue(self, client):
        with self.control.open("a", encoding="utf-8") as stream:
            stream.write(json.dumps({"teleport": f"{self.target[0]}:{self.target[1]:#x}"}) + "\n")
        started = self.collect(client, 1, lambda world: world.get("teleport") == [1, *self.target])
        self.assertTrue(any(world.get("teleport") == [1, *self.target] for world in started), started)
        return started

    def test_target_sent_before_live_teleport_completes_on_state_only_arrival(self):
        client = self.start()
        self.report(client, current=[112, 0x811C9DC5], target=self.target, state=1)
        accepted = self.collect(client, 1, lambda world: world.get("target") == 24)
        self.assertTrue(any(world.get("target") == 24 for world in accepted), accepted)
        started = self.queue(client)
        self.report(client, state=4)
        later = self.collect(client, 0.6)
        finished = [world for world in later if world.get("teleport") == [3, *self.target]]
        self.assertTrue(finished, later)
        self.assertTrue(any(world.get("teleport") is None for world in later), later)
        initial = next(world for world in started if world.get("teleport") == [1, *self.target])
        self.assertEqual(finished[0]["version"], initial["version"])
        self.assertTrue(any(world.get("teleport") is None and world["version"] > initial["version"]
                            for world in later), later)

    def test_default_heartbeat_retains_pending_request_and_version(self):
        self.args.ah_heartbeat = bap.parse_args(["--ah-heartbeat", "0.005"]).ah_heartbeat
        client = self.start()
        started = self.queue(client)
        pending = self.collect(client, 0.2)
        self.assertGreaterEqual(len(pending), 2)
        initial = next(world for world in started if world.get("teleport") == [1, *self.target])
        self.assertTrue(all(world.get("teleport") == [1, *self.target]
                            and world["version"] == initial["version"] for world in pending), pending)

    def test_explicit_numeric_heartbeat_retains_empty_diagnostic_mode(self):
        self.assertEqual(bap.parse_args(["--ah-heartbeat", "1:11"]).ah_heartbeat, (1, 11))
        self.assertEqual(bap.parse_args(["--ah-heartbeat", "1:6"]).ah_heartbeat, (1, 6))

    def test_stale_current_does_not_finish_on_target_only_delta(self):
        client = self.start()
        # JOIN92 reported current 24 long before the host requested it again. The
        # next target/peer delta preceded current -1 while the actual load began.
        self.report(client, current=self.target, target=[112, 0x811C9DC5], state=3)
        self.collect(client, 0.2)
        started = self.queue(client)
        self.report(client, target=self.target, peer_byte=4)
        self.report(client, current=[-1, 0x811C9DC5])
        loading = self.collect(client, 1.0, until=lambda world: True)
        self.assertTrue(loading)
        self.assertTrue(all(world.get("teleport") == [1, *self.target] for world in loading), loading)
        self.assertTrue(all(world["zone"] == 112 and world.get("target") is None
                            for world in loading), loading)

        self.report(client, state=4)
        arrived = self.collect(client, 0.6)
        finished = [world for world in arrived if world.get("teleport") == [3, *self.target]]
        self.assertTrue(finished, arrived)
        initial = next(world for world in started if world.get("teleport") == [1, *self.target])
        # Peer and group updates may advance the version before arrival. The
        # finish keeps the version of the last loading world rather than adding one.
        self.assertGreaterEqual(finished[0]["version"], initial["version"])
        self.assertEqual(finished[0]["version"], loading[-1]["version"])
        self.assertTrue(any(world.get("teleport") is None and
                            world["version"] > finished[0]["version"] for world in arrived), arrived)

    def test_fresh_current_can_complete_without_state_four(self):
        client = self.start()
        self.queue(client)
        self.report(client, current=self.target, state=3)
        # 0.15 s was not always enough for a slow runner to see the completion too (#192).
        arrived = self.collect(client, 0.6)
        self.assertTrue(any(world.get("teleport") == [3, *self.target] for world in arrived), arrived)
        self.assertTrue(any(world.get("teleport") is None for world in arrived), arrived)

    def test_stale_current_does_not_finish_on_peer_only_delta(self):
        client = self.start()
        self.report(client, current=self.target, target=[112, 0x811C9DC5], state=4)
        self.collect(client, 0.2)
        self.queue(client)
        self.report(client, peer_byte=4)
        pending = self.collect(client, 1.0, until=lambda world: True)
        self.assertTrue(pending)
        self.assertTrue(all(world.get("teleport") == [1, *self.target] for world in pending), pending)

        self.report(client, current=self.target)
        arrived = self.collect(client, 0.6)
        self.assertTrue(any(world.get("teleport") == [3, *self.target] for world in arrived), arrived)

    def test_group_switch_waits_for_fresh_current_on_peer_only_delta(self):
        client = self.start()
        self.report(client, current=[112, 0x811C9DC5], target=self.target, state=3)
        switching = self.collect(client, 1.0, until=lambda w: w["zone"] == 112 and w.get("target") == 24)
        self.assertTrue(any(world["zone"] == 112 and world.get("target") == 24
                            for world in switching), switching)
        self.report(client, peer_byte=4)
        pending = self.collect(client, 1.0, until=lambda world: True)
        self.assertTrue(pending)
        self.assertTrue(all(world["zone"] == 112 and world.get("target") == 24
                            for world in pending), pending)

        self.report(client, current=self.target)
        arrived = self.collect(client, 1.0, until=lambda w: w["zone"] == 24 and w.get("target") is None)
        self.assertTrue(any(world["zone"] == 24 and world.get("target") is None
                            for world in arrived), arrived)

    def check_invalidated_target(self, invalid):
        client = self.start()
        self.report(client, current=[112, 0x811C9DC5], target=self.target, state=1)
        self.collect(client, 0.2)
        self.queue(client)
        # Invalidate an already matched target, then send the same state-only arrival delta.
        self.report(client, target=invalid)
        self.report(client, state=4)
        worlds = self.collect(client, 0.4)
        self.assertTrue(worlds)
        self.assertFalse(any(world.get("teleport") == [3, *self.target] for world in worlds), worlds)
        self.assertTrue(all(world.get("teleport") == [1, *self.target] for world in worlds), worlds)

    def test_different_target_does_not_finish_the_previous_teleport(self):
        self.check_invalidated_target([32, 0x12345678])

    def test_explicitly_cleared_target_does_not_finish_the_previous_teleport(self):
        self.check_invalidated_target([-1, 0x811C9DC5])

    def test_zero_clear_record_does_not_finish_a_skywatch_teleport(self):
        self.target = [0, 0x811C9DC5]
        self.check_invalidated_target([0, 0])

    def test_target_cache_does_not_cross_activity_host_connections(self):
        previous = self.start()
        self.report(previous, target=self.target, state=1)
        self.collect(previous, 0.2)
        previous.sock.close()
        client = self.start()
        self.queue(client)
        self.report(client, state=4)
        worlds = self.collect(client, 0.4)
        self.assertTrue(worlds)
        self.assertFalse(any(world.get("teleport") == [3, *self.target] for world in worlds), worlds)

    def test_target_change_cancels_a_pending_delayed_finish(self):
        self.args.ah_teleport_finish_delay = 0.1
        client = self.start()
        self.queue(client)
        self.report(client, target=self.target, state=4)
        pending = self.collect(client, 0.015)
        self.assertFalse(any(world.get("teleport") == [3, *self.target] for world in pending), pending)
        self.report(client, target=[-1, 0x811C9DC5])
        worlds = self.collect(client, 0.15)
        self.assertFalse(any(world.get("teleport") == [3, *self.target] for world in worlds), worlds)


if __name__ == "__main__":
    unittest.main()
