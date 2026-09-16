#!/usr/bin/env python3
"""Synthetic, isolated acceptance tests for the frontend harness itself."""

from __future__ import annotations

import importlib.util
import json
import os
import stat
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


sys.dont_write_bytecode = True
HARNESS = Path(__file__).resolve().parents[1]


def write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def git(root: Path, *args: str, check: bool = True) -> str:
    result = subprocess.run(["git", *args], cwd=root, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if check and result.returncode:
        raise AssertionError(result.stderr)
    return result.stdout.strip()


def command(root: Path, *args: str, env_extra: dict[str, str] | None = None) -> tuple[int, str]:
    env = os.environ.copy()
    env.update({"FRONTEND_REPO_ROOT": str(root), "FRONTEND_REVIEW_DIR": str(root / "target" / "reviews")})
    if env_extra:
        env.update(env_extra)
    script = root / "tools/frontend-harness/lib/harness.py"
    result = subprocess.run([sys.executable, str(script), *args], cwd=root, env=env, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    return result.returncode, result.stdout


def expect(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def prepare(root: Path) -> str:
    shutil.copytree(HARNESS, root / "tools" / "frontend-harness")
    write(root / ".gitignore", "target/\n")
    write(root / "searchkernel-web/src/features/search.ts", 'import { SearchRequest } from "clients/typescript";\nexport const gate = capabilities.search.basic;\n')
    write(root / "searchkernel-web/src/admin-client/diagnostics.ts", "export async function diagnostics() { return fetch('/admin/diagnostics'); }\n")
    write(root / "clients/typescript/index.ts", "export interface SearchRequest {}\n")
    write(root / "searchkernel-web/package.json", '{"scripts":{"test":"true"}}\n')
    write(root / "searchkerneld/src/main.rs", "fn main() {}\n")
    write(root / "core/crates/searchkernel-ir/Cargo.toml", "[package]\nname = \"searchkernel-ir\"\n[dependencies]\n")
    write(root / "core/crates/searchkernel-vespa-engine/Cargo.toml", "[package]\nname = \"searchkernel-vespa-engine\"\n[dependencies]\nsearchkernel-vespa-native = \"*\"\n")
    write(root / "core/crates/searchkernel-ir-adapter/Cargo.toml", "[package]\nname = \"searchkernel-ir-adapter\"\n[dependencies]\nsearchkernel-ir = \"*\"\nsearchkernel-vespa-engine = \"*\"\n")
    manifest_path = root / "tools/frontend-harness/verifiers/F0/manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    manifest["implementation"] = "IMPLEMENTED"
    manifest["required_paths"] = ["searchkernel-web", "clients/typescript"]
    manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    write(root / "tools/frontend-harness/verifiers/F0/verify.sh", "#!/usr/bin/env bash\nprintf '%s\\n' '{\"milestone\":\"F0\",\"status\":\"PASS\",\"tests\":[\"smoke\"]}'\n")
    write(root / "tools/frontend-harness/verifiers/F0/expected/golden.json", '{"fixture":"stable"}\n')
    git(root, "init", "-q")
    git(root, "config", "user.email", "selftest@example.invalid")
    git(root, "config", "user.name", "frontend-harness-selftest")
    git(root, "add", ".")
    git(root, "commit", "-qm", "synthetic verifier baseline")
    baseline = git(root, "rev-parse", "HEAD")
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    manifest["freeze_commit"] = baseline
    manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    state_path = root / "tools/frontend-harness/state.json"
    state = json.loads(state_path.read_text(encoding="utf-8"))
    state["milestones"]["F0"]["status"] = "candidate"
    state_path.write_text(json.dumps(state, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    git(root, "add", "tools/frontend-harness/verifiers/F0/manifest.json", "tools/frontend-harness/state.json")
    git(root, "commit", "-qm", "record synthetic verifier freeze")
    candidate = git(root, "rev-parse", "HEAD")
    install_reviews(root, candidate)
    return candidate


def install_reviews(root: Path, candidate: str, status: str = "PASS", warning: bool = False) -> None:
    review_root = root / "target/reviews/F0"
    for reviewer in ("architecture", "correctness", "security"):
        value = {
            "milestone": "F0",
            "candidate_commit": candidate,
            "reviewer": reviewer,
            "status": status,
            "gates": {"review": "checked"},
            "blocking_findings": [],
            "warnings": ([{"message": "non-blocking concern"}] if warning else []),
        }
        write(review_root / f"{reviewer}.json", json.dumps(value) + "\n")


def clone(source: Path, destination: Path) -> None:
    subprocess.run(["git", "clone", "-q", "--no-local", str(source), str(destination)], check=True)
    git(destination, "config", "user.email", "selftest@example.invalid")
    git(destination, "config", "user.name", "frontend-harness-selftest")


def arch_case(root: Path, relative: str, content: str, should_pass: bool, label: str) -> None:
    path = root / relative
    write(path, content)
    code, output = command(root, "architecture")
    path.unlink()
    expect((code == 0) == should_pass, f"{label}: {output}")


def run() -> None:
    with tempfile.TemporaryDirectory(prefix="frontend-harness-selftest-") as directory:
        root = Path(directory) / "candidate"
        root.mkdir(parents=True)
        candidate = prepare(root)

        valid = Path(directory) / "valid"
        clone(root, valid)
        install_reviews(valid, candidate)
        code, output = command(valid, "accept", "F0")
        expect(code == 0, f"valid candidate did not pass: {output}")
        state = json.loads((valid / "tools/frontend-harness/state.json").read_text(encoding="utf-8"))
        expect(state["milestones"]["F0"]["status"] == "passed", "accept did not set passed")

        tampered = Path(directory) / "tampered-verifier"
        clone(root, tampered)
        write(tampered / "tools/frontend-harness/verifiers/F0/verify.sh", "#!/usr/bin/env bash\nexit 0\n")
        git(tampered, "add", "tools/frontend-harness/verifiers/F0/verify.sh")
        git(tampered, "commit", "-qm", "tamper verifier")
        code, _ = command(tampered, "integrity", "F0")
        expect(code != 0, "verifier modification was accepted")

        tampered = Path(directory) / "tampered-golden"
        clone(root, tampered)
        (tampered / "tools/frontend-harness/verifiers/F0/expected/golden.json").unlink()
        git(tampered, "add", "-u")
        git(tampered, "commit", "-qm", "delete golden")
        code, _ = command(tampered, "integrity", "F0")
        expect(code != 0, "golden deletion was accepted")

        skipped = Path(directory) / "skipped.sh"
        write(skipped, "#!/usr/bin/env bash\nprintf '%s\\n' '{\"status\":\"PASS\",\"tests\":[]}'\n")
        skipped.chmod(skipped.stat().st_mode | stat.S_IXUSR)
        spec = importlib.util.spec_from_file_location("frontend_harness", HARNESS / "lib/harness.py")
        module = importlib.util.module_from_spec(spec)
        assert spec and spec.loader
        spec.loader.exec_module(module)
        os.environ["FRONTEND_REPO_ROOT"] = str(root)
        os.environ["FRONTEND_HARNESS_ROOT"] = str(root / "tools/frontend-harness")
        result = module.run_verifier({"verifier": [str(skipped)], "implementation": "IMPLEMENTED", "required_tests": ["smoke"]})
        expect(result["status"] == "FAIL", "skipped verifier test was accepted")
        os.environ.pop("FRONTEND_HARNESS_ROOT", None)
        os.environ.pop("FRONTEND_REPO_ROOT", None)

        arch_case(root, "searchkernel-web/src/features/bad-fetch.ts", "export const x = fetch('/api/search');\n", False, "portable fetch")
        arch_case(root, "searchkernel-web/src/admin-client/ok.ts", "export const x = fetch('/admin/diagnostics');\n", True, "admin client")
        arch_case(root, "searchkernel-web/src/features/duplicate.ts", "export interface SearchRequest {}\n", False, "duplicate portable type")
        arch_case(root, "searchkernel-web/src/features/core.ts", "import Engine from '../../../core/crates/searchkernel';\n", False, "core import")
        arch_case(root, "searchkernel-web/src/features/raw.ts", "import { Builder } from 'flatbuffers';\n", False, "raw FlatBuffers")
        arch_case(root, "searchkernel-web/src/features/version.ts", "if (server.version >= '1.0') showFeature();\n", False, "version gating")
        arch_case(root, "searchkernel-web/src/features/capability.ts", "if (capabilities.ranking.features) showFeature();\n", True, "capability gating")

        code, _ = command(root, "milestone", "begin", "F1")
        expect(code != 0, "unsatisfied dependency was accepted")
        code, _ = command(root, "accept", "F1")
        expect(code != 0, "NOT_IMPLEMENTED milestone was accepted")

        dirty = Path(directory) / "dirty"
        clone(root, dirty)
        write(dirty / "local-untracked.txt", "local work\n")
        code, _ = command(dirty, "accept", "F0")
        expect(code != 0, "dirty candidate was accepted")

        write(root / "local-untracked.txt", "ignored by clean candidate mode\n")
        code, output = command(root, "verify", "F0", "--clean-worktree")
        expect(code == 0, f"clean worktree did not ignore untracked file: {output}")
        (root / "local-untracked.txt").unlink()

        install_reviews(root, candidate)
        write(root / "target/reviews/F0/security.json", "not-json\n")
        code, _ = command(root, "verify", "F0")
        expect(code != 0, "malformed review JSON was accepted")
        state_path = root / "tools/frontend-harness/state.json"
        state = json.loads(state_path.read_text(encoding="utf-8"))
        state["milestones"]["F0"]["status"] = "candidate"
        state_path.write_text(json.dumps(state, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        install_reviews(root, candidate, status="FAIL")
        code, _ = command(root, "verify", "F0")
        expect(code != 0, "review blocker was accepted")
        state["milestones"]["F0"]["status"] = "candidate"
        state_path.write_text(json.dumps(state, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        install_reviews(root, candidate, warning=True)
        code, _ = command(root, "verify", "F0")
        expect(code == 0, "warning-only review failed acceptance")

        state["milestones"]["F0"]["status"] = "planned"
        state_path.write_text(json.dumps(state, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        code, _ = command(root, "milestone", "candidate", "F0")
        expect(code != 0, "illegal planned-to-candidate transition was accepted")

        def dependency_case(relative: str, content: str, label: str, expected: bool) -> None:
            path = root / relative
            write(path, content)
            code, output = command(root, "architecture")
            path.unlink()
            expect((code == 0) == expected, f"{label}: {output}")

        dependency_case("core/crates/searchkernel-ir/Cargo.toml", "[package]\nname='searchkernel-ir'\n[dependencies]\nsearchkernel-vespa-engine='*'\n", "IR to Vespa", False)
        dependency_case("core/crates/searchkernel-vespa-engine/Cargo.toml", "[package]\nname='searchkernel-vespa-engine'\n[dependencies]\nsearchkernel-ir='*'\n", "Vespa to IR", False)
        dependency_case("core/crates/searchkernel-ir-adapter/Cargo.toml", "[package]\nname='searchkernel-ir-adapter'\n[dependencies]\nsearchkernel-ir='*'\nsearchkernel-vespa-engine='*'\n", "adapter bridge", True)

    print("deliberate verifier-integrity rejection PASS")
    print("deliberate architecture violation rejection PASS")
    print("frontend harness self-test PASS (22 cases)")


if __name__ == "__main__":
    try:
        run()
    except (AssertionError, OSError, subprocess.SubprocessError) as exc:
        print(f"frontend harness self-test FAIL: {exc}", file=sys.stderr)
        raise SystemExit(1)
