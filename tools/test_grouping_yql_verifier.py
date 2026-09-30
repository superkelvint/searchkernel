#!/usr/bin/env python3
"""Regression coverage for the grouping/YQL Python runtime handoff."""

from __future__ import annotations

import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
YQL_VERIFIER = ROOT / "reference/vespa/m13/grouping-yql/verify-grouping-yql.sh"


class GroupingYqlVerifierRuntimeTests(unittest.TestCase):
    def test_python_showcase_uses_canonical_e2e_runner(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Path(temporary)
            repo = fixture / "repo"
            fakebin = fixture / "bin"
            repo.mkdir()
            fakebin.mkdir()

            verifier = repo / "reference/vespa/m13/grouping-yql/verify-grouping-yql.sh"
            verifier.parent.mkdir(parents=True)
            shutil.copy2(YQL_VERIFIER, verifier)
            verifier.chmod(0o755)

            common = repo / "reference/vespa/common/verify-common.sh"
            common.parent.mkdir(parents=True)
            common.write_text(
                """#!/usr/bin/env bash
set -euo pipefail
fail_verification() { echo "verification error: $*" >&2; exit 1; }
require_pinned_vespa() { :; }
""",
                encoding="utf-8",
            )
            common.chmod(0o755)

            m9 = repo / "reference/vespa/m9-general-grouping-verifier/verify-recursive-grouping.sh"
            m9.parent.mkdir(parents=True)
            m9.write_text(
                "#!/usr/bin/env bash\necho 'recursive grouping ABI verifier PASS'\n",
                encoding="utf-8",
            )
            m9.chmod(0o755)

            validate = repo / "reference/vespa/m13/grouping-yql/validate.py"
            validate.write_text(
                "print('M13 grouping/YQL coverage manifest is valid')\n",
                encoding="utf-8",
            )

            runner_log = fixture / "runner-args"
            runner = repo / "tools/run-python-native-e2e.sh"
            runner.parent.mkdir(parents=True)
            runner.write_text(
                f"#!/usr/bin/env bash\nprintf '%s\\n' \"$*\" > {runner_log}\n"
                "echo 'textual filter parser, clause-local bindings, grammar, and syntax errors PASS'\n",
                encoding="utf-8",
            )
            runner.chmod(0o755)

            resolver = repo / "tools/resolve-python.sh"
            resolver.write_text(
                "#!/usr/bin/env bash\nprintf '%s\\n' /usr/bin/python3\n",
                encoding="utf-8",
            )
            resolver.chmod(0o755)

            bare_python = fakebin / "python3"
            bare_python.write_text(
                """#!/usr/bin/env bash
set -euo pipefail
case "${1:-}" in
  *try_searchkernel_client_api_e2e.py)
    echo 'bare python3 lacks grpc' >&2
    exit 97
    ;;
  *)
    exec /usr/bin/python3 "$@"
    ;;
esac
""",
                encoding="utf-8",
            )
            bare_python.chmod(0o755)

            subprocess.run(["git", "init", "-q"], cwd=repo, check=True)
            environment = os.environ.copy()
            environment["PATH"] = f"{fakebin}:{environment['PATH']}"

            completed = subprocess.run(
                [str(verifier), "--skip-workspace-quality"],
                cwd=repo,
                env=environment,
                text=True,
                capture_output=True,
                check=False,
            )

            self.assertEqual(
                completed.returncode,
                0,
                msg=completed.stdout + completed.stderr,
            )
            self.assertEqual(
                runner_log.read_text(encoding="utf-8").strip(),
                "--prevalidated-native-sdk --include-facade",
            )


if __name__ == "__main__":
    unittest.main()
