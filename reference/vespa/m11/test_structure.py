#!/usr/bin/env python3
"""Behavioral tests for the M11 structural guard's runtime/oracle boundary."""

from pathlib import Path
import subprocess
import tempfile
import unittest


CHECKER = Path(__file__).with_name("check-structure.sh")


class M11StructureTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.m11 = self.root / "reference/vespa/m11"
        self._write("reference/vespa/m11/check-structure.sh", CHECKER.read_text())
        self._write("core/crates/searchkernel/examples/searchkernel-m11-driver.rs", "fn main() {}\n")
        self._write("reference/vespa/m11/sanitizer/lsan.supp", "")
        self._write("reference/vespa/m11/sanitizer/tsan.supp", "")
        self._write("core/crates/searchkernel-vespa-engine/src/lib.rs", "")
        self._write(
            "core/crates/searchkernel-vespa-engine/src/engine.rs",
            'fn decode(s: &str) { let _ = serde_json::from_str::<serde_json::Value>(s); }\n'
            "// SAFETY: fixture owns the native handle.\n"
            "unsafe impl Send for Engine {}\n"
            "// SAFETY: fixture synchronizes all access.\n"
            "unsafe impl Sync for Engine {}\n",
        )
        self._write(
            "core/crates/searchkernel-vespa-engine/src/native/mod.rs",
            'fn decode(s: &str) { let _ = serde_json::from_str::<serde_json::Value>(s); }\n',
        )
        self._write("core/crates/searchkernel-vespa-engine/Cargo.toml", 'serde_json = "1"\n')
        self._write("core/crates/searchkernel-vespa-bindings/src/lib.rs", "")
        self._write("core/crates/searchkernel-vespa-bindings/Cargo.toml", "")
        self._write("core/native/vespa/engine.cc", "std::shared_mutex lifecycle_gate;\n")

    def _write(self, path, content):
        target = self.root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(content)

    def _check(self):
        return subprocess.run(
            ["bash", str(self.m11 / "check-structure.sh")],
            cwd=self.root,
            capture_output=True,
            text=True,
            check=False,
        )

    def test_engine_json_value_decoding_is_permitted(self):
        result = self._check()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_safety_context_near_file_start_uses_valid_line_range(self):
        self._write(
            "core/crates/searchkernel-vespa-engine/src/engine.rs",
            "// SAFETY: fixture owns the native handle.\n"
            "unsafe impl Send for Engine {}\n"
            "// SAFETY: fixture synchronizes all access.\n"
            "unsafe impl Sync for Engine {}\n",
        )
        result = self._check()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_missing_send_safety_comment_is_rejected(self):
        self._write(
            "core/crates/searchkernel-vespa-engine/src/engine.rs",
            "unsafe impl Send for Engine {}\n"
            "// SAFETY: fixture synchronizes all access.\n"
            "unsafe impl Sync for Engine {}\n",
        )
        result = self._check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "unsafe impl Send for Engine has no adjacent SAFETY justification",
            result.stderr,
        )

    def test_missing_sync_safety_comment_is_rejected(self):
        self._write(
            "core/crates/searchkernel-vespa-engine/src/engine.rs",
            "// SAFETY: fixture owns the native handle.\n"
            "unsafe impl Send for Engine {}\n"
            "unsafe impl Sync for Engine {}\n",
        )
        result = self._check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "unsafe impl Sync for Engine has no adjacent SAFETY justification",
            result.stderr,
        )

    def test_unrelated_safety_comment_cannot_justify_sync(self):
        self._write(
            "core/crates/searchkernel-vespa-engine/src/engine.rs",
            "// SAFETY: fixture owns the native handle.\n"
            "unsafe impl Send for Engine {}\n"
            "\n"
            "// ordinary comment, not a safety justification\n"
            "unsafe impl Sync for Engine {}\n",
        )
        result = self._check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "unsafe impl Sync for Engine has no adjacent SAFETY justification",
            result.stderr,
        )

    def test_driver_json_oracle_is_rejected(self):
        self._write(
            "core/crates/searchkernel/examples/searchkernel-m11-driver.rs",
            'fn main() { let _ = serde_json::from_str::<serde_json::Value>("{}"); }\n',
        )
        result = self._check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("alternate runtime/oracle path", result.stderr)

    def test_bindings_json_oracle_is_rejected(self):
        self._write(
            "core/crates/searchkernel-vespa-bindings/src/lib.rs",
            'let _ = serde_json::from_str::<serde_json::Value>("{}");\\n',
        )
        result = self._check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("alternate runtime/oracle path", result.stderr)

    def test_production_runtime_launch_is_rejected(self):
        for path, code in (
            ("core/crates/searchkernel-vespa-engine/src/engine.rs", 'std::process::Command::new("java");\n'),
            ("core/crates/searchkernel-vespa-engine/src/native/mod.rs", 'Command::new("java");\n'),
            ("core/crates/searchkernel-vespa-bindings/src/lib.rs", 'ProcessBuilder::new("java");\n'),
            ("core/crates/searchkernel/examples/searchkernel-m11-driver.rs", 'JNI_CreateJavaVM();\n'),
        ):
            with self.subTest(path=path):
                previous = (self.root / path).read_text()
                self._write(path, previous + code)
                result = self._check()
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("alternate runtime/oracle path", result.stderr)
                self._write(path, previous)

    def test_other_m11_boundary_checks_remain_active(self):
        path = "core/crates/searchkernel-vespa-bindings/src/lib.rs"
        self._write(path, "use searchkernel_testkit::FakeBackend;\n")
        result = self._check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("forbidden IR/testkit/FFI dependency", result.stderr)


if __name__ == "__main__":
    unittest.main()
