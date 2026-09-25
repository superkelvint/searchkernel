#!/usr/bin/env python3
"""Tests for exact Cargo-built native adapter runtime resolution."""

from __future__ import annotations

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / ".github" / "ci" / "resolve-native-adapter-lib.py"


class ResolveNativeAdapterLibTests(unittest.TestCase):
    def _event(self, out_dir: Path, *, linked: Path | None = None) -> dict[str, object]:
        adapter_lib = out_dir / "native-adapter" / "lib"
        return {
            "reason": "build-script-executed",
            "package_id": (
                "path+file:///checkout/core/crates/"
                "searchkernel-vespa-bindings#0.1.0"
            ),
            "linked_paths": [f"native={linked or adapter_lib}"],
            "out_dir": str(out_dir),
        }

    def _run(
        self,
        target_root: Path,
        events: list[dict[str, object]],
    ) -> subprocess.CompletedProcess[str]:
        messages = target_root.parent / "cargo-messages.jsonl"
        messages.write_text(
            "\n".join(json.dumps(event) for event in events) + "\n",
            encoding="utf-8",
        )
        return subprocess.run(
            [sys.executable, str(SCRIPT), str(messages), str(target_root)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )

    def _make_adapter(self, out_dir: Path) -> Path:
        lib = out_dir / "native-adapter" / "lib"
        lib.mkdir(parents=True)
        (lib / "libsearchkernel_vespa.so.2").write_bytes(b"adapter")
        return lib

    def test_resolves_exact_adapter_reported_by_cargo(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            target = root / "target"
            out_dir = target / "debug" / "build" / "bindings-hash" / "out"
            adapter = self._make_adapter(out_dir)

            completed = self._run(target, [self._event(out_dir)])

            self.assertEqual(completed.returncode, 0, completed.stderr)
            self.assertEqual(Path(completed.stdout.strip()), adapter.resolve())

    def test_rejects_adapter_path_outside_target_root(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            target = root / "target"
            target.mkdir()
            outside = root / "outside" / "out"
            self._make_adapter(outside)

            completed = self._run(target, [self._event(outside)])

            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("escaped Cargo target root", completed.stderr)

    def test_rejects_missing_runtime_library(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            target = root / "target"
            out_dir = target / "debug" / "build" / "bindings-hash" / "out"
            (out_dir / "native-adapter" / "lib").mkdir(parents=True)

            completed = self._run(target, [self._event(out_dir)])

            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("runtime library is missing", completed.stderr)

    def test_rejects_link_metadata_that_does_not_match_out_dir(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            target = root / "target"
            out_dir = target / "debug" / "build" / "bindings-hash" / "out"
            self._make_adapter(out_dir)
            wrong = target / "stale" / "native-adapter" / "lib"
            wrong.mkdir(parents=True)

            completed = self._run(
                target,
                [self._event(out_dir, linked=wrong)],
            )

            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("did not publish the expected native adapter link path", completed.stderr)

    def test_rejects_multiple_adapter_candidates(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            target = root / "target"
            first = target / "debug" / "build" / "bindings-one" / "out"
            second = target / "debug" / "build" / "bindings-two" / "out"
            self._make_adapter(first)
            self._make_adapter(second)

            completed = self._run(
                target,
                [self._event(first), self._event(second)],
            )

            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("expected exactly one", completed.stderr)


if __name__ == "__main__":
    unittest.main()
