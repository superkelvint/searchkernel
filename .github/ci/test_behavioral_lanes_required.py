#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

MODULE_PATH = Path(__file__).with_name("behavioral_lanes_required.py")
SPEC = importlib.util.spec_from_file_location("behavioral_lanes_required", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
POLICY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(POLICY)

ROOT = Path(__file__).resolve().parents[2]
BEHAVIORAL_WORKFLOW = ROOT / ".github" / "workflows" / "behavioral-verification.yml"
CI_IMAGE_RUNNER = ROOT / ".github" / "ci" / "ci-image-runner.yml"


class BehavioralLanesRequiredTests(unittest.TestCase):
    def assert_lanes(self, paths: list[str], expected: set[str]) -> None:
        selected, _reasons = POLICY.classify(paths)
        actual = {lane for lane, required in selected.items() if required}
        self.assertEqual(actual, expected)

    def test_client_local_changes_select_only_their_lane(self) -> None:
        cases = {
            "clients/python/searchkernel/client.py": {"python"},
            "clients/typescript/src/client.ts": {"typescript"},
            "clients/ruby/lib/searchkernel.rb": {"ruby"},
        }
        for path, expected in cases.items():
            with self.subTest(path=path):
                self.assert_lanes([path], expected)

    def test_php_change_does_not_select_unrelated_behavioral_lanes(self) -> None:
        self.assert_lanes(["clients/php/src/Client.php"], set())

    def test_rust_local_change_selects_only_rust(self) -> None:
        self.assert_lanes(["core/crates/searchkernel-filter/src/lib.rs"], {"rust"})

    def test_shared_bfbs_artifacts_preserve_cross_language_closure(self) -> None:
        self.assert_lanes(
            ["core/crates/searchkernel-protocol/bfbs/searchkernel.bfbs"],
            {"rust", "python", "typescript"},
        )

    def test_shared_protocol_tooling_preserves_cross_language_closure(self) -> None:
        paths = [
            "api/rpc/searchkernel.proto",
            "tools/generate-flatbuffers-bfbs.sh",
            "tools/check-flatbuffers-bfbs.sh",
            "tools/check-flatbuffers-cpp.sh",
            "tools/resolve-flatc.sh",
            "tools/test-flatbuffers-cpp-toolchain.sh",
        ]
        for path in paths:
            with self.subTest(path=path):
                self.assert_lanes([path], {"rust", "python", "typescript"})

    def test_ruby_json_contract_selects_rust_and_ruby(self) -> None:
        self.assert_lanes(
            ["core/crates/searchkernel-protocol/tests/ruby_json_contract.rs"],
            {"rust", "ruby"},
        )

    def test_multiple_client_changes_union_lanes(self) -> None:
        self.assert_lanes(
            [
                "clients/python/searchkernel/client.py",
                "clients/typescript/src/client.ts",
            ],
            {"python", "typescript"},
        )

    def test_dev_change_does_not_broaden_an_owned_client_change(self) -> None:
        cases = {
            "clients/python/searchkernel/client.py": {"python"},
            "clients/typescript/src/client.ts": {"typescript"},
            "clients/ruby/lib/searchkernel.rb": {"ruby"},
        }
        for path, expected in cases.items():
            with self.subTest(path=path):
                self.assert_lanes([path, "dev"], expected)

    def test_dev_only_change_still_fails_closed(self) -> None:
        self.assert_lanes(["dev"], set(POLICY.LANES))

    def test_ruby_contract_plus_dev_does_not_run_other_clients(self) -> None:
        self.assert_lanes(
            [
                "clients/ruby/lib/searchkernel/value.rb",
                "clients/ruby/test/run.rb",
                "core/crates/searchkernel-protocol/tests/ruby_json_contract.rs",
                "dev",
            ],
            {"rust", "ruby"},
        )

    def test_rust_rpc_codegen_and_generated_artifacts_stay_in_rust_lane(self) -> None:
        paths = [
            "core/crates/searchkernel-rpc/src/generated/rpc.rs",
            "searchkerneld/src/generated/rpc.rs",
            "tools/rpc-codegen/src/main.rs",
            "tools/generate-searchkernel-rpc.sh",
            "tools/check-searchkernel-rpc.sh",
            "tools/generate-searchkernel-protocol.sh",
            "tools/check-searchkernel-protocol-generated.sh",
        ]
        for path in paths:
            with self.subTest(path=path):
                self.assert_lanes([path], {"rust"})

    def test_language_owned_generated_artifacts_stay_in_owner_lane(self) -> None:
        cases = {
            "clients/python/searchkernel/_wire/v1/generated/SearchKernel/Wire/document.py": {"python"},
            "clients/python/searchkernel/rpc/searchkernel_pb2.py": {"python"},
            "clients/typescript/src/_wire/v1/generated/flatbuffers/search-kernel/wire/document.ts": {"typescript"},
            "tools/generate-python-grpc.sh": {"python"},
            "tools/check-python-grpc.sh": {"python"},
            "tools/generate-typescript-grpc-web.sh": {"typescript"},
            "tools/check-typescript-generated.sh": {"typescript"},
        }
        for path, expected in cases.items():
            with self.subTest(path=path):
                self.assert_lanes([path], expected)

    def test_remote_rust_client_regression_does_not_select_other_clients(self) -> None:
        self.assert_lanes(
            [
                "Cargo.lock",
                "Cargo.toml",
                "core/crates/searchkernel-ir/src/api_to_ir/mod.rs",
                "core/crates/searchkernel-ir/src/api_to_ir/result.rs",
                "core/crates/searchkernel-ir/src/ir_to_api/mod.rs",
                "core/crates/searchkernel-ir/src/ir_to_api/transport.rs",
                "core/crates/searchkernel-ir/src/lib.rs",
                "core/crates/searchkernel-rpc/Cargo.toml",
                "core/crates/searchkernel-rpc/src/generated/rpc.rs",
                "core/crates/searchkernel-rpc/src/lib.rs",
                "core/crates/searchkernel/Cargo.toml",
                "core/crates/searchkernel/src/index/mod.rs",
                "core/crates/searchkernel/src/lib.rs",
                "core/crates/searchkernel/src/remote.rs",
                "core/crates/searchkernel/tests/remote_client.rs",
                "searchkerneld/Cargo.toml",
                "searchkerneld/src/lib.rs",
                "searchkerneld/tests/grpc.rs",
                "tools/check-layout.sh",
                "tools/check-searchkernel-rpc.sh",
                "tools/generate-searchkernel-rpc.sh",
            ],
            {"rust"},
        )

    def test_portable_api_change_selects_all_lanes(self) -> None:
        self.assert_lanes(["api/searchkernel_request.fbs"], set(POLICY.LANES))

    def test_portable_api_rust_lane_runs_protocol_freshness(self) -> None:
        self.assert_lanes(["api/searchkernel_request.fbs"], set(POLICY.LANES))

        source = CI_IMAGE_RUNNER.read_text(encoding="utf-8")
        rust_lane = source.split("            rust-non-native)", 1)[1].split(
            "            python)", 1
        )[0]
        self.assertIn(
            "run_phase protocol-bfbs bash tools/check-flatbuffers-bfbs.sh",
            rust_lane,
        )
        self.assertIn(
            "run_phase protocol-rust-generated bash tools/check-searchkernel-protocol-generated.sh",
            rust_lane,
        )

    def test_shared_client_contract_change_selects_all_lanes(self) -> None:
        self.assert_lanes(["tools/check-client-contracts.py"], set(POLICY.LANES))

    def test_ci_policy_change_selects_all_lanes(self) -> None:
        self.assert_lanes(
            [".github/ci/behavioral_lanes_required.py"], set(POLICY.LANES)
        )

    def test_documentation_only_change_selects_no_lanes(self) -> None:
        self.assert_lanes(["docs/architecture.md", "README.md"], set())

    def test_unknown_behavioral_path_fails_closed_to_all_lanes(self) -> None:
        self.assert_lanes(["integrations/new-runtime/bridge.rs"], set(POLICY.LANES))

    def test_behavioral_workflow_uses_current_merge_base_and_trusted_policy(self) -> None:
        source = BEHAVIORAL_WORKFLOW.read_text(encoding="utf-8")
        for needle in (
            'rust: ${{ steps.behavioral.outputs.rust }}',
            'python: ${{ steps.behavioral.outputs.python }}',
            'typescript: ${{ steps.behavioral.outputs.typescript }}',
            'ruby: ${{ steps.behavioral.outputs.ruby }}',
            'PR_NUMBER: ${{ github.event.pull_request.number }}',
            'BASE_REF: ${{ github.event.pull_request.base.ref }}',
            '+refs/pull/$PR_NUMBER/head:refs/searchkernel/ci-head',
            '+refs/heads/$BASE_REF:refs/searchkernel/ci-base',
            'diff_base_sha=$(git merge-base refs/searchkernel/ci-head refs/searchkernel/ci-base || true)',
            'git diff --name-only --no-renames "$diff_base_sha" "$HEAD_SHA"',
            'policy_sha=$(git rev-parse refs/searchkernel/ci-base)',
            'CHANGES_RESOLVED: ${{ steps.changes.outputs.resolved }}',
            'POLICY_SHA: ${{ steps.changes.outputs.policy_sha }}',
            'selector="$RUNNER_TEMP/behavioral_lanes_required.py"',
            'git show "$POLICY_SHA:.github/ci/behavioral_lanes_required.py" > "$selector"',
            'cp .github/ci/behavioral_lanes_required.py "$selector"',
            '"$CI_PYTHON" "$selector" --github-output "$GITHUB_OUTPUT"',
            'echo "rust=true"',
            'echo "python=true"',
            'echo "typescript=true"',
            'echo "ruby=true"',
        ):
            self.assertIn(needle, source)
        self.assertNotIn(
            'git show "$BASE_SHA:.github/ci/behavioral_lanes_required.py"',
            source,
        )

    def test_windows_separators_are_normalized(self) -> None:
        self.assert_lanes([r"clients\python\searchkernel\client.py"], {"python"})


if __name__ == "__main__":
    unittest.main()
