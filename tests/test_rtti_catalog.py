# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""Generated metadata consistency and optional owned-executable regeneration."""
import csv
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("d1_rtti", ROOT / "tools/ps4_setup/d1_rtti.py")
RTTI = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RTTI)
TABLES = ROOT / "ps4/fixtures/56578/rtti"


def rows(name):
    with (TABLES / name).open(encoding="utf-8", newline="") as stream:
        return list(csv.DictReader(stream, delimiter="\t"))


class CatalogTests(unittest.TestCase):
    def test_tables_have_matching_hashes_and_resolvable_edges(self):
        manifest = json.loads((TABLES / "manifest.json").read_text(encoding="utf-8"))
        for name, digest in manifest["tables_sha256"].items():
            self.assertEqual(hashlib.sha256((TABLES / name).read_bytes()).hexdigest(), digest, name)
        classes = {c["typeinfo"]: c for c in rows("classes.tsv")}
        self.assertTrue(classes)
        for c in classes.values():
            for base in filter(None, c["bases"].split(",")):
                self.assertIn(base, classes)
        schemas = {s["tag"]: s for s in rows("schemas.tsv")}
        self.assertTrue(schemas)
        fields = rows("fields.tsv")
        counts = {tag: 0 for tag in schemas}
        for field in fields:
            self.assertIn(field["tag"], schemas)
            counts[field["tag"]] += 1
        for tag, count in counts.items():
            self.assertEqual(count, int(schemas[tag]["field_count"]), tag)
        self.assertEqual(len(classes), manifest["counts"]["classes"])
        self.assertEqual(len(fields), manifest["counts"]["fields"])

    def test_vacuous_export_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "empty"):
            RTTI.validate({"rtti": {"types": []}, "schemas": [], "envelopes": {}})

    @unittest.skipUnless(os.environ.get("D1_EBOOT") and Path(os.environ["D1_EBOOT"]).is_file(),
                         "SKIP: set D1_EBOOT to the owned 56578 executable")
    def test_owned_56578_regenerates_every_table(self):
        command = [str(RTTI.binary()), "--eboot", os.environ["D1_EBOOT"], "wire-catalog"]
        result = subprocess.run(command, check=True, capture_output=True, encoding="utf-8")
        data = json.loads(result.stdout)
        checked = json.loads((TABLES / "manifest.json").read_text(encoding="utf-8"))
        self.assertEqual(data["eboot_sha256"], checked["eboot_sha256"], "requires build 56578")
        with tempfile.TemporaryDirectory() as tmp:
            RTTI.export(data, Path(tmp))
            # The checked export may include optional content mappings. Eboot regeneration
            # compares all executable-derived tables, never silently requires private content.
            for name in checked["tables_sha256"]:
                if name != "services.tsv":
                    self.assertEqual((Path(tmp) / name).read_bytes(), (TABLES / name).read_bytes(), name)


if __name__ == "__main__":
    unittest.main()
