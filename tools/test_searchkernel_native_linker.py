#!/usr/bin/env python3
from __future__ import annotations

import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
WRAPPER = ROOT / "tools" / "searchkernel-native-linker.sh"
CONFIG = ROOT / ".cargo" / "config.toml"


class SearchKernelNativeLinkerTests(unittest.TestCase):
    def _fixture(self, root: Path) -> tuple[Path, Path, Path]:
        target = root / "target"
        adapter = (
            target
            / "debug"
            / "build"
            / "searchkernel-vespa-bindings-hash"
            / "out"
            / "native-adapter"
            / "lib"
        )
        adapter.mkdir(parents=True)
        (adapter / "libsearchkernel_vespa.so.2").write_bytes(b"adapter")

        capture = root / "linker-args.txt"
        fake_linker = root / "fake-linker.sh"
        fake_linker.write_text(
            "#!/usr/bin/env bash\n"
            "set -euo pipefail\n"
            "printf '%s\\n' \"$@\" > \"$SEARCHKERNEL_LINKER_CAPTURE\"\n",
            encoding="utf-8",
        )
        fake_linker.chmod(0o755)
        return target, adapter, capture

    def _run(
        self,
        root: Path,
        capture: Path,
        args: list[str],
    ) -> subprocess.CompletedProcess[str]:
        env = os.environ.copy()
        env["SEARCHKERNEL_REAL_LINKER"] = str(root / "fake-linker.sh")
        env["SEARCHKERNEL_LINKER_CAPTURE"] = str(capture)
        return subprocess.run(
            [str(WRAPPER), *args],
            cwd=ROOT,
            env=env,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )

    def test_cargo_config_routes_linux_links_through_wrapper(self) -> None:
        config = CONFIG.read_text(encoding="utf-8")
        self.assertIn("[target.x86_64-unknown-linux-gnu]", config)
        self.assertIn('linker = "tools/searchkernel-native-linker.sh"', config)
        self.assertTrue(os.access(WRAPPER, os.X_OK))

    def test_native_binary_gets_output_relative_origin_runpath(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            target, adapter, capture = self._fixture(root)
            output = target / "debug" / "searchkernel-searchreply-config"
            output.parent.mkdir(parents=True, exist_ok=True)

            completed = self._run(
                root,
                capture,
                ["-o", str(output), f"-L{adapter}", "-lsearchkernel_vespa"],
            )

            self.assertEqual(completed.returncode, 0, completed.stderr)
            args = capture.read_text(encoding="utf-8").splitlines()
            expected = "$ORIGIN/build/searchkernel-vespa-bindings-hash/out/native-adapter/lib"
            self.assertIn("-Wl,--enable-new-dtags", args)
            self.assertIn("-Wl,-z,origin", args)
            self.assertIn(expected, args)

    def test_native_test_binary_gets_parent_relative_origin_runpath(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            target, adapter, capture = self._fixture(root)
            output = target / "debug" / "deps" / "native_test_hash"
            output.parent.mkdir(parents=True, exist_ok=True)

            completed = self._run(
                root,
                capture,
                ["-o", str(output), "-L", str(adapter), "-l", "searchkernel_vespa"],
            )

            self.assertEqual(completed.returncode, 0, completed.stderr)
            args = capture.read_text(encoding="utf-8").splitlines()
            expected = "$ORIGIN/../build/searchkernel-vespa-bindings-hash/out/native-adapter/lib"
            self.assertIn(expected, args)

    def test_non_native_link_is_unchanged(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            target, adapter, capture = self._fixture(root)
            output = target / "debug" / "portable-tool"
            output.parent.mkdir(parents=True, exist_ok=True)

            completed = self._run(
                root,
                capture,
                ["-o", str(output), f"-L{adapter}", "-lm"],
            )

            self.assertEqual(completed.returncode, 0, completed.stderr)
            args = capture.read_text(encoding="utf-8").splitlines()
            self.assertNotIn("-rpath", args)
            self.assertNotIn("-Wl,--enable-new-dtags", args)

    def test_native_link_fails_closed_without_exact_adapter_library(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            target, _adapter, capture = self._fixture(root)
            empty = target / "debug" / "missing-adapter"
            empty.mkdir(parents=True)
            output = target / "debug" / "broken-native-tool"

            completed = self._run(
                root,
                capture,
                ["-o", str(output), f"-L{empty}", "-lsearchkernel_vespa"],
            )

            self.assertEqual(completed.returncode, 70)
            self.assertIn(
                "could not resolve libsearchkernel_vespa.so.2",
                completed.stderr,
            )
            self.assertFalse(capture.exists())


if __name__ == "__main__":
    unittest.main()
