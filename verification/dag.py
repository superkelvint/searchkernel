#!/usr/bin/env python3
"""SearchKernel-owned verification DAG loader, validator, and selector helpers."""

from __future__ import annotations

import argparse
import fnmatch
import json
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

SCHEMA_VERSION = 1


class DagError(RuntimeError):
    pass


@dataclass(frozen=True)
class Task:
    id: str
    kind: str
    runner: str
    command: str | None
    depends_on: tuple[str, ...]
    legacy_lanes: tuple[str, ...]
    cache_result: bool


@dataclass(frozen=True)
class Subsystem:
    id: str
    paths: tuple[str, ...]
    requires: tuple[str, ...]
    behavioral: bool
    override_exemption: bool
    notes: str | None


@dataclass(frozen=True)
class Exemption:
    id: str
    paths: tuple[str, ...]
    reason: str


@dataclass(frozen=True)
class VerificationDag:
    path: Path
    unknown_behavior: str
    full_roots: tuple[str, ...]
    legacy_lane_names: tuple[str, ...]
    coverage_include: tuple[str, ...]
    tasks: dict[str, Task]
    subsystems: tuple[Subsystem, ...]
    exemptions: tuple[Exemption, ...]


def _tuple_of_strings(value: object, field: str) -> tuple[str, ...]:
    if not isinstance(value, list) or not all(isinstance(item, str) and item for item in value):
        raise DagError(f"{field} must be a non-empty string array")
    return tuple(value)


def _optional_tuple_of_strings(value: object, field: str) -> tuple[str, ...]:
    if value is None:
        return ()
    if not isinstance(value, list) or not all(isinstance(item, str) and item for item in value):
        raise DagError(f"{field} must be a string array")
    return tuple(value)


def _parse_policy_toml(source: str) -> dict[str, object]:
    """Parse the deliberately small TOML subset used by verification/dag.toml.

    The CI runner's supported Python may predate stdlib tomllib. Keeping this
    parser local avoids making trusted verification policy depend on an
    environment-installed package. The accepted subset is intentionally narrow:
    tables, array-of-tables, JSON-compatible scalar values, and string arrays.
    Unsupported TOML syntax fails closed.
    """
    root: dict[str, object] = {}
    current: dict[str, object] = root
    lines = source.splitlines()
    index = 0

    def parse_value(text: str, line_number: int) -> object:
        # TOML permits trailing commas in arrays; JSON does not. The policy
        # subset otherwise deliberately uses JSON-compatible scalar syntax.
        text = re.sub(r",\s*]", "]", text)
        try:
            return json.loads(text)
        except json.JSONDecodeError as error:
            raise DagError(
                f"unsupported TOML value at line {line_number}: {text}"
            ) from error

    while index < len(lines):
        raw_line = lines[index]
        line_number = index + 1
        line = raw_line.strip()
        index += 1
        if not line or line.startswith("#"):
            continue
        if "#" in line:
            raise DagError(
                f"inline TOML comments are not supported at line {line_number}"
            )

        if line.startswith("[[") and line.endswith("]]"):
            name = line[2:-2].strip()
            if not name or "." in name:
                raise DagError(f"unsupported array-table header at line {line_number}")
            bucket = root.setdefault(name, [])
            if not isinstance(bucket, list):
                raise DagError(f"table kind conflict for {name} at line {line_number}")
            entry: dict[str, object] = {}
            bucket.append(entry)
            current = entry
            continue

        if line.startswith("[") and line.endswith("]"):
            name = line[1:-1].strip()
            if not name or "." in name:
                raise DagError(f"unsupported table header at line {line_number}")
            entry = root.setdefault(name, {})
            if not isinstance(entry, dict):
                raise DagError(f"table kind conflict for {name} at line {line_number}")
            current = entry
            continue

        if "=" not in line:
            raise DagError(f"expected key = value at line {line_number}")
        key, value_text = (part.strip() for part in line.split("=", 1))
        if not key or key in current:
            raise DagError(f"invalid or duplicate key {key!r} at line {line_number}")

        if value_text.startswith("["):
            parts = [value_text]
            balance = value_text.count("[") - value_text.count("]")
            while balance > 0 and index < len(lines):
                continuation = lines[index].strip()
                index += 1
                if not continuation or continuation.startswith("#"):
                    continue
                if "#" in continuation:
                    raise DagError(
                        f"inline TOML comments are not supported at line {index}"
                    )
                parts.append(continuation)
                balance += continuation.count("[") - continuation.count("]")
            if balance != 0:
                raise DagError(f"unterminated array starting at line {line_number}")
            value_text = " ".join(parts)

        current[key] = parse_value(value_text, line_number)

    return root


