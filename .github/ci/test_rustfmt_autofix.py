#!/usr/bin/env python3
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github" / "workflows" / "rustfmt-autofix.yml"


class RustfmtAutofixWorkflowTests(unittest.TestCase):
    def test_pinned_toolchain_runs_as_checkout_owner(self) -> None:
        source = WORKFLOW.read_text(encoding="utf-8")
        self.assertNotIn('--user "$(id -u):$(id -g)"', source)
        for required in (
            '-e HOST_UID="$(id -u)"',
            '-e HOST_GID="$(id -g)"',
            'chmod o+x /root',
            'setpriv',
            '--reuid="$HOST_UID"',
            '--regid="$HOST_GID"',
            'RUSTUP_HOME=/root/.rustup',
            'CARGO_HOME=/root/.cargo',
            '/root/.cargo/bin/cargo fmt --all',
        ):
            self.assertIn(required, source)


if __name__ == "__main__":
    unittest.main()
