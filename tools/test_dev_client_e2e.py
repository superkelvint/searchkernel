#!/usr/bin/env python3
"""Regression tests for per-client dev test E2E coverage."""

from __future__ import annotations

import os
import shutil
import shlex
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CLIENTS = ("python", "typescript", "php", "ruby")


class DevClientE2ETest(unittest.TestCase):
    def setUp(self) -> None:
        self.tempdir = tempfile.TemporaryDirectory()
        self.fixture = Path(self.tempdir.name)
        self.repo = self.fixture / "repo"
        self.fakebin = self.fixture / "bin"
        self.repo.mkdir()
        self.fakebin.mkdir()

        shutil.copy2(ROOT / "dev", self.repo / "dev")
        (self.repo / "dev").chmod(0o755)
        shutil.copytree(ROOT / "build", self.repo / "build")
        (self.repo / "tools").mkdir(exist_ok=True)
        shutil.copy2(ROOT / "tools" / "dev-targets.py", self.repo / "tools" / "dev-targets.py")
        (self.repo / "tools" / "dev-targets.py").chmod(0o755)

        subprocess.run(["git", "init", "-q"], cwd=self.repo, check=True)
        subprocess.run(
            ["git", "config", "user.name", "Dev Client E2E Test"],
            cwd=self.repo,
            check=True,
        )
        subprocess.run(
            ["git", "config", "user.email", "dev-client-e2e@example.invalid"],
            cwd=self.repo,
            check=True,
        )

        self._write_resolvers()
        self._write_stub_scripts()
        self._write_fake_tools()

    def tearDown(self) -> None:
        self.tempdir.cleanup()

    def _write(self, path: Path, content: str, *, executable: bool = False) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")
        if executable:
            path.chmod(0o755)

    def _write_resolvers(self) -> None:
        self._write(
            self.repo / "tools" / "resolve-python.sh",
            '#!/usr/bin/env bash\nprintf "%s\\n" "$FAKE_BIN/python"\n',
            executable=True,
        )
        self._write(
            self.repo / "tools" / "resolve-rust.sh",
            """#!/usr/bin/env bash
set -euo pipefail
case "$1" in
  cargo|rustc|rustfmt) printf '%s/%s\\n' "$FAKE_BIN" "$1" ;;
  *) exit 2 ;;
esac
""",
            executable=True,
        )
        self._write(
            self.repo / "tools" / "resolve-node.sh",
            """#!/usr/bin/env bash
set -euo pipefail
case "$1" in
  node|npm) printf '%s/%s\\n' "$FAKE_BIN" "$1" ;;
  *) exit 2 ;;
esac
""",
            executable=True,
        )

    def _write_stub_scripts(self) -> None:
        for relative in (
            "tools/test-typescript-generation.sh",
            "tools/check-typescript-generated.sh",
            "clients/php/tests/verify_package.sh",
        ):
            self._write(
                self.repo / relative,
                "#!/usr/bin/env bash\nexit 0\n",
                executable=True,
            )
        self._write(
            self.repo / "tools" / "check-client-contracts.py",
            "# placeholder; fake python records invocation\n",
        )

    def _write_fake_tools(self) -> None:
        self._write(
            self.fakebin / "python",
            """#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == */tools/dev-targets.py ]]; then
  exec python3 "$@"
fi
if [[ "$*" == *"check-client-contracts.py --run-e2e-surface"* ]]; then
  printf '%s installed_gem=%s\\n' "$*" "${SEARCHKERNEL_RUBY_USE_INSTALLED_GEM:-0}" >>"$E2E_LOG"
fi
exit 0
""",
            executable=True,
        )
        for name in ("cargo", "rustc", "rustfmt", "node", "npm", "php", "ruby"):
            self._write(
                self.fakebin / name,
                "#!/usr/bin/env bash\nexit 0\n",
                executable=True,
            )

    def test_each_client_test_command_runs_its_registered_e2e_lane(self) -> None:
        for client in CLIENTS:
            with self.subTest(client=client):
                e2e_log = self.fixture / f"{client}.e2e.log"
                env = os.environ.copy()
                env.update(
                    {
                        "PATH": f"{self.fakebin}:{env['PATH']}",
                        "FAKE_BIN": str(self.fakebin),
                        "E2E_LOG": str(e2e_log),
                        "SEARCHKERNEL_DEV_USE_CI_IMAGE": "0",
                    }
                )
                completed = subprocess.run(
                    [str(self.repo / "dev"), "test", client],
                    cwd=self.repo,
                    env=env,
                    text=True,
                    capture_output=True,
                    check=False,
                )
                self.assertEqual(
                    completed.returncode,
                    0,
                    msg=completed.stdout + completed.stderr,
                )
                self.assertTrue(
                    e2e_log.is_file(),
                    msg=f"./dev test {client} did not invoke the client E2E runner",
                )
                self.assertIn(
                    f"--run-e2e-surface {client}",
                    e2e_log.read_text(encoding="utf-8"),
                )

    def test_python_real_e2e_wrapper_prepares_runtime_and_opts_in(self) -> None:
        wrapper = (ROOT / "tools" / "run-python-native-e2e.sh").read_text(
            encoding="utf-8"
        )

        self.assertIn('source "$ROOT/tools/client-e2e-runtime.sh"', wrapper)
        self.assertIn('prepare_client_e2e_runtime "$prevalidated_native_sdk"', wrapper)
        self.assertIn("SEARCHKERNEL_RUN_NATIVE_E2E=1", wrapper)
        self.assertIn("-p 'test_grpc_e2e.py'", wrapper)
        self.assertIn('if [[ "$include_facade" == 1 ]]; then', wrapper)
        self.assertIn(
            '"$PYTHON_BIN" clients/python/examples/try_searchkernel_client_api_e2e.py',
            wrapper,
        )

    def test_clean_semantic_parity_prepares_ignored_grpc_client_modules(self) -> None:
        marker = self.fixture / "grpc-generated"
        self._write(self.repo / "clients/typescript/node_modules/.bin/tsc", "#!/bin/sh\nexit 0\n", executable=True)
        self._write(self.repo / "tools/check-flatbuffers-bfbs.sh", "#!/bin/sh\nexit 0\n", executable=True)
        self._write(
            self.fakebin / "npm",
            '#!/bin/sh\ncase "$*" in *generate:grpc-web*) touch "$PARITY_MARKER" ;; esac\n',
            executable=True,
        )
        self._write(
            self.fakebin / "python",
            '''#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == */tools/dev-targets.py ]]; then
  exec PYTHON_EXECUTABLE "$@"
fi
if [[ "${1:-}" == */tools/check-client-semantic-parity.py ]]; then
  [[ -f "$PARITY_MARKER" ]] || { echo 'browser/Node generated modules missing' >&2; exit 11; }
fi
'''.replace("PYTHON_EXECUTABLE", shlex.quote(sys.executable)),
            executable=True,
        )
        env = os.environ.copy()
        env.update({"PATH": f"{self.fakebin}:{env['PATH']}", "FAKE_BIN": str(self.fakebin), "PARITY_MARKER": str(marker), "SEARCHKERNEL_DEV_USE_CI_IMAGE": "0"})
        completed = subprocess.run([str(self.repo / "dev"), "verify", "client-semantic-parity"], cwd=self.repo, env=env, text=True, capture_output=True, check=False)
        self.assertEqual(completed.returncode, 0, msg=completed.stdout + completed.stderr)
        self.assertTrue(marker.is_file(), "clean parity must generate the ignored browser/Node RPC modules before importing SDK source")

    def test_ruby_real_e2e_uses_the_installed_gem(self) -> None:
        e2e_log = self.fixture / "ruby-installed.e2e.log"
        env = os.environ.copy()
        env.update(
            {
                "PATH": f"{self.fakebin}:{env['PATH']}",
                "FAKE_BIN": str(self.fakebin),
                "E2E_LOG": str(e2e_log),
                "SEARCHKERNEL_DEV_USE_CI_IMAGE": "0",
            }
        )
        completed = subprocess.run(
            [str(self.repo / "dev"), "test", "ruby"],
            cwd=self.repo,
            env=env,
            text=True,
            capture_output=True,
            check=False,
        )
        self.assertEqual(
            completed.returncode,
            0,
            msg=completed.stdout + completed.stderr,
        )
        self.assertIn(
            "installed_gem=1",
            e2e_log.read_text(encoding="utf-8"),
        )


if __name__ == "__main__":
    unittest.main()