def load_dag(path: Path) -> VerificationDag:
    try:
        raw = _parse_policy_toml(path.read_text(encoding="utf-8"))
    except OSError as error:
        raise DagError(f"cannot load verification DAG {path}: {error}") from error

    if raw.get("schema_version") != SCHEMA_VERSION:
        raise DagError(
            f"unsupported verification DAG schema_version {raw.get('schema_version')!r}; "
            f"expected {SCHEMA_VERSION}"
        )
    if raw.get("unknown_behavior") != "block":
        raise DagError("verification DAG must set unknown_behavior = \"block\"")

    full_roots = _tuple_of_strings(raw.get("full_roots"), "full_roots")
    legacy_lane_names = _tuple_of_strings(raw.get("legacy_lane_names"), "legacy_lane_names")

    coverage = raw.get("coverage")
    if not isinstance(coverage, dict):
        raise DagError("[coverage] table is required")
    coverage_include = _tuple_of_strings(coverage.get("include"), "coverage.include")

    tasks: dict[str, Task] = {}
    for index, entry in enumerate(raw.get("task", [])):
        if not isinstance(entry, dict):
            raise DagError(f"task[{index}] must be a table")
        task_id = entry.get("id")
        if not isinstance(task_id, str) or not task_id:
            raise DagError(f"task[{index}].id must be a non-empty string")
        if task_id in tasks:
            raise DagError(f"duplicate task id: {task_id}")
        kind = entry.get("kind")
        if kind not in {"command", "aggregate"}:
            raise DagError(f"task {task_id}: kind must be command or aggregate")
        runner = entry.get("runner")
        if not isinstance(runner, str) or not runner:
            raise DagError(f"task {task_id}: runner must be a non-empty string")
        command = entry.get("command")
        if kind == "command":
            if not isinstance(command, str) or not command.strip():
                raise DagError(f"task {task_id}: command task requires command")
        elif command is not None:
            raise DagError(f"task {task_id}: aggregate task must not define command")
        depends_on = _optional_tuple_of_strings(entry.get("depends_on"), f"task {task_id}.depends_on")
        legacy_lanes = _optional_tuple_of_strings(
            entry.get("legacy_lanes"), f"task {task_id}.legacy_lanes"
        )
        cache_result = entry.get("cache_result", False)
        if not isinstance(cache_result, bool):
            raise DagError(f"task {task_id}: cache_result must be boolean")
        tasks[task_id] = Task(
            id=task_id,
            kind=kind,
            runner=runner,
            command=command if isinstance(command, str) else None,
            depends_on=depends_on,
            legacy_lanes=legacy_lanes,
            cache_result=cache_result,
        )

    subsystems: list[Subsystem] = []
    seen_subsystems: set[str] = set()
    for index, entry in enumerate(raw.get("subsystem", [])):
        if not isinstance(entry, dict):
            raise DagError(f"subsystem[{index}] must be a table")
        subsystem_id = entry.get("id")
        if not isinstance(subsystem_id, str) or not subsystem_id:
            raise DagError(f"subsystem[{index}].id must be a non-empty string")
        if subsystem_id in seen_subsystems:
            raise DagError(f"duplicate subsystem id: {subsystem_id}")
        seen_subsystems.add(subsystem_id)
        paths = _tuple_of_strings(entry.get("paths"), f"subsystem {subsystem_id}.paths")
        requires = _optional_tuple_of_strings(
            entry.get("requires"), f"subsystem {subsystem_id}.requires"
        )
        behavioral = entry.get("behavioral", True)
        if not isinstance(behavioral, bool):
            raise DagError(f"subsystem {subsystem_id}: behavioral must be boolean")
        override_exemption = entry.get("override_exemption", False)
        if not isinstance(override_exemption, bool):
            raise DagError(f"subsystem {subsystem_id}: override_exemption must be boolean")
        if not behavioral and override_exemption:
            raise DagError(
                f"subsystem {subsystem_id}: non-behavioral subsystem cannot override exemptions"
            )
        if not behavioral and requires:
            raise DagError(
                f"subsystem {subsystem_id}: non-behavioral subsystem must not require tasks"
            )
        notes = entry.get("notes")
        if notes is not None and not isinstance(notes, str):
            raise DagError(f"subsystem {subsystem_id}: notes must be a string")
        subsystems.append(
            Subsystem(
                id=subsystem_id,
                paths=paths,
                requires=requires,
                behavioral=behavioral,
                override_exemption=override_exemption,
                notes=notes,
            )
        )

    exemptions: list[Exemption] = []
    seen_exemptions: set[str] = set()
    for index, entry in enumerate(raw.get("exemption", [])):
        if not isinstance(entry, dict):
            raise DagError(f"exemption[{index}] must be a table")
        exemption_id = entry.get("id")
        if not isinstance(exemption_id, str) or not exemption_id:
            raise DagError(f"exemption[{index}].id must be a non-empty string")
        if exemption_id in seen_exemptions:
            raise DagError(f"duplicate exemption id: {exemption_id}")
        seen_exemptions.add(exemption_id)
        paths = _tuple_of_strings(entry.get("paths"), f"exemption {exemption_id}.paths")
        reason = entry.get("reason")
        if not isinstance(reason, str) or not reason.strip():
            raise DagError(f"exemption {exemption_id}: reason must be a non-empty string")
        exemptions.append(Exemption(exemption_id, paths, reason))

    dag = VerificationDag(
        path=path,
        unknown_behavior="block",
        full_roots=full_roots,
        legacy_lane_names=legacy_lane_names,
        coverage_include=coverage_include,
        tasks=tasks,
        subsystems=tuple(subsystems),
        exemptions=tuple(exemptions),
    )
    validate_dag(dag)
    return dag


