#!/usr/bin/env python3
"""Implementation of the Search Kernel frontend acceptance harness.

The command wrappers in this directory intentionally contain no policy.  This
module owns manifest validation, Git-boundary checking, architecture checks,
review ingestion, state transitions, and result formatting.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any, Iterable


STATUSES = {
    "planned",
    "implementing",
    "candidate",
    "verifying",
    "reviewing",
    "fixing",
    "failed",
    "passed",
    "blocked",
}
REVIEW_STATUSES = {"PASS", "WARN", "FAIL"}
MILESTONES = [f"F{i}" for i in range(13)]
PASSABLE_GATE_STATUSES = {"PASS", "NOT_APPLICABLE", "WARN"}
PROTECTED_PREFIXES = ("tools/frontend-harness/verifiers/", "tools/frontend-harness/schemas/", "tools/frontend-harness/reviewers/")
PORTABLE_TYPES = (
    "SearchRequest",
    "SearchResponse",
    "FilterExpr",
    "RankingPolicy",
    "QueryExpr",
    "SearchResult",
    "DocumentRequest",
    "DocumentResponse",
    "FieldCapabilities",
    "CapabilitySet",
)


class HarnessError(RuntimeError):
    pass


def repo_root() -> Path:
    configured = os.environ.get("FRONTEND_REPO_ROOT")
    if configured:
        return Path(configured).resolve()
    return Path(__file__).resolve().parents[3]


def harness_root() -> Path:
    configured = os.environ.get("FRONTEND_HARNESS_ROOT")
    if configured:
        return Path(configured).resolve()
    return Path(__file__).resolve().parents[1]


def state_path() -> Path:
    return harness_root() / "state.json"


def run_git(*args: str, cwd: Path | None = None, check: bool = True) -> str:
    result = subprocess.run(
        ["git", *args], cwd=cwd or repo_root(), text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE
    )
    if check and result.returncode:
        raise HarnessError(result.stderr.strip() or f"git {' '.join(args)} failed")
    return result.stdout.strip()


def git_exists(*args: str, cwd: Path | None = None) -> bool:
    result = subprocess.run(["git", *args], cwd=cwd or repo_root(), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    return result.returncode == 0


def load_json(path: Path) -> Any:
    try:
        with path.open(encoding="utf-8") as stream:
            return json.load(stream)
    except (OSError, json.JSONDecodeError) as exc:
        raise HarnessError(f"malformed JSON: {path}: {exc}") from exc


def write_json_atomic(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.tmp")
    temporary.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    temporary.replace(path)


def manifest_path(milestone: str) -> Path:
    return harness_root() / "verifiers" / milestone / "manifest.json"


def load_manifest(milestone: str) -> dict[str, Any]:
    if milestone not in MILESTONES:
        raise HarnessError(f"unknown milestone: {milestone}; expected F0..F12")
    manifest = load_json(manifest_path(milestone))
    if not isinstance(manifest, dict):
        raise HarnessError(f"manifest is not an object: {manifest_path(milestone)}")
    validate_manifest(manifest, milestone)
    return manifest


def validate_manifest(manifest: dict[str, Any], expected_id: str | None = None) -> None:
    required = ("milestone", "title", "depends_on", "verifier", "required_reviewers", "required_tests", "ci", "freeze_commit", "implementation")
    missing = [key for key in required if key not in manifest]
    if missing:
        raise HarnessError(f"manifest missing required keys: {', '.join(missing)}")
    milestone = manifest["milestone"]
    if expected_id and milestone != expected_id:
        raise HarnessError(f"manifest ID mismatch: expected {expected_id}, got {milestone}")
    if milestone not in MILESTONES:
        raise HarnessError(f"invalid manifest milestone: {milestone}")
    if not isinstance(manifest["title"], str) or not manifest["title"].strip():
        raise HarnessError(f"invalid title in {milestone}")
    dependencies = manifest["depends_on"]
    if not isinstance(dependencies, list) or any(item not in MILESTONES for item in dependencies):
        raise HarnessError(f"invalid dependencies in {milestone}: {dependencies!r}")
    verifier = manifest["verifier"]
    if not isinstance(verifier, list) or not verifier or any(not isinstance(item, str) for item in verifier):
        raise HarnessError(f"verifier must be a non-empty command array in {milestone}")
    reviewers = manifest["required_reviewers"]
    if not isinstance(reviewers, list) or any(item not in {"architecture", "correctness", "security"} for item in reviewers):
        raise HarnessError(f"invalid required reviewers in {milestone}")
    tests = manifest["required_tests"]
    if not isinstance(tests, list) or not tests or any(not isinstance(item, str) or not item for item in tests):
        raise HarnessError(f"invalid required tests in {milestone}")
    ci = manifest["ci"]
    if not isinstance(ci, list):
        raise HarnessError(f"ci must be an array in {milestone}")
    for job in ci:
        if not isinstance(job, dict) or not isinstance(job.get("name"), str) or not isinstance(job.get("command"), list):
            raise HarnessError(f"invalid CI job in {milestone}")
        if any(not isinstance(argument, str) for argument in job["command"]):
            raise HarnessError(f"CI command contains a non-string argument in {milestone}")
        if "paths" in job and (not isinstance(job["paths"], list) or any(not isinstance(path, str) for path in job["paths"])):
            raise HarnessError(f"invalid CI paths in {milestone}")
    if not isinstance(manifest["freeze_commit"], str) or not manifest["freeze_commit"]:
        raise HarnessError(f"invalid freeze_commit in {milestone}")
    if manifest["implementation"] not in {"NOT_IMPLEMENTED", "IMPLEMENTED"}:
        raise HarnessError(f"invalid implementation marker in {milestone}")
    required_paths = manifest.get("required_paths", [])
    if not isinstance(required_paths, list) or any(not isinstance(path, str) for path in required_paths):
        raise HarnessError(f"invalid required_paths in {milestone}")


def load_all_manifests() -> dict[str, dict[str, Any]]:
    manifests = {milestone: load_manifest(milestone) for milestone in MILESTONES}
    visiting: set[str] = set()
    visited: set[str] = set()

    def visit(milestone: str) -> None:
        if milestone in visiting:
            raise HarnessError(f"milestone dependency cycle includes {milestone}")
        if milestone in visited:
            return
        visiting.add(milestone)
        for dependency in manifests[milestone]["depends_on"]:
            visit(dependency)
        visiting.remove(milestone)
        visited.add(milestone)

    for milestone in MILESTONES:
        visit(milestone)
    return manifests


def load_state() -> dict[str, Any]:
    state = load_json(state_path())
    if not isinstance(state, dict) or not isinstance(state.get("milestones"), dict):
        raise HarnessError(f"malformed state file: {state_path()}")
    for milestone in MILESTONES:
        entry = state["milestones"].get(milestone)
        if not isinstance(entry, dict) or entry.get("status") not in STATUSES:
            raise HarnessError(f"invalid state entry for {milestone}")
    return state


def set_state(milestone: str, status: str, *, allow_passed: bool = False) -> None:
    state = load_state()
    old = state["milestones"][milestone]["status"]
    allowed = {
        "planned": {"implementing"},
        "implementing": {"candidate"},
        "candidate": {"verifying", "fixing", "blocked"},
        "verifying": {"reviewing", "failed", "fixing", "blocked"},
        "reviewing": {"passed", "fixing", "failed", "blocked"},
        "fixing": {"candidate", "blocked"},
        "failed": {"implementing", "candidate", "blocked"},
        "blocked": {"implementing", "fixing"},
        "passed": set(),
    }
    if status == "passed" and not allow_passed:
        raise HarnessError("only frontend-accept may set a milestone to passed")
    if status == "passed" and allow_passed:
        if old not in {"candidate", "verifying", "reviewing"}:
            raise HarnessError(f"illegal milestone state transition: {milestone} {old} -> passed")
        state["milestones"][milestone]["status"] = status
        write_json_atomic(state_path(), state)
        return
    if status not in allowed.get(old, set()):
        raise HarnessError(f"illegal milestone state transition: {milestone} {old} -> {status}")
    state["milestones"][milestone]["status"] = status
    write_json_atomic(state_path(), state)


def dependency_gate(milestone: str, manifests: dict[str, dict[str, Any]], state: dict[str, Any]) -> list[str]:
    return [
        dependency
        for dependency in manifests[milestone]["depends_on"]
        if state["milestones"][dependency]["status"] != "passed"
    ]


def git_status_dirty(root: Path) -> bool:
    return bool(run_git("status", "--porcelain", "--untracked-files=all", cwd=root))


def candidate_commit(root: Path | None = None) -> str:
    return run_git("rev-parse", "HEAD", cwd=root or repo_root())


def protected_paths(commit: str | None = None) -> list[str]:
    paths: list[str] = []
    if commit:
        tree = run_git("ls-tree", "-r", "--name-only", commit, cwd=repo_root())
        paths.extend(path for path in tree.splitlines() if path.startswith(PROTECTED_PREFIXES))
    for prefix in PROTECTED_PREFIXES:
        base = repo_root() / prefix
        if base.exists():
            paths.extend(str(path.relative_to(repo_root())) for path in base.rglob("*") if path.is_file())
    return sorted(paths)


def verifier_integrity(milestone: str, candidate: str | None = None) -> dict[str, Any]:
    manifest = load_manifest(milestone)
    root = repo_root()
    freeze = manifest["freeze_commit"]
    if freeze in {"UNSET", "", "HEAD"}:
        raise HarnessError(f"{milestone} has no recorded verifier-freeze commit")
    if not git_exists("cat-file", "-e", f"{freeze}^{{commit}}", cwd=root):
        raise HarnessError(f"recorded verifier-freeze commit does not exist: {freeze}")
    candidate = candidate or os.environ.get("FRONTEND_CANDIDATE_COMMIT") or candidate_commit(root)
    if not git_exists("cat-file", "-e", f"{candidate}^{{commit}}", cwd=root):
        raise HarnessError(f"candidate commit does not exist: {candidate}")
    protected = protected_paths(freeze) + protected_paths(candidate)
    protected = sorted(set(protected))
    if not protected:
        raise HarnessError("no protected verifier material found")
    changed_lines = run_git("diff", "--name-status", f"{freeze}..{candidate}", "--", *protected, cwd=root).splitlines()
    # The freeze commit is recorded in the manifest itself.  Recording it is a
    # one-time metadata update after verifier authoring; compare that field in
    # normalized form so it does not make the freeze commit self-referential.
    normalized_manifest_changes: list[str] = []
    for line in changed_lines:
        fields = line.split(maxsplit=1)
        if len(fields) == 2 and fields[1].endswith("/manifest.json") and fields[0] == "M":
            base_text = run_git("show", f"{freeze}:{fields[1]}", cwd=root, check=False)
            candidate_text = run_git("show", f"{candidate}:{fields[1]}", cwd=root, check=False)
            try:
                base_manifest = json.loads(base_text)
                candidate_manifest = json.loads(candidate_text)
                base_manifest["freeze_commit"] = "<freeze>"
                candidate_manifest["freeze_commit"] = "<freeze>"
                if base_manifest == candidate_manifest:
                    continue
            except json.JSONDecodeError:
                pass
        normalized_manifest_changes.append(line)
    changed = normalized_manifest_changes
    local_changed = run_git("diff", "--name-status", "--", *protected, cwd=root).splitlines()
    untracked = []
    for line in run_git("status", "--porcelain", "--untracked-files=all", cwd=root).splitlines():
        path = line[3:] if len(line) >= 4 else line
        if path.startswith(PROTECTED_PREFIXES):
            untracked.append(path)
    if changed or local_changed or untracked:
        details = changed + [f"worktree {line}" for line in local_changed] + [f"?? {path}" for path in untracked]
        raise HarnessError("protected verifier material changed:\n" + "\n".join(details))
    return {"status": "PASS", "candidate_commit": candidate, "freeze_commit": freeze}


def source_files(root: Path, suffixes: Iterable[str]) -> Iterable[Path]:
    if not root.exists():
        return []
    return (
        path
        for path in root.rglob("*")
        if path.is_file() and path.suffix in suffixes and not {"node_modules", "dist", "target", ".git"}.intersection(path.parts)
    )


def architecture_check() -> dict[str, Any]:
    root = repo_root()
    findings: list[dict[str, str]] = []
    applicable = False

    web = root / "searchkernel-web"
    web_files = list(source_files(web, {".ts", ".tsx", ".js", ".jsx"}))
    if web_files:
        applicable = True
    feature_files = [path for path in web_files if "admin-client" not in path.parts]
    checks = [
        (re.compile(r"\bfetch\s*\(|\baxios\s*(?:\.|\()", re.I), "direct portable network call in dashboard feature code", feature_files),
        (re.compile(r"(?:from|import)\s+[^;]*(?:['\"]).*(?:^|[/])core[/]|['\"](?:\.\./)+core[/]", re.I), "dashboard imports internal core code", feature_files),
        (re.compile(r"(?:from|import)\s+['\"](?:flatbuffers|.*flatbuffers.*|.*_generated|searchkernel_protocol)", re.I), "raw FlatBuffers binding imported by dashboard feature code", feature_files),
    ]
    declarations = re.compile(r"\b(?:interface|type|class|enum)\s+(" + "|".join(PORTABLE_TYPES) + r")\b")
    version_gate = re.compile(r"(?:version\s*(?:>=|<=|===|==|>|<)|(?:semver|versionCompare|compareVersions)\s*\(|parseInt\s*\([^)]*version)", re.I)
    for pattern, message, files in checks:
        for path in files:
            for line_number, line in enumerate(path.read_text(encoding="utf-8", errors="replace").splitlines(), 1):
                if pattern.search(line):
                    findings.append({"code": "WEB_BOUNDARY", "message": message, "path": str(path.relative_to(root)), "line": str(line_number)})
    for path in feature_files:
        text = path.read_text(encoding="utf-8", errors="replace")
        for match in declarations.finditer(text):
            line_number = text.count("\n", 0, match.start()) + 1
            findings.append({"code": "DUPLICATE_PORTABLE_TYPE", "message": f"dashboard redeclares {match.group(1)}", "path": str(path.relative_to(root)), "line": str(line_number)})
        for line_number, line in enumerate(text.splitlines(), 1):
            if version_gate.search(line):
                findings.append({"code": "VERSION_GATING", "message": "feature availability is gated by version comparison", "path": str(path.relative_to(root)), "line": str(line_number)})

    cargo_files = list((root / "core" / "crates").rglob("Cargo.toml")) if (root / "core" / "crates").exists() else []
    ir_crate = re.compile(r"\bsearchkernel-ir\b")
    vespa_crate = re.compile(r"(?:searchkernel-vespa|vespa[-_]|vespa[\"' ])", re.I)
    crate_info: list[tuple[Path, bool, bool]] = []
    for manifest in cargo_files:
        text = manifest.read_text(encoding="utf-8", errors="replace")
        has_ir = bool(ir_crate.search(text))
        has_vespa = bool(vespa_crate.search(text))
        crate_info.append((manifest, has_ir, has_vespa))
        relative = manifest.relative_to(root)
        name = manifest.parent.name
        if name == "searchkernel-ir" and has_vespa:
            findings.append({"code": "IR_VESPA_DEPENDENCY", "message": "searchkernel-ir depends on Vespa", "path": str(relative), "line": "1"})
        if name in {"searchkernel-vespa-engine", "searchkernel-vespa-native", "searchkernel-vespa-bindings"} and has_ir:
            findings.append({"code": "VESPA_IR_DEPENDENCY", "message": f"{name} depends on searchkernel-ir", "path": str(relative), "line": "1"})
        if name != "searchkernel-ir-adapter" and has_ir and has_vespa:
            findings.append({"code": "BRIDGE_OUTSIDE_ADAPTER", "message": "crate depends on both IR and Vespa outside searchkernel-ir-adapter", "path": str(relative), "line": "1"})
        if has_ir or has_vespa:
            applicable = True

    status = "PASS" if not findings else "FAIL"
    if not applicable:
        status = "NOT_APPLICABLE"
    return {"status": status, "findings": findings}


def run_command(command: list[str], cwd: Path, env: dict[str, str] | None = None) -> tuple[int, str]:
    result = subprocess.run(command, cwd=cwd, env=env, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    return result.returncode, result.stdout.strip()


def run_verifier(manifest: dict[str, Any]) -> dict[str, Any]:
    command = [str((harness_root() / argument).resolve()) if argument.startswith("verifiers/") else argument for argument in manifest["verifier"]]
    code, output = run_command(command, repo_root(), os.environ.copy())
    parsed: dict[str, Any] | None = None
    for line in reversed(output.splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict) and "status" in value:
            parsed = value
            break
    if parsed is None:
        if manifest["implementation"] == "NOT_IMPLEMENTED":
            return {"status": "NOT_IMPLEMENTED", "message": output or "unimplemented verifier"}
        return {"status": "FAIL", "message": output or f"verifier exited {code}"}
    if manifest["implementation"] == "NOT_IMPLEMENTED" and parsed.get("status") != "NOT_IMPLEMENTED":
        return {"status": "FAIL", "message": "manifest marks verifier unimplemented but verifier did not return NOT_IMPLEMENTED"}
    if parsed.get("status") == "PASS":
        tests = parsed.get("tests")
        if not isinstance(tests, list) or not tests or any(test not in tests for test in manifest["required_tests"]):
            return {"status": "FAIL", "message": "verifier reported PASS without executing all required tests"}
    if code and parsed.get("status") == "PASS":
        return {"status": "FAIL", "message": f"verifier returned PASS with exit code {code}", "verifier": parsed}
    return parsed


def run_ci(manifest: dict[str, Any]) -> dict[str, Any]:
    gates: dict[str, str] = {}
    failures: list[dict[str, str]] = []
    for job in manifest["ci"]:
        paths = job.get("paths", [])
        if paths and not any((repo_root() / path).exists() for path in paths):
            gates[job["name"]] = "NOT_APPLICABLE"
            continue
        cwd = repo_root() / job.get("cwd", ".")
        code, output = run_command(job["command"], cwd, os.environ.copy())
        gates[job["name"]] = "PASS" if code == 0 else "FAIL"
        if code:
            failures.append({"name": job["name"], "output": output})
    return {"status": "FAIL" if failures else ("PASS" if gates else "NOT_APPLICABLE"), "gates": gates, "failures": failures}


def validate_review(value: Any, milestone: str, candidate: str, path: Path) -> None:
    if not isinstance(value, dict):
        raise HarnessError(f"review result is not an object: {path}")
    required = {"milestone", "candidate_commit", "reviewer", "status", "gates", "blocking_findings", "warnings"}
    if not required.issubset(value):
        raise HarnessError(f"review result missing keys: {path}")
    if value["milestone"] != milestone or value["candidate_commit"] != candidate:
        raise HarnessError(f"review result does not match candidate: {path}")
    if value["reviewer"] not in {"architecture", "correctness", "security"} or value["status"] not in REVIEW_STATUSES:
        raise HarnessError(f"invalid reviewer/status in {path}")
    if not isinstance(value["gates"], dict) or not isinstance(value["blocking_findings"], list) or not isinstance(value["warnings"], list):
        raise HarnessError(f"invalid result arrays/object in {path}")


def ingest_reviews(manifest: dict[str, Any], milestone: str, candidate: str) -> dict[str, Any]:
    review_root = Path(os.environ.get("FRONTEND_REVIEW_DIR", str(repo_root() / "target" / "frontend-harness" / "reviews"))) / milestone
    findings: list[Any] = []
    warnings: list[Any] = []
    statuses: dict[str, str] = {}
    for reviewer in manifest["required_reviewers"]:
        path = review_root / f"{reviewer}.json"
        if not path.exists():
            findings.append({"code": "MISSING_REVIEW", "reviewer": reviewer, "message": f"missing required review: {path}"})
            continue
        value = load_json(path)
        validate_review(value, milestone, candidate, path)
        statuses[reviewer] = value["status"]
        findings.extend(value["blocking_findings"])
        warnings.extend(value["warnings"])
        if value["status"] == "FAIL":
            findings.append({"code": "REVIEW_BLOCKER", "reviewer": reviewer, "message": "reviewer reported FAIL"})
    return {"status": "FAIL" if findings else ("WARN" if warnings else "PASS"), "reviewers": statuses, "blocking_findings": findings, "warnings": warnings}


def result_is_passable(result: dict[str, Any]) -> bool:
    if result.get("status") not in {"PASS", "WARN"}:
        return False
    return not result.get("blocking_findings")


def verify(milestone: str, clean: bool = False, child: bool = False) -> int:
    if clean and not child:
        root = repo_root()
        candidate = candidate_commit(root)
        temporary = Path(tempfile.mkdtemp(prefix="frontend-candidate-"))
        try:
            run_git("worktree", "add", "--detach", str(temporary), candidate, cwd=root)
            environment = os.environ.copy()
            environment.pop("FRONTEND_HARNESS_ROOT", None)
            environment["FRONTEND_REPO_ROOT"] = str(temporary)
            environment["FRONTEND_HARNESS_CLEAN_CHILD"] = "1"
            environment.setdefault("FRONTEND_REVIEW_DIR", str(root / "target" / "frontend-harness" / "reviews"))
            command = [sys.executable, str(temporary / "tools/frontend-harness/lib/harness.py"), "verify", milestone]
            result = subprocess.run(command, cwd=temporary, env=environment, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
            print(result.stdout, end="")
            return result.returncode
        finally:
            run_git("worktree", "remove", "--force", str(temporary), cwd=root, check=False)
            shutil.rmtree(temporary, ignore_errors=True)

    manifests = load_all_manifests()
    manifest = manifests[milestone]
    state = load_state()
    status = state["milestones"][milestone]["status"]
    if status not in {"candidate", "verifying", "reviewing", "fixing"}:
        raise HarnessError(f"{milestone} must be candidate/reviewing before verification; found {status}")
    if status in {"candidate", "fixing"}:
        set_state(milestone, "verifying")
        status = "verifying"
    candidate = candidate_commit()
    gates: dict[str, Any] = {}
    blocking: list[Any] = []
    warnings: list[Any] = []
    try:
        dependencies = dependency_gate(milestone, manifests, state)
        gates["manifest"] = {"status": "FAIL" if dependencies else "PASS", "unsatisfied_dependencies": dependencies}
        if dependencies:
            blocking.append({"code": "UNSATISFIED_DEPENDENCY", "dependencies": dependencies})
        try:
            gates["integrity"] = verifier_integrity(milestone, candidate)
        except HarnessError as exc:
            gates["integrity"] = {"status": "FAIL", "message": str(exc)}
            blocking.append({"code": "VERIFIER_INTEGRITY", "message": str(exc)})
        gates["architecture"] = architecture_check()
        if gates["architecture"]["status"] == "FAIL":
            blocking.extend(gates["architecture"]["findings"])
        gates["verifier"] = run_verifier(manifest)
        if gates["verifier"].get("status") != "PASS":
            blocking.append({"code": "MILESTONE_VERIFIER", "message": gates["verifier"].get("message", gates["verifier"].get("status"))})
        gates["ci"] = run_ci(manifest)
        if gates["ci"]["status"] == "FAIL":
            blocking.extend(gates["ci"].get("failures", []))
        gates["reviews"] = ingest_reviews(manifest, milestone, candidate)
        blocking.extend(gates["reviews"].get("blocking_findings", []))
        warnings.extend(gates["reviews"].get("warnings", []))
    except HarnessError as exc:
        blocking.append({"code": "HARNESS_ERROR", "message": str(exc)})
    overall = "PASS" if not blocking else "FAIL"
    result = {
        "milestone": milestone,
        "candidate_commit": candidate,
        "status": overall,
        "gates": gates,
        "blocking_findings": blocking,
        "warnings": warnings,
    }
    output_dir = repo_root() / "target" / "frontend-harness" / "results"
    write_json_atomic(output_dir / f"{milestone}.json", result)
    print(f"Frontend verification {milestone}: {overall}")
    for name, gate in gates.items():
        print(f"  {name}: {gate.get('status', 'UNKNOWN') if isinstance(gate, dict) else gate}")
    print("__FRONTEND_RESULT__" + json.dumps(result, sort_keys=True))
    if overall == "PASS":
        if status == "verifying":
            set_state(milestone, "reviewing")
        return 0
    if status in {"candidate", "verifying", "reviewing", "fixing"} and os.environ.get("FRONTEND_HARNESS_CLEAN_CHILD") != "1":
        set_state(milestone, "failed")
    return 3 if gates.get("verifier", {}).get("status") == "NOT_IMPLEMENTED" else 1


def accept(milestone: str) -> int:
    root = repo_root()
    if git_status_dirty(root):
        raise HarnessError("frontend-accept requires a clean worktree, including no untracked files")
    state = load_state()
    if state["milestones"][milestone]["status"] not in {"candidate", "reviewing", "verifying"}:
        raise HarnessError(f"{milestone} is not an acceptance candidate")
    code = verify(milestone, clean=True)
    if code:
        return code
    set_state(milestone, "passed", allow_passed=True)
    return 0


def milestone_command(action: str, milestone: str | None = None) -> int:
    manifests = load_all_manifests()
    state = load_state()
    if action == "status":
        for item in MILESTONES:
            print(f"{item}\t{state['milestones'][item]['status']}\t{manifests[item]['title']}")
        return 0
    if action == "next":
        for item in MILESTONES:
            if state["milestones"][item]["status"] == "passed":
                continue
            missing = dependency_gate(item, manifests, state)
            if not missing:
                print(item)
                return 0
        print("NONE")
        return 0
    if milestone is None or milestone not in MILESTONES:
        raise HarnessError("a milestone F0..F12 is required")
    missing = dependency_gate(milestone, manifests, state)
    if missing:
        raise HarnessError(f"unsatisfied milestone dependencies for {milestone}: {', '.join(missing)}")
    if action == "begin":
        set_state(milestone, "implementing")
        updated = load_state()
        updated["milestones"][milestone]["candidate_commit"] = None
        write_json_atomic(state_path(), updated)
    elif action == "candidate":
        set_state(milestone, "candidate")
        updated = load_state()
        updated["milestones"][milestone]["candidate_commit"] = candidate_commit()
        write_json_atomic(state_path(), updated)
    else:
        raise HarnessError(f"unknown milestone action: {action}")
    print(f"{milestone}: {state['milestones'][milestone]['status']} -> {('implementing' if action == 'begin' else 'candidate')}")
    return 0


def frontend_ci() -> int:
    load_all_manifests()
    architecture = architecture_check()
    print(json.dumps({"architecture": architecture}, indent=2, sort_keys=True))
    if architecture["status"] == "FAIL":
        return 1
    state = load_state()
    candidates = [item for item in MILESTONES if state["milestones"][item]["status"] in {"candidate", "verifying", "reviewing", "fixing"}]
    if not candidates:
        print("No frontend milestone candidate is currently staged")
        return 0
    code = 0
    for milestone in candidates:
        code = max(code, verify(milestone))
    return code


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)
    milestone_parser = subparsers.add_parser("milestone")
    milestone_parser.add_argument("action", choices=["status", "next", "begin", "candidate"])
    milestone_parser.add_argument("milestone", nargs="?")
    verify_parser = subparsers.add_parser("verify")
    verify_parser.add_argument("milestone")
    verify_parser.add_argument("--clean-worktree", action="store_true")
    accept_parser = subparsers.add_parser("accept")
    accept_parser.add_argument("milestone")
    subparsers.add_parser("architecture")
    integrity_parser = subparsers.add_parser("integrity")
    integrity_parser.add_argument("milestone")
    subparsers.add_parser("ci")
    args = parser.parse_args(argv)
    try:
        if args.command == "milestone":
            return milestone_command(args.action, args.milestone)
        if args.command == "verify":
            return verify(args.milestone, args.clean_worktree)
        if args.command == "accept":
            return accept(args.milestone)
        if args.command == "architecture":
            result = architecture_check()
            print(json.dumps(result, indent=2, sort_keys=True))
            return 0 if result["status"] in {"PASS", "NOT_APPLICABLE"} else 1
        if args.command == "integrity":
            print(json.dumps(verifier_integrity(args.milestone), indent=2, sort_keys=True))
            return 0
        if args.command == "ci":
            return frontend_ci()
    except HarnessError as exc:
        print(f"frontend harness error: {exc}", file=sys.stderr)
        return 1
    return 2


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
