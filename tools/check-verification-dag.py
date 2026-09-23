#!/usr/bin/env python3
"""Validate SearchKernel's authoritative verification DAG."""

from __future__ import annotations

import argparse
import ast
import re
import shlex
import subprocess
import sys
import tomllib
from pathlib import Path
from typing import Any, Iterable

ID_RE = re.compile(r"^[a-z0-9][a-z0-9._-]*$")
ALLOWED_TOP_LEVEL = {
    "schema_version",
    "unknown_behavior",
    "runner_classes",
    "full_roots",
    "task",
    "subsystem",
    "selector_alias",
}
ALLOWED_TASK_KEYS = {
    "id",
    "kind",
    "runner",
    "command",
    "depends_on",
    "subsumes",
    "cache_result",
    "required_in_full",
    "notes",
    "reason",
    "disposition",
}
ALLOWED_SUBSYSTEM_KEYS = {"id", "paths", "requires", "behavioral", "notes"}
ALLOWED_ALIAS_KEYS = {"id", "target"}
KINDS = {"command", "aggregate", "unverified"}


class DagError(ValueError):
    pass


def _list(value: Any, *, field: str) -> list[str]:
    if value is None:
        return []
    if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
        raise DagError(f"{field} must be an array of strings")
    return value


def _unique(records: Iterable[dict[str, Any]], *, kind: str) -> dict[str, dict[str, Any]]:
    result: dict[str, dict[str, Any]] = {}
    for record in records:
        identifier = record.get("id")
        if not isinstance(identifier, str) or not ID_RE.fullmatch(identifier):
            raise DagError(f"invalid {kind} id: {identifier!r}")
        if identifier in result:
            raise DagError(f"duplicate {kind} id: {identifier}")
        result[identifier] = record
    return result


def _check_keys(record: dict[str, Any], allowed: set[str], *, label: str) -> None:
    unknown = sorted(set(record) - allowed)
    if unknown:
        raise DagError(f"{label} has unknown fields: {', '.join(unknown)}")


def _closure(tasks: dict[str, dict[str, Any]], roots: Iterable[str]) -> set[str]:
    reached: set[str] = set()
    stack = list(roots)
    while stack:
        task_id = stack.pop()
        if task_id in reached:
            continue
        reached.add(task_id)
        task = tasks[task_id]
        stack.extend(_list(task.get("depends_on"), field=f"task {task_id}.depends_on"))
        stack.extend(_list(task.get("subsumes"), field=f"task {task_id}.subsumes"))
    return reached


def _check_cycles(tasks: dict[str, dict[str, Any]]) -> None:
    state: dict[str, int] = {}
    path: list[str] = []

    def visit(task_id: str) -> None:
        mark = state.get(task_id, 0)
        if mark == 2:
            return
        if mark == 1:
            start = path.index(task_id)
            cycle = path[start:] + [task_id]
            raise DagError(f"verification DAG cycle: {' -> '.join(cycle)}")
        state[task_id] = 1
        path.append(task_id)
        task = tasks[task_id]
        edges = _list(task.get("depends_on"), field=f"task {task_id}.depends_on")
        edges += _list(task.get("subsumes"), field=f"task {task_id}.subsumes")
        for target in edges:
            visit(target)
        path.pop()
        state[task_id] = 2

    for task_id in tasks:
        visit(task_id)


def _repo_path_tokens(command: str) -> Iterable[str]:
    try:
        tokens = shlex.split(command)
    except ValueError as exc:
        raise DagError(f"invalid command quoting: {command!r}: {exc}") from exc
    for token in tokens:
        if token in {"&&", "||", ";"} or token.startswith("-"):
            continue
        if token.startswith(("http://", "https://", "$")):
            continue
        if any(char in token for char in "*?[]{}"):
            continue
        if token.startswith("./") or "/" in token:
            yield token[2:] if token.startswith("./") else token


def _check_command_paths(tasks: dict[str, dict[str, Any]], repo_root: Path) -> None:
    for task_id, task in tasks.items():
        if task["kind"] != "command":
            continue
        command = task["command"]
        for token in _repo_path_tokens(command):
            path = repo_root / token
            if not path.exists():
                raise DagError(
                    f"task {task_id} command references missing repository path: {token}"
                )


def _literal_assignment(tree: ast.Module, name: str) -> Any:
    for node in tree.body:
        if isinstance(node, (ast.Assign, ast.AnnAssign)):
            targets = node.targets if isinstance(node, ast.Assign) else [node.target]
            if any(isinstance(target, ast.Name) and target.id == name for target in targets):
                value = node.value
                return ast.literal_eval(value)
    raise DagError(f"trusted selector is missing {name}")


def _selector_ids(repo_root: Path) -> set[str]:
    selector = repo_root / ".github/ci/exact_head.py"
    tree = ast.parse(selector.read_text(encoding="utf-8"), filename=str(selector))
    all_tasks = set(_literal_assignment(tree, "ALL_TRUSTED_TASKS"))
    lanes = _literal_assignment(tree, "TASKS_BY_LANE")
    for values in lanes.values():
        all_tasks.update(values)
    return all_tasks


