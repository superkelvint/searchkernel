#!/usr/bin/env python3
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github" / "workflows" / "rustfmt-autofix.yml"


def job_block(source: str, name: str) -> str:
    lines = source.splitlines()
    marker = f"  {name}:"
    try:
        start = lines.index(marker)
    except ValueError as exc:
        raise AssertionError(f"missing workflow job {name!r}") from exc

    end = len(lines)
    for index in range(start + 1, len(lines)):
        line = lines[index]
        if line.startswith("  ") and not line.startswith("    ") and line.endswith(":"):
            end = index
            break
    return "\n".join(lines[start:end])


class RustfmtAutofixWorkflowTests(unittest.TestCase):
    def setUp(self) -> None:
        self.source = WORKFLOW.read_text(encoding="utf-8")

    def test_untrusted_formatter_and_credentialed_writer_are_separate_jobs(self) -> None:
        formatter = job_block(self.source, "format_untrusted")
        writer = job_block(self.source, "write_autofix")

        self.assertIn("runs-on: ubuntu-24.04", formatter)
        self.assertNotIn("secrets.", formatter)
        self.assertNotIn("AUTOFIX_TOKEN", formatter)
        self.assertIn("actions/upload-artifact@v4", formatter)
        self.assertIn("rustup which cargo-fmt", formatter)
        self.assertNotIn("cargo fmt --all", formatter)

        self.assertIn("needs: format_untrusted", writer)
        self.assertIn("runs-on: ubuntu-24.04", writer)
        self.assertIn("actions/download-artifact@v4", writer)
        self.assertEqual(writer.count("secrets.RUSTFMT_AUTOFIX_TOKEN"), 1)
        for forbidden in ("cargo fmt", "cargo-fmt", "docker run", "GITHUB_PATH", "GITHUB_ENV"):
            self.assertNotIn(forbidden, writer)

    def test_malicious_cargo_alias_plus_rs_trigger_cannot_reach_write_credentials(self) -> None:
        formatter = job_block(self.source, "format_untrusted")
        writer = job_block(self.source, "write_autofix")

        malicious_config = '[alias]\nfmt = "!sh -c \'echo pwned > /tmp/cargo-fmt-alias-ran\'"\n'
        self.assertIn("[alias]", malicious_config)
        self.assertIn('"**/*.rs"', self.source)

        # A candidate-controlled Cargo alias matters only when Cargo resolves
        # the fmt subcommand. Resolve cargo-fmt through rustup and execute that
        # binary directly in the untrusted job instead.
        self.assertIn("rustup which cargo-fmt", formatter)
        self.assertNotIn("/root/.cargo/bin/cargo fmt", formatter)

        # The write credential exists only in the clean writer job, which never
        # executes candidate-controlled Cargo configuration.
        self.assertNotIn("secrets.RUSTFMT_AUTOFIX_TOKEN", formatter)
        self.assertIn("secrets.RUSTFMT_AUTOFIX_TOKEN", writer)

    def test_writer_accepts_only_an_inert_modified_rust_patch(self) -> None:
        writer = job_block(self.source, "write_autofix")

        for required in (
            "git apply --index --whitespace=error-all",
            "git diff --cached --name-status -z",
            '[[ "$status" == "M" ]]',
            '[[ "$path" == *.rs ]]',
            "git diff --cached --check",
            "-c core.hooksPath=/dev/null",
            "persist-credentials: false",
        ):
            self.assertIn(required, writer)

        validate_at = writer.index("Validate and apply formatter patch")
        push_at = writer.index("Commit and push formatting changes")
        self.assertLess(validate_at, push_at)


if __name__ == "__main__":
    unittest.main()
