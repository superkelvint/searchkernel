from __future__ import annotations

import subprocess
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
DEV = (ROOT / "dev").read_text(encoding="utf-8")


def extract_function(name: str, next_name: str) -> str:
    start = DEV.index(f"{name}() {{")
    end = DEV.index(f"\n{next_name}() {{", start)
    return DEV[start:end]


FORMAT_COMMAND = extract_function("universe_format_command", "universe_step")
PRINT_SUMMARY = extract_function("universe_print_summary", "universe")
UNIVERSE = extract_function("universe", "regen_protocol")


class DevUniverseSummaryTests(unittest.TestCase):
    def run_bash(self, body: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["bash", "-c", body],
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
        )

    def test_repro_commands_are_copy_pasteable_from_repo_root(self) -> None:
        result = self.run_bash(
            f"""
set -euo pipefail
ROOT=/work
CARGO=/opt/rust/bin/cargo
PYTHON=/opt/python/bin/python
{FORMAT_COMMAND}
universe_format_command "$ROOT/dev" verify vespa.ranking --skip-workspace-quality
universe_format_command "$CARGO" test --workspace --all-targets --all-features -- --include-ignored
universe_format_command bash "$ROOT/tools/test-native-build-hardening.sh"
"""
        )
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertEqual(
            result.stdout.splitlines(),
            [
                "./dev verify vespa.ranking --skip-workspace-quality",
                "$(./tools/resolve-rust.sh cargo) test --workspace --all-targets --all-features -- --include-ignored",
                "bash ./tools/test-native-build-hardening.sh",
            ],
        )

    def test_failure_summary_prints_repro_below_each_failed_lane(self) -> None:
        result = self.run_bash(
            f"""
set -euo pipefail
{PRINT_SUMMARY}
failed=(
  "semantic verifier vespa.ranking (exit 127)"
  "Rust tests (all targets/features, including ignored) (exit 101)"
)
repro=(
  './dev verify vespa.ranking --skip-workspace-quality'
  '$(./tools/resolve-rust.sh cargo) test --workspace --all-targets --all-features -- --include-ignored'
)
universe_print_summary 44 failed repro
"""
        )
        self.assertEqual(result.returncode, 1, result.stdout)
        self.assertIn("PASS: 44 lane(s)", result.stdout)
        self.assertIn("FAIL: 2 lane(s)", result.stdout)
        self.assertIn(
            "  - semantic verifier vespa.ranking (exit 127)\n"
            "    repro: ./dev verify vespa.ranking --skip-workspace-quality",
            result.stdout,
        )
        self.assertIn(
            "  - Rust tests (all targets/features, including ignored) (exit 101)\n"
            "    repro: $(./tools/resolve-rust.sh cargo) test --workspace --all-targets --all-features -- --include-ignored",
            result.stdout,
        )
        self.assertIn("Search Kernel universe: FAIL", result.stdout)

    def test_universe_discovers_semantic_verifiers_from_the_canonical_list(self) -> None:
        result = self.run_bash(
            f"""
set -euo pipefail
ROOT={str(ROOT)!r}
CARGO=cargo
PYTHON={sys.executable!r}
universe_dev_step() {{ printf 'DEV:%s\\n' "$*"; }}
universe_step() {{ printf 'STEP:%s\\n' "$*"; }}
universe_print_summary() {{ printf 'SUMMARY:%s\\n' "$1"; }}
{UNIVERSE}
universe
"""
        )
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertIn(
            "DEV:semantic verifier vespa.rust-ownership verify vespa.rust-ownership --skip-workspace-quality",
            result.stdout,
        )
        self.assertIn(
            "DEV:cumulative milestone.m14 closure verify milestone.m14 --skip-workspace-quality",
            result.stdout,
        )
        self.assertIn(
            "DEV:semantic verifier product.integration verify product.integration --baseline-already-proved --skip-workspace-quality",
            result.stdout,
        )
        self.assertIn(
            "semantic verifier vespa.nearest-neighbor.hnsw covered by cumulative milestone.m14 closure",
            result.stdout,
        )
        self.assertNotIn(
            "DEV:semantic verifier vespa.nearest-neighbor.hnsw verify",
            result.stdout,
        )
        self.assertLess(
            result.stdout.index("DEV:cumulative milestone.m14 closure"),
            result.stdout.index("DEV:semantic verifier product.integration"),
        )

    def test_product_integration_does_not_skip_a_failed_cumulative_baseline(self) -> None:
        result = self.run_bash(
            f"""
set -euo pipefail
ROOT={str(ROOT)!r}
CARGO=cargo
PYTHON={sys.executable!r}
universe_dev_step() {{
  printf 'DEV:%s\\n' "$*"
  if [[ "$*" == *"milestone.m14"* ]]; then
    UNIVERSE_FAILED+=("M14 failed")
    UNIVERSE_FAILED_REPRO+=("./dev verify milestone.m14")
  fi
}}
universe_step() {{ printf 'STEP:%s\\n' "$*"; }}
universe_print_summary() {{ printf 'SUMMARY:%s\\n' "$1"; }}
{UNIVERSE}
universe
"""
        )
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertIn(
            "DEV:semantic verifier product.integration verify product.integration --skip-workspace-quality",
            result.stdout,
        )
        self.assertNotIn(
            "DEV:semantic verifier product.integration verify product.integration --baseline-already-proved",
            result.stdout,
        )


if __name__ == "__main__":
    unittest.main()
