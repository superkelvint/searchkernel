#!/usr/bin/env python3
import importlib.util
import json
import subprocess
import tempfile
import unittest
from pathlib import Path

P = Path(__file__).with_name("trusted_pr_authority.py")
S = importlib.util.spec_from_file_location("trusted_pr_authority", P)
M = importlib.util.module_from_spec(S)
S.loader.exec_module(M)


class TrustedAuthorityTests(unittest.TestCase):
    def test_candidate_cannot_redefine_authorization_executor(self):
        for path in (
            ".github/workflows/behavioral-verification.yml",
            ".github/workflows/ci-image-runner.yml",
            ".github/ci/exact_head.py",
            "verification/dag.toml",
            "dev",
            "reference/vespa/m14/verify-m14.sh",
            "tools/check-layout.sh",
            ".moon/tasks.yml",
        ):
            with self.subTest(path=path):
                self.assertTrue(M.protected(path))

    def test_product_code_is_not_authority(self):
        for path in (
            "core/crates/searchkernel/src/lib.rs",
            "clients/python/searchkernel/client.py",
            "searchkerneld/src/main.rs",
        ):
            with self.subTest(path=path):
                self.assertFalse(M.protected(path))

    def _repo(self):
        tmp = tempfile.TemporaryDirectory()
        root = Path(tmp.name)
        subprocess.run(["git", "init", "-q", str(root)], check=True)
        subprocess.run(["git", "-C", str(root), "config", "user.email", "ci@example.invalid"], check=True)
        subprocess.run(["git", "-C", str(root), "config", "user.name", "CI"], check=True)
        return tmp, root

    def test_real_noop_authority_attacks_require_transition(self):
        tmp, root = self._repo()
        self.addCleanup(tmp.cleanup)
        fixtures = {
            ".github/workflows/behavioral-verification.yml": "jobs:\n  gate:\n    steps:\n      - run: ./dev verify integration\n",
            ".moon/tasks.yml": "tasks:\n  vespa-m14:\n    command: ./dev verify m14\n",
            "dev": "#!/usr/bin/env bash\nexec true\n",
            "reference/vespa/m14/verify-m14.sh": "#!/usr/bin/env bash\nexit 1\n",
        }
        for path, content in fixtures.items():
            target = root / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(content, encoding="utf-8")
        subprocess.run(["git", "-C", str(root), "add", "."], check=True)
        subprocess.run(["git", "-C", str(root), "commit", "-qm", "base"], check=True)
        base = subprocess.check_output(["git", "-C", str(root), "rev-parse", "HEAD"], text=True).strip()

        for path in fixtures:
            (root / path).write_text("true\n", encoding="utf-8")
        subprocess.run(["git", "-C", str(root), "add", "."], check=True)
        subprocess.run(["git", "-C", str(root), "commit", "-qm", "attack"], check=True)
        head = subprocess.check_output(["git", "-C", str(root), "rev-parse", "HEAD"], text=True).strip()
        changed = subprocess.check_output(
            ["git", "-C", str(root), "diff", "--name-only", "--no-renames", base, head],
            text=True,
        ).splitlines()

        direct, semantic, transition_only = M.classify(changed, base=base, head=head, repo=root)
        self.assertEqual(sorted(fixtures), direct)
        self.assertEqual([], semantic)
        self.assertTrue(transition_only)

    def test_typescript_script_noop_is_semantic_authority_but_dependency_change_is_not(self):
        tmp, root = self._repo()
        self.addCleanup(tmp.cleanup)
        package = root / "clients/typescript/package.json"
        package.parent.mkdir(parents=True, exist_ok=True)
        package.write_text(
            json.dumps({"scripts": {"test": "vitest run"}, "dependencies": {"a": "1"}}),
            encoding="utf-8",
        )
        subprocess.run(["git", "-C", str(root), "add", "."], check=True)
        subprocess.run(["git", "-C", str(root), "commit", "-qm", "base"], check=True)
        base = subprocess.check_output(["git", "-C", str(root), "rev-parse", "HEAD"], text=True).strip()

        package.write_text(
            json.dumps({"scripts": {"test": "true"}, "dependencies": {"a": "2"}}),
            encoding="utf-8",
        )
        subprocess.run(["git", "-C", str(root), "add", "."], check=True)
        subprocess.run(["git", "-C", str(root), "commit", "-qm", "attack"], check=True)
        head = subprocess.check_output(["git", "-C", str(root), "rev-parse", "HEAD"], text=True).strip()
        direct, semantic, transition_only = M.classify(
            ["clients/typescript/package.json"], base=base, head=head, repo=root
        )
        self.assertEqual([], direct)
        self.assertEqual(["clients/typescript/package.json#scripts"], semantic)
        self.assertTrue(transition_only)

        package.write_text(
            json.dumps({"scripts": {"test": "true"}, "dependencies": {"a": "3"}}),
            encoding="utf-8",
        )
        subprocess.run(["git", "-C", str(root), "add", "."], check=True)
        subprocess.run(["git", "-C", str(root), "commit", "-qm", "dependency"], check=True)
        dependency_head = subprocess.check_output(
            ["git", "-C", str(root), "rev-parse", "HEAD"], text=True
        ).strip()
        direct, semantic, _ = M.classify(
            ["clients/typescript/package.json"], base=head, head=dependency_head, repo=root
        )
        self.assertEqual([], direct)
        self.assertEqual([], semantic)

    def test_transition_rejects_mixed_product_and_authority_change(self):
        direct, semantic, transition_only = M.classify(
            [".github/workflows/ci.yml", "core/crates/searchkernel/src/lib.rs"]
        )
        self.assertEqual([".github/workflows/ci.yml"], direct)
        self.assertEqual([], semantic)
        self.assertFalse(transition_only)


if __name__ == "__main__":
    unittest.main()
