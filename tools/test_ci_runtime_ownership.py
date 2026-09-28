#!/usr/bin/env python3
from __future__ import annotations
import os, pathlib, subprocess, tempfile, unittest
ROOT = pathlib.Path(__file__).resolve().parents[1]
HELPER = ROOT / "tools" / "ci-runtime-ownership.sh"

class RuntimeOwnershipTests(unittest.TestCase):
    def run_bash(self, script: str, *, cwd: pathlib.Path | None = None):
        return subprocess.run(["bash", "-c", f'set -euo pipefail; source "{HELPER}"; {script}'], cwd=cwd or ROOT, text=True, capture_output=True)

    def test_missing_leaf_vespa_digest_fails_closed(self):
        r = self.run_bash('ci_runtime_require_digest "" "leaf"')
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("missing immutable Vespa image digest", r.stderr)

    def test_non_digest_leaf_vespa_identity_fails_closed(self):
        r = self.run_bash('ci_runtime_require_digest "docker.io/x/y:tag" "leaf"')
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("not immutable", r.stderr)

    def test_exact_leaf_digest_is_preserved(self):
        ref = "docker.io/x/y@sha256:" + "a" * 64
        r = self.run_bash(f'ci_runtime_require_digest "{ref}" "leaf"')
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(r.stdout.strip(), ref)

    def test_download_volume_is_shared_by_environment_not_worktree(self):
        a = self.run_bash('ci_runtime_shared_download_volume cargo-registry env-a')
        b = self.run_bash('ci_runtime_shared_download_volume cargo-registry env-a', cwd=ROOT / "tools")
        other = self.run_bash('ci_runtime_shared_download_volume cargo-registry env-b')
        self.assertEqual(a.returncode, 0, a.stderr)
        self.assertEqual(a.stdout, b.stdout)
        self.assertNotEqual(a.stdout, other.stdout)

    def test_target_volume_is_stable_per_worktree_and_environment(self):
        with tempfile.TemporaryDirectory() as raw:
            repo = pathlib.Path(raw) / "repo"
            wt = pathlib.Path(raw) / "wt"
            subprocess.run(["git", "init", "-q", str(repo)], check=True)
            subprocess.run(["git", "-C", str(repo), "config", "user.name", "t"], check=True)
            subprocess.run(["git", "-C", str(repo), "config", "user.email", "t@invalid"], check=True)
            (repo / "x").write_text("x")
            subprocess.run(["git", "-C", str(repo), "add", "x"], check=True)
            subprocess.run(["git", "-C", str(repo), "commit", "-qm", "x"], check=True)
            subprocess.run(["git", "-C", str(repo), "worktree", "add", "-qb", "other", str(wt)], check=True)
            a1 = self.run_bash('ci_runtime_target_volume "$PWD" env-a', cwd=repo)
            a2 = self.run_bash('ci_runtime_target_volume "$PWD" env-a', cwd=repo)
            b = self.run_bash('ci_runtime_target_volume "$PWD" env-b', cwd=repo)
            other = self.run_bash('ci_runtime_target_volume "$PWD" env-a', cwd=wt)
            self.assertEqual(a1.returncode, 0, a1.stderr)
            self.assertEqual(a1.stdout, a2.stdout)
            self.assertNotEqual(a1.stdout, b.stdout)
            self.assertNotEqual(a1.stdout, other.stdout)

if __name__ == "__main__": unittest.main()