def validate_dag(dag: VerificationDag) -> None:
    lane_names = set(dag.legacy_lane_names)
    if len(lane_names) != len(dag.legacy_lane_names):
        raise DagError("legacy_lane_names contains duplicates")

    for root in dag.full_roots:
        if root not in dag.tasks:
            raise DagError(f"full root references unknown task: {root}")

    for task in dag.tasks.values():
        if task.id in task.depends_on:
            raise DagError(f"task {task.id}: cannot depend on itself")
        for dependency in task.depends_on:
            if dependency not in dag.tasks:
                raise DagError(f"task {task.id}: unknown dependency {dependency}")
        for lane in task.legacy_lanes:
            if lane not in lane_names:
                raise DagError(f"task {task.id}: unknown legacy lane {lane}")

    for subsystem in dag.subsystems:
        for task_id in subsystem.requires:
            if task_id not in dag.tasks:
                raise DagError(f"subsystem {subsystem.id}: unknown required task {task_id}")

    visiting: set[str] = set()
    visited: set[str] = set()

    def visit(task_id: str, stack: list[str]) -> None:
        if task_id in visited:
            return
        if task_id in visiting:
            cycle = " -> ".join([*stack, task_id])
            raise DagError(f"task dependency cycle: {cycle}")
        visiting.add(task_id)
        stack.append(task_id)
        for dependency in dag.tasks[task_id].depends_on:
            visit(dependency, stack)
        stack.pop()
        visiting.remove(task_id)
        visited.add(task_id)

    for task_id in dag.tasks:
        visit(task_id, [])

    full_closure = set(task_closure(dag, dag.full_roots))
    required_by_behavioral = {
        task_id
        for subsystem in dag.subsystems
        if subsystem.behavioral
        for task_id in subsystem.requires
    }
    unreachable_required = sorted(required_by_behavioral - full_closure)
    if unreachable_required:
        raise DagError(
            "behavioral subsystem tasks are not reachable from full_roots: "
            + ", ".join(unreachable_required)
        )


def matches(path: str, patterns: Iterable[str]) -> bool:
    normalized = path.strip().replace("\\", "/")
    return any(fnmatch.fnmatchcase(normalized, pattern) for pattern in patterns)


def matching_subsystems(dag: VerificationDag, path: str) -> tuple[Subsystem, ...]:
    return tuple(
        subsystem for subsystem in dag.subsystems if matches(path, subsystem.paths)
    )


def matching_exemptions(dag: VerificationDag, path: str) -> tuple[Exemption, ...]:
    return tuple(exemption for exemption in dag.exemptions if matches(path, exemption.paths))