def _check_selector(
    tasks: dict[str, dict[str, Any]],
    aliases: dict[str, dict[str, Any]],
    repo_root: Path,
) -> None:
    selector_ids = _selector_ids(repo_root)
    for alias_id, alias in aliases.items():
        if alias_id not in selector_ids:
            raise DagError(f"stale selector alias not emitted by trusted selector: {alias_id}")
        if alias["target"] not in tasks:
            raise DagError(
                f"selector alias {alias_id} targets unknown DAG task {alias['target']}"
            )
    unresolved = sorted(
        task_id for task_id in selector_ids if task_id not in tasks and task_id not in aliases
    )
    if unresolved:
        raise DagError(
            "trusted selector emits task ids absent from DAG/aliases: "
            + ", ".join(unresolved)
        )


def _parse_verifier_list(repo_root: Path) -> tuple[set[str], set[str]]:
    result = subprocess.run(
        ["bash", "reference/vespa/verify-through.sh", "--list"],
        cwd=repo_root,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    if result.returncode != 0:
        raise DagError(f"verify-through --list failed:\n{result.stdout}")
    semantic: set[str] = set()
    milestones: set[str] = set()
    section: str | None = None
    for raw in result.stdout.splitlines():
        line = raw.strip()
        if line == "Semantic verifier targets:":
            section = "semantic"
            continue
        if line == "Milestone aggregate targets:":
            section = "milestones"
            continue
        if line.endswith(":"):
            section = None
            continue
        if not line:
            continue
        if section == "semantic":
            semantic.add(line)
        elif section == "milestones":
            milestones.add(line)
    return semantic, milestones


def _check_verifier_surface(tasks: dict[str, dict[str, Any]], repo_root: Path) -> None:
    semantic, milestones = _parse_verifier_list(repo_root)
    dag_semantic = {
        task_id
        for task_id, task in tasks.items()
        if task.get("command") == f"./dev verify {task_id}"
        and task_id.startswith(("vespa.", "product."))
    }
    if semantic != dag_semantic:
        missing = sorted(semantic - dag_semantic)
        stale = sorted(dag_semantic - semantic)
        raise DagError(
            "semantic verifier/DAG drift"
            + (f"; missing from DAG: {', '.join(missing)}" if missing else "")
            + (f"; stale in DAG: {', '.join(stale)}" if stale else "")
        )
    dag_milestones = {task_id for task_id in tasks if task_id.startswith("milestone.m")}
    if milestones != dag_milestones:
        missing = sorted(milestones - dag_milestones)
        stale = sorted(dag_milestones - milestones)
        raise DagError(
            "milestone verifier/DAG drift"
            + (f"; missing from DAG: {', '.join(missing)}" if missing else "")
            + (f"; stale in DAG: {', '.join(stale)}" if stale else "")
        )


def _check_repository_family_coverage(
    subsystems: dict[str, dict[str, Any]], repo_root: Path
) -> None:
    patterns = {
        pattern
        for subsystem in subsystems.values()
        for pattern in _list(subsystem.get("paths"), field=f"subsystem {subsystem['id']}.paths")
    }
    feature_root = repo_root / "searchkernel-web/src/features"
    if feature_root.is_dir():
        for feature in sorted(path for path in feature_root.iterdir() if path.is_dir()):
            required = f"searchkernel-web/src/features/{feature.name}/**"
            if required not in patterns:
                raise DagError(
                    f"web feature family lacks explicit DAG ownership: {feature.name}"
                )
    crates_root = repo_root / "core/crates"
    if crates_root.is_dir():
        for crate in sorted(path for path in crates_root.iterdir() if path.is_dir()):
            if not (crate / "Cargo.toml").exists():
                continue
            required = f"core/crates/{crate.name}/**"
            if required not in patterns:
                raise DagError(f"Rust crate lacks explicit DAG ownership: {crate.name}")


def validate(
    data: dict[str, Any],
    *,
    repo_root: Path,
    structure_only: bool = False,
) -> tuple[dict[str, dict[str, Any]], dict[str, dict[str, Any]]]:
    unknown_top = sorted(set(data) - ALLOWED_TOP_LEVEL)
    if unknown_top:
        raise DagError(f"unknown top-level DAG fields: {', '.join(unknown_top)}")
    if data.get("schema_version") != 1:
        raise DagError(f"unsupported schema_version: {data.get('schema_version')!r}")
    if data.get("unknown_behavior") != "block":
        raise DagError("unknown_behavior must be 'block'")

    runners = set(_list(data.get("runner_classes"), field="runner_classes"))
    if not runners:
        raise DagError("runner_classes must be non-empty")

    task_records = data.get("task", [])
    subsystem_records = data.get("subsystem", [])
    alias_records = data.get("selector_alias", [])
    if not isinstance(task_records, list) or not all(isinstance(x, dict) for x in task_records):
        raise DagError("task must contain TOML tables")
    if not isinstance(subsystem_records, list) or not all(
        isinstance(x, dict) for x in subsystem_records
    ):
        raise DagError("subsystem must contain TOML tables")
    if not isinstance(alias_records, list) or not all(isinstance(x, dict) for x in alias_records):
        raise DagError("selector_alias must contain TOML tables")

    tasks = _unique(task_records, kind="task")
    subsystems = _unique(subsystem_records, kind="subsystem")
    aliases = _unique(alias_records, kind="selector alias")

    for task_id, task in tasks.items():
        _check_keys(task, ALLOWED_TASK_KEYS, label=f"task {task_id}")
        kind = task.get("kind")
        if kind not in KINDS:
            raise DagError(f"task {task_id} has unsupported kind: {kind!r}")
        runner = task.get("runner")
        if runner not in runners:
            raise DagError(f"task {task_id} has unknown runner: {runner!r}")
        depends = _list(task.get("depends_on"), field=f"task {task_id}.depends_on")
        subsumes = _list(task.get("subsumes"), field=f"task {task_id}.subsumes")
        if len(depends) != len(set(depends)):
            raise DagError(f"task {task_id} has duplicate depends_on entries")
        if len(subsumes) != len(set(subsumes)):
            raise DagError(f"task {task_id} has duplicate subsumes entries")
        for target in depends + subsumes:
            if target not in tasks:
                raise DagError(f"task {task_id} references unknown task: {target}")
            if target == task_id:
                raise DagError(f"task {task_id} cannot reference itself")
        command = task.get("command")
        if kind == "command":
            if not isinstance(command, str) or not command.strip():
                raise DagError(f"command task {task_id} must define command")
        elif command is not None:
            raise DagError(f"{kind} task {task_id} must not define command")
        if kind == "unverified":
            reason = task.get("reason")
            if not isinstance(reason, str) or not reason.strip():
                raise DagError(f"unverified task {task_id} must explain reason")
        if "required_in_full" in task and not isinstance(task["required_in_full"], bool):
            raise DagError(f"task {task_id}.required_in_full must be boolean")

    for subsystem_id, subsystem in subsystems.items():
        _check_keys(subsystem, ALLOWED_SUBSYSTEM_KEYS, label=f"subsystem {subsystem_id}")
        paths = _list(subsystem.get("paths"), field=f"subsystem {subsystem_id}.paths")
        if not paths:
            raise DagError(f"subsystem {subsystem_id} must own at least one path")
        requires = _list(
            subsystem.get("requires"), field=f"subsystem {subsystem_id}.requires"
        )
        behavioral = subsystem.get("behavioral", True)
        if not isinstance(behavioral, bool):
            raise DagError(f"subsystem {subsystem_id}.behavioral must be boolean")
        if behavioral and not requires:
            raise DagError(
                f"behavioral subsystem {subsystem_id} must require verification tasks"
            )
        for target in requires:
            if target not in tasks:
                raise DagError(
                    f"subsystem {subsystem_id} requires unknown task: {target}"
                )

    for alias_id, alias in aliases.items():
        _check_keys(alias, ALLOWED_ALIAS_KEYS, label=f"selector alias {alias_id}")
        target = alias.get("target")
        if not isinstance(target, str) or target not in tasks:
            raise DagError(f"selector alias {alias_id} targets unknown task: {target!r}")

    _check_cycles(tasks)

    roots = _list(data.get("full_roots"), field="full_roots")
    if not roots:
        raise DagError("full_roots must be non-empty")
    for root in roots:
        if root not in tasks:
            raise DagError(f"full_roots references unknown task: {root}")
        if tasks[root]["kind"] != "aggregate":
            raise DagError(f"full root must be aggregate: {root}")

    reached = _closure(tasks, roots)
    required = {
        task_id
        for task_id, task in tasks.items()
        if task.get("required_in_full", True)
    }
    unreachable = sorted(required - reached)
    if unreachable:
        raise DagError(
            "required tasks are unreachable from full_roots: " + ", ".join(unreachable)
        )

    if not structure_only:
        _check_command_paths(tasks, repo_root)
        _check_selector(tasks, aliases, repo_root)
        _check_verifier_surface(tasks, repo_root)
        _check_repository_family_coverage(subsystems, repo_root)

    return tasks, subsystems


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--dag", type=Path, default=Path("verification/dag.toml"))
    parser.add_argument("--repo-root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--structure-only", action="store_true")
    args = parser.parse_args()

    repo_root = args.repo_root.resolve()
    dag_path = args.dag
    if not dag_path.is_absolute():
        dag_path = repo_root / dag_path
    try:
        data = tomllib.loads(dag_path.read_text(encoding="utf-8"))
        tasks, subsystems = validate(
            data, repo_root=repo_root, structure_only=args.structure_only
        )
    except (OSError, tomllib.TOMLDecodeError, DagError) as exc:
        print(f"verification DAG FAILED: {exc}", file=sys.stderr)
        return 1

    unverified = sorted(
        task_id for task_id, task in tasks.items() if task["kind"] == "unverified"
    )
    print(
        f"verification DAG PASS: {len(tasks)} tasks, {len(subsystems)} subsystems"
        + (f"; explicit unverified tasks: {', '.join(unverified)}" if unverified else "")
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
