#!/usr/bin/env python3
"""Exercise real sparse-registry responses and registry alias/feature semantics."""
import hashlib
import io
import json
import tarfile
import tempfile
import unittest
from pathlib import Path
from urllib.error import HTTPError
from urllib.request import urlopen

from cargo_consumer_registry import LocalCargoRegistry, cargo_index_record, crate_index_path


class RegistryTests(unittest.TestCase):
    def crate(self, root):
        crate = root / "searchkernel-0.1.0.crate"
        source = b'''[package]
name = "searchkernel"
version = "0.1.0"
links = "test-native"
[dependencies]
renamed = { package = "searchkernel-ir", version = "=0.1.0", optional = true, default-features = false }
[features]
default = ["native"]
native = ["dep:renamed"]
[target.'cfg(unix)'.build-dependencies]
cc = "1"
'''
        with tarfile.open(crate, "w:gz") as archive:
            entry = tarfile.TarInfo("searchkernel-0.1.0/Cargo.toml")
            entry.size = len(source)
            archive.addfile(entry, io.BytesIO(source))
        return crate

    def test_aliases_namespaced_features_targets_and_checksum(self):
        with tempfile.TemporaryDirectory() as raw:
            crate = self.crate(Path(raw))
            name, encoded, version, filename = cargo_index_record(crate)
            record = json.loads(encoded)
            self.assertEqual((name, version, filename), ("searchkernel", "0.1.0", crate.name))
            self.assertEqual(record["cksum"], hashlib.sha256(crate.read_bytes()).hexdigest())
            self.assertEqual(record["features2"]["native"], ["dep:renamed"])
            renamed = record["deps"][0]
            self.assertEqual((renamed["name"], renamed["package"], renamed["req"]), ("renamed", "searchkernel-ir", "=0.1.0"))
            self.assertFalse(renamed["default_features"])
            target = record["deps"][1]
            self.assertEqual((target["target"], target["kind"]), ("cfg(unix)", "build"))

    def test_registry_serves_exact_bytes_and_never_falls_back_for_missing_release_crates(self):
        with tempfile.TemporaryDirectory() as raw:
            crate = self.crate(Path(raw))
            with LocalCargoRegistry([crate]) as registry:
                origin = f"http://127.0.0.1:{registry.server.server_port}/"
                with urlopen(origin + "config.json") as response:
                    self.assertIn("/crates/{crate}/{version}/download", json.load(response)["dl"])
                with urlopen(origin + crate_index_path("searchkernel")) as response:
                    self.assertEqual(json.load(response)["vers"], "0.1.0")
                with urlopen(origin + "crates/searchkernel/0.1.0/download") as response:
                    self.assertEqual(response.read(), crate.read_bytes())
                for path in [crate_index_path("searchkernel-ir"), "crates/searchkernel/9.9.9/download"]:
                    with self.assertRaises(HTTPError) as error:
                        urlopen(origin + path)
                    self.assertEqual(error.exception.code, 404)

    def test_sparse_paths(self):
        self.assertEqual([crate_index_path(name) for name in ["a", "ab", "ABC", "SearchKernel"]],
                         ["1/a", "2/ab", "3/a/abc", "se/ar/searchkernel"])


if __name__ == "__main__":
    unittest.main()