def task_closure(dag: VerificationDag, roots: Iterable[str]) -> tuple[str, ...]:
    ordered: list[str] = []
    emitted: set[str] = set()

    def add(task_id: str) -> None:
        if task_id in emitted:
            return
        emitted.add(task_id)
        ordered.append(task_id)
        for dependency in dag.tasks[task_id].depends_on:
            add(dependency)

    for root in roots:
        if root not in dag.tasks:
            raise DagError(f"unknown task root: {root}")
        add(root)
    return tuple(ordered)


def plan_paths(
    dag: VerificationDag, paths: Iterable[str]
) -> tuple[tuple[str, ...], dict[str, tuple[str, ...]], tuple[str, ...]]:
    normalized = tuple(
        sorted({path.strip().replace("\\", "/") for path in paths if path.strip()})
    )
    roots: list[str] = []
    reasons: dict[str, list[str]] = {}
    unclassified: list[str] = []

    for path in normalized:
        subsystems = matching_subsystems(dag, path)
        exemptions = matching_exemptions(dag, path)
        if exemptions:
            subsystems = tuple(
                subsystem for subsystem in subsystems if subsystem.override_exemption
            )
            if not subsystems:
                continue
        if subsystems:
            for subsystem in subsystems:
                if not subsystem.behavioral:
                    continue
                for task_id in subsystem.requires:
                    if task_id not in roots:
                        roots.append(task_id)
                    reasons.setdefault(task_id, []).append(
                        f"{path}: subsystem {subsystem.id}"
                    )
            continue
        unclassified.append(path)

    closure = task_closure(dag, roots)
    expanded_reasons: dict[str, tuple[str, ...]] = {}
    for task_id in closure:
        direct = list(reasons.get(task_id, ()))
        if not direct:
            for root_id, root_reasons in reasons.items():
                if task_id in task_closure(dag, (root_id,)):
                    direct.extend(root_reasons)
        expanded_reasons[task_id] = tuple(dict.fromkeys(direct))
    return closure, expanded_reasons, tuple(unclassified)


def legacy_lanes_for_tasks(dag: VerificationDag, task_ids: Iterable[str]) -> dict[str, bool]:
    selected = {lane: False for lane in dag.legacy_lane_names}
    for task_id in task_ids:
        for lane in dag.tasks[task_id].legacy_lanes:
            selected[lane] = True
    return selected


def _tracked_files(repo: Path) -> tuple[str, ...]:
    completed = subprocess.run(
        ["git", "ls-files", "-z"],
        cwd=repo,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if completed.returncode != 0:
        raise DagError(
            completed.stderr.decode("utf-8", errors="replace").strip()
            or "git ls-files failed"
        )
    return tuple(
        chunk.decode("utf-8")
        for chunk in completed.stdout.split(b"\0")
        if chunk
    )


def validate_repository_coverage(dag: VerificationDag, repo: Path) -> None:
    uncovered: list[str] = []
    for path in _tracked_files(repo):
        if not matches(path, dag.coverage_include):
            continue
        if matching_subsystems(dag, path) or matching_exemptions(dag, path):
            continue
        uncovered.append(path)
    if uncovered:
        preview = "\n".join(f"  {path}" for path in uncovered[:100])
        suffix = "" if len(uncovered) <= 100 else f"\n  ... and {len(uncovered) - 100} more"
        raise DagError(
            "tracked production paths are not owned by verification DAG or an explicit "
            f"non-behavioral exemption:\n{preview}{suffix}"
        )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--dag",
        type=Path,
        default=Path("verification/dag.toml"),
        help="verification DAG path",
    )
    subparsers = parser.add_subparsers(dest="command", required=True)
    check = subparsers.add_parser("check")
    check.add_argument("--repo", type=Path, default=Path.cwd())
    plan = subparsers.add_parser("plan")
    plan.add_argument("paths", nargs="+")
    return parser


def main() -> int:
    args = build_parser().parse_args()
    try:
        dag = load_dag(args.dag)
        if args.command == "check":
            validate_repository_coverage(dag, args.repo)
            print(
                f"verification DAG OK: {len(dag.tasks)} tasks, "
                f"{len(dag.subsystems)} subsystems, "
                f"{len(dag.exemptions)} exemptions"
            )
            return 0
        task_ids, reasons, unclassified = plan_paths(dag, args.paths)
        if unclassified:
            print("unclassified behavioral paths:")
            for path in unclassified:
                print(f"  {path}")
            return 3
        for task_id in task_ids:
            print(task_id)
            for reason in reasons.get(task_id, ()):
                print(f"  - {reason}")
        return 0
    except DagError as error:
        print(f"verification DAG error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
