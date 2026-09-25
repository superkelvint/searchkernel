#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
DEV="$ROOT/dev"
RUNNER="$ROOT/tools/run-in-ci-image.sh"
HOOK="$ROOT/.githooks/pre-commit"
COVERAGE="$ROOT/tools/coverage.sh"
CI_WORKFLOW="$ROOT/.github/workflows/ci-image-runner.yml"

bash -n "$DEV"
bash -n "$RUNNER"
bash -n "$HOOK"
bash -n "$COVERAGE"
python3 -m py_compile "$ROOT/.github/ci/test_exact_head.py"

python3 - "$DEV" "$RUNNER" "$HOOK" "$ROOT/.github/ci/test_exact_head.py" "$CI_WORKFLOW" <<'PY'
from pathlib import Path
import sys

dev = Path(sys.argv[1]).read_text(encoding="utf-8")
runner = Path(sys.argv[2]).read_text(encoding="utf-8")
hook = Path(sys.argv[3]).read_text(encoding="utf-8")
exact_head = Path(sys.argv[4]).read_text(encoding="utf-8")
ci_workflow = Path(sys.argv[5]).read_text(encoding="utf-8")

if exact_head.count('if __name__ == "__main__":') != 1:
    raise SystemExit("exact-head regression module must contain exactly one module terminator")

dispatch = 'exec bash "$ROOT/tools/run-in-ci-image.sh" "$@"'
resolve_python = 'PYTHON=$(bash "$ROOT/tools/resolve-python.sh")'
if dispatch not in dev:
    raise SystemExit("dev does not dispatch build/test/verify commands through the CI image")
if dev.index(dispatch) > dev.index(resolve_python):
    raise SystemExit("dev must dispatch before resolving any host language toolchain")
if "Configured repository-managed Git hooks at .githooks' >&2" not in dev:
    raise SystemExit("dev hook auto-configuration must not contaminate machine-readable stdout")

for fragment in (
    "nightly-coverage-evidence",
    '"coverage.json"',
    '"summary.json"',
    '"lcov.info"',
    '"summary.txt"',
    '"README.txt"',
    '"metadata.json"',
    '"html/index.html"',
    'metadata.get("git_head", "").lower() != expected_head.lower()',
    'metadata.get("working_tree_clean") is not True',
    'metadata.get("tool") != "cargo-llvm-cov 0.9.1"',
    "if-no-files-found: warn",
):
    if fragment not in ci_workflow:
        raise SystemExit(f"CI coverage evidence contract is missing {fragment!r}")

for command in (
    "check|universe|check-fast|check-protocol|coverage|lint|test|verify|precommit-verify",
    "SEARCHKERNEL_DEV_USE_CI_IMAGE",
    "SEARCHKERNEL_IN_CI_IMAGE",
    "/opt/searchkernel/native-validation-pass",
):
    if command not in dev:
        raise SystemExit(f"dev image-dispatch contract is missing {command!r}")

coverage_start = dev.index("coverage_target() {")
coverage_end = dev.index("\nlint_target()", coverage_start)
coverage_body = dev[coverage_start:coverage_end]
if "native_sdk_command ensure >/dev/null" not in coverage_body:
    raise SystemExit("dev coverage must provision the native SDK before all-features tests")
if coverage_body.index("native_sdk_command ensure >/dev/null") > coverage_body.index('bash "$ROOT/tools/coverage.sh"'):
    raise SystemExit("dev coverage provisions the native SDK too late")

