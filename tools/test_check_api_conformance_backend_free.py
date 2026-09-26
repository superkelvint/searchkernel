#!/usr/bin/env python3
"""Regression test for the backend-free conformance guard."""

from __future__ import annotations

import os
from pathlib import Path
import stat
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
CHECKER = ROOT / "tools/check-testkit-backend-free.py"
VERIFY_SOURCE = ROOT / "reference/api-conformance/verify.sh"


class BackendFreeConformanceGuardTest(unittest.TestCase):
    def test_missing_input_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            result = subprocess.run(
                ["python3", str(CHECKER), str(Path(temporary) / "missing")],
                text=True,
                capture_output=True,
                check=False,
            )

            self.assertEqual(result.returncode, 2)

    def test_forbidden_dependency_fails_when_rg_is_unavailable(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            conformance = root / "reference/api-conformance"
            testkit = root / "core/crates/searchkernel-testkit"
            tools = root / "tools"
            fake_bin = root / "bin"
            conformance.mkdir(parents=True)
            (testkit / "src").mkdir(parents=True)
            tools.mkdir()
            fake_bin.mkdir()

            verify = conformance / "verify.sh"
            verify.write_text(VERIFY_SOURCE.read_text())
            verify.chmod(verify.stat().st_mode | stat.S_IXUSR)
            fixtures = conformance / "verify-fixtures.sh"
            fixtures.write_text("#!/bin/sh\nexit 0\n")
            fixtures.chmod(fixtures.stat().st_mode | stat.S_IXUSR)
            (tools / CHECKER.name).write_text(CHECKER.read_text())
            (tools / "resolve-python.sh").write_text(
                "#!/bin/sh\nprintf '%s\\n' /usr/bin/python3\n"
            )
            (tools / "resolve-rust.sh").write_text(
                f'#!/bin/sh\nprintf \'%s\\n\' "{fake_bin / "cargo"}"\n'
            )

            (testkit / "Cargo.toml").write_text("searchkernel-vespa = \"forbidden\"\n")
            (fake_bin / "cargo").write_text("#!/bin/sh\nexit 0\n")
            (fake_bin / "cargo").chmod(
                (fake_bin / "cargo").stat().st_mode | stat.S_IXUSR
            )
            (fake_bin / "bash").write_text('#!/bin/sh\nexec /bin/bash "$@"\n')
            (fake_bin / "bash").chmod(
                (fake_bin / "bash").stat().st_mode | stat.S_IXUSR
            )
            (fake_bin / "dirname").write_text(
                '#!/bin/sh\n\nif [ "$1" = "--" ]; then\n    shift\nfi\nprintf \'%s\\n\' "${1%/*}"\n'
            )
            (fake_bin / "dirname").chmod(
                (fake_bin / "dirname").stat().st_mode | stat.S_IXUSR
            )

            environment = os.environ.copy()
            environment["PATH"] = str(fake_bin)
            result = subprocess.run(
                [str(verify)],
                cwd=root,
                env=environment,
                text=True,
                capture_output=True,
                check=False,
            )

            self.assertNotEqual(
                result.returncode,
                0,
                f"backend-free guard accepted a forbidden dependency:\n{result.stdout}\n{result.stderr}",
            )
            self.assertIn("searchkernel-testkit must remain backend-free", result.stderr)


if __name__ == "__main__":
    unittest.main()
