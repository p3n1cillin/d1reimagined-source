# SPDX-License-Identifier: GPL-2.0-or-later
"""Incident fact-table controls, plus an optional full owned-content comparison."""

import csv
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("d1_incidents", ROOT / "tools/ps4_setup/d1_incidents.py")
M = importlib.util.module_from_spec(spec)
spec.loader.exec_module(M)


def rows(path):
    with path.open(encoding="utf-8", newline="") as source:
        return list(csv.DictReader(source, delimiter="\t"))


class IncidentFactsTest(unittest.TestCase):
    def test_catalogue_identities_names_and_field_ranges(self):
        catalogue = rows(M.CATALOG)
        self.assertEqual(len(catalogue), 3902)
        self.assertEqual(tuple(catalogue[0]), M.CATALOG_FIELDS)
        self.assertEqual([int(r["index"]) for r in catalogue], list(range(len(catalogue))))
        hashes = [int(r["name_hash"], 16) for r in catalogue]
        self.assertEqual(hashes, sorted(set(hashes)))
        self.assertEqual(len(M.read_names(M.CATALOG)), 213)
        for row in catalogue:
            self.assertTrue(0 <= int(row["payload_selector"]) <= 255)
            aux = row["aux"].split(",")
            self.assertEqual(len(aux), 4)
            self.assertTrue(all(0 <= int(v) <= 65535 for v in aux))
        self.assertEqual(catalogue[965]["name"], "activity_complete")
        self.assertEqual(catalogue[965]["name_hash"], "0x42660690")

    def test_mapping_matches_every_source_hash_and_target_index(self):
        legacy = json.loads(M.LEGACY.read_text(encoding="utf-8"))["records"]
        catalogue, mapping = rows(M.CATALOG), rows(M.MAPPING)
        self.assertEqual(len(mapping), len(legacy))
        self.assertEqual(tuple(mapping[0]), M.MAPPING_FIELDS)
        target_hashes = {r["name_hash"] for r in catalogue}
        unmatched = []
        for old, mapped in zip(legacy, mapping):
            self.assertEqual(int(mapped["index_51539"]), old["index"])
            self.assertEqual(mapped["name_hash"], old["name_hash"])
            if mapped["index_56578"]:
                new = catalogue[int(mapped["index_56578"])]
                self.assertEqual(mapped["name_hash"], new["name_hash"])
                self.assertEqual(mapped["name"], new["name"])
            else:
                self.assertNotIn(old["name_hash"], target_hashes)
                unmatched.append(old["index"])
        self.assertEqual(unmatched, [2114])

    def test_capture_anchors_and_host_incidents(self):
        mapping = rows(M.MAPPING)
        for old, new, name in ((734, 891, "cinematic_finished"),
                               (2120, 2681, "cinematic_started"),
                               (2665, 3403, "player_trigger"),
                               (2426, 3092, "activity_join"),
                               (2938, 3732, "insertion_point"),
                               (808, 990, "tutorial_weapon"),
                               (1170, 1440, "tutorial_character_level"),
                               (789, 965, "activity_complete")):
            with self.subTest(old=old):
                self.assertEqual((int(mapping[old]["index_56578"]), mapping[old]["name"]), (new, name))

    def test_join_uses_hashes_and_preserves_missing_rows(self):
        def record(index, value):
            return {"index": index, "name_hash": f"0x{value:08x}",
                    "payload_selector": 7, "auxiliary_u16": [1, 2, 3, 4]}
        target = [record(0, 30), record(1, 10)]
        old = [record(0, 10), record(1, 20), record(2, 30)]
        catalogue, mapping = M.fact_tables(target, old, {})
        self.assertEqual([r["index_56578"] for r in mapping], [1, "", 0])
        self.assertEqual(catalogue[0]["aux"], "1,2,3,4")
        self.assertEqual(catalogue[0]["name"], "")
        for bad in ([], [record(1, 10)], [record(0, 10), record(1, 10)]):
            with self.subTest(bad=bad), self.assertRaises(ValueError):
                M.fact_tables(bad, old, {})

    def test_name_sources_retain_only_verified_pairs(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            source = root / "names.csv"
            source.write_text("hash,string,source,confidence\n42660690,activity_complete,private,med\n"
                              "0046D7EE,player_left,private,med\n", encoding="utf-8")
            self.assertEqual(M.dictionary_names(root, {0x42660690}), {0x42660690: "activity_complete"})
            note = root / "INCIDENTS.md"
            note.write_text("| `activity_complete` | Some explanation |\n", encoding="utf-8")
            self.assertEqual(M.documented_names(note, {0x42660690}), {0x42660690: "activity_complete"})
            source.write_text("hash,string\n42660690,wrong_name\n", encoding="utf-8")
            with self.assertRaises(ValueError):
                M.dictionary_names(root, {0x42660690})

    def test_full_export_from_owned_content(self):
        owned = os.environ.get("D1_REPO")
        if not owned:
            self.skipTest("D1_REPO not set: owned 56578 packages unavailable (offline fixture controls still run)")
        packages = Path(os.environ.get("D1_PACKAGES", str(Path(owned) /
                        "dumps/later-56578-CUSA00219/CUSA00219/packages")))
        if not packages.is_dir():
            self.skipTest("owned packages absent under D1_REPO; optionally set D1_PACKAGES")
        # Inputs present: missing reader prerequisites must fail, never skip or pass vacuously.
        self.assertTrue(os.environ.get("D1_TIGER"), "owned inputs present: set D1_TIGER to built tiger")
        result, provenance = M.read_client(packages, os.environ["D1_TIGER"], os.environ.get("D1_NODE", "node"))
        self.assertEqual(provenance["tag"], 0x81656598)
        self.assertEqual(provenance["reference_offset"], 0x3c)
        self.assertEqual(result["record_count"], 3902)
        legacy = json.loads(M.LEGACY.read_text(encoding="utf-8"))["records"]
        catalogue, mapping = M.fact_tables(result["records"], legacy, M.read_names(M.CATALOG))
        self.assertEqual(M.tsv_text(catalogue, M.CATALOG_FIELDS), M.CATALOG.read_text(encoding="utf-8"))
        self.assertEqual(M.tsv_text(mapping, M.MAPPING_FIELDS), M.MAPPING.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