required_runner = (
    "docker.io/superkelvint/searchkernel-ci:latest",
    'IMAGE_ID=$(docker image inspect --format',
    'source_volume="searchkernel-dev-src-',
    'target_volume="searchkernel-dev-target-',
    'git -C "$ROOT" bundle create "$bundle" HEAD',
    'git -C /workspace config core.hooksPath .githooks',
    'git -C "$ROOT" diff --no-renames --name-only -z HEAD',
    'git -C "$ROOT" ls-files --others --exclude-standard -z',
    '[[ "${1:-}" == "precommit-verify" ]] && PRECOMMIT_SNAPSHOT=1',
    "git add -A && git diff --quiet",
    '-v "$source_volume:/work"',
    '-v "$target_volume:/cargo-target"',
    "-e CARGO_TARGET_DIR=/cargo-target",
    "-e FLATBUFFERS_CPP_ROOT=/usr/local",
    "-e FLATC_PATH=/usr/local/bin/flatc",
    "-e CC=/opt/rh/gcc-toolset-15/root/usr/bin/gcc",
    "-e CXX=/opt/rh/gcc-toolset-15/root/usr/bin/g++",
    "-e CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=/opt/rh/gcc-toolset-15/root/usr/bin/gcc",
    "-e LIBRARY_PATH=/opt/rh/gcc-toolset-15/root/usr/lib64:/opt/rh/gcc-toolset-15/root/usr/lib",
    "-e SEARCHKERNEL_IN_CI_IMAGE=1",
    '[[ "${1:-}" == "coverage" ]] && COVERAGE_SNAPSHOT=1',
    'artifact_volume="searchkernel-dev-artifacts-$(id -u)-$$-${RANDOM}"',
    "-e SEARCHKERNEL_COVERAGE_DIR=/artifacts/coverage",
    'tar -C /artifacts -cf - coverage | tar -C "$ROOT" -xf -',
    'if [[ "$status" -eq 0 ]]; then',
    'coverage failed; no canonical report was exported',
)
for fragment in required_runner:
    if fragment not in runner:
        raise SystemExit(f"CI-image runner is missing {fragment!r}")

for forbidden in (
    "searchkernel-ci-cargo-registry",
    "searchkernel-ci-cargo-git",
    "searchkernel-native-ci-lock",
    ":/root/.cargo/registry",
    ":/root/.cargo/git",
    '-v "$ROOT:',
    "/work/.git/searchkernel/toolchains",
):
    if forbidden in runner:
        raise SystemExit(f"CI-image runner reintroduced shared/host mutable state: {forbidden}")

if 'SEARCHKERNEL_PRECOMMIT_IN_CONTAINER=1' not in dev:
    raise SystemExit("dev precommit verifier does not mark the container phase")
if '"$ROOT/dev" precommit-verify' not in hook:
    raise SystemExit("pre-commit does not delegate its verification body to one dev container")
if '-f "$ROOT/Cargo.toml"' not in hook:
    raise SystemExit("pre-commit fixture/real-checkout boundary is missing")
if "canonical CI-image launcher is missing" not in hook:
    raise SystemExit("pre-commit must fail closed if the real checkout loses its launcher")
if "SEARCHKERNEL_PRECOMMIT_IN_CONTAINER" not in hook:
    raise SystemExit("pre-commit has no container-phase boundary")
if hook.index('"$ROOT/dev" precommit-verify') > hook.index('echo "pre-commit: checking rustfmt"'):
    raise SystemExit("pre-commit must enter the CI image before compiler/test execution")

print("dev/pre-commit CI-image source contract: PASS")
PY

for fragment in \
    '"$CARGO" llvm-cov clean --workspace' \
    '--workspace' \
    '--all-targets' \
    '--all-features' \
    '--no-report' \
    '--summary-only' \
    'metadata.json' \
    '"working_tree_clean": not bool(git_status)' \
    '"native_cpp_coverage": False' \
    'refusing destructive coverage output directory'; do
    grep -F -- "$fragment" "$COVERAGE" >/dev/null || {
        echo "coverage runner is missing contract fragment: $fragment" >&2
        exit 1
    }
done
if grep -F -- 'cargo install' "$COVERAGE" >/dev/null; then
    echo 'coverage endpoint must not install/mutate the immutable CI toolchain' >&2
    exit 1
fi

tmp=$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-dev-image-test.XXXXXX")
trap 'rm -rf -- "$tmp"' EXIT

repo="$tmp/repo"
clone="$tmp/clone"
bundle="$tmp/repository.bundle"
mkdir -p "$repo"
git -C "$repo" init -q
git -C "$repo" config user.name "CI Image Test"
git -C "$repo" config user.email "ci-image@example.invalid"
printf 'tracked\n' > "$repo/tracked.txt"
git -C "$repo" add tracked.txt
git -C "$repo" commit -q -m initial
head=$(git -C "$repo" rev-parse HEAD)

git -C "$repo" bundle create "$bundle" HEAD
git clone -q "$bundle" "$clone"
git -C "$clone" checkout -q --detach "$head"

[[ "$(git -C "$clone" rev-parse HEAD)" == "$head" ]] || {
    echo "git bundle snapshot did not preserve exact HEAD" >&2
    exit 1
}

echo "dev CI-image Git snapshot regression: PASS"
