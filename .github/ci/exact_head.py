#!/usr/bin/env python3
"""Exact-head and change-aware behavioral verification helpers for CI."""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
import subprocess
import sys
from pathlib import Path
from typing import Iterable

LOCAL_SCRATCH_PATTERNS: tuple[str, ...] = (
    "*.tmp",
    "*.log",
    "*.swp",
    ".DS_Store",
    "**/.DS_Store",
)


def _git(repo: Path, *args: str) -> str:
    completed = subprocess.run(
        ["git", *args],
        cwd=repo,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )
    if completed.returncode != 0:
        raise RuntimeError(completed.stdout.strip() or "git command failed")
    return completed.stdout.strip()


def assert_exact_head(expected: str, repo: Path) -> int:
    expected = expected.strip().lower()
    if not expected:
        print("::error title=Exact-head verification::Expected head SHA is empty.")
        return 2
    try:
        actual = _git(repo, "rev-parse", "HEAD").lower()
    except RuntimeError as error:
        print(f"::error title=Exact-head verification::{error}")
        return 2

    if actual != expected:
        print(
            "::error title=Stale verification checkout::"
            f"Expected exact PR head {expected}, checked out {actual}. "
            "Verification from a different SHA cannot authorize this head."
        )
        return 1

    print(f"Verified exact PR head {actual}.")
    return 0


def _load_dag_module():
    # Loading verification policy must not mutate the checkout. In particular,
    # pre-commit immediately scans for untracked behavioral files, so emitting a
    # verification/__pycache__ directory here would make the selector reject its
    # own side effect.
    sys.dont_write_bytecode = True
    module_path = Path(
        os.environ.get(
            "SEARCHKERNEL_VERIFICATION_DAG_MODULE",
            str(Path.cwd() / "verification" / "dag.py"),
        )
    )
    spec = importlib.util.spec_from_file_location("searchkernel_verification_dag", module_path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load verification DAG module from {module_path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def _load_dag():
    module = _load_dag_module()
    dag_path = Path(
        os.environ.get(
            "SEARCHKERNEL_VERIFICATION_DAG",
            str(Path.cwd() / "verification" / "dag.toml"),
        )
    )
    return module, module.load_dag(dag_path)


def _matches_local_scratch(path: str) -> bool:
    import fnmatch

    normalized = path.strip().replace("\\", "/")
    return any(fnmatch.fnmatchcase(normalized, pattern) for pattern in LOCAL_SCRATCH_PATTERNS)


def _normalized_paths(
    paths: Iterable[str], *, allow_local_scratch: bool = False
) -> list[str]:
    normalized = [
        path.strip().replace("\\", "/")
        for path in paths
        if path.strip()
    ]
    if allow_local_scratch:
        normalized = [path for path in normalized if not _matches_local_scratch(path)]
    return normalized


def _plan(
    paths: Iterable[str], *, allow_local_scratch: bool = False
):
    module, dag = _load_dag()
    normalized = _normalized_paths(paths, allow_local_scratch=allow_local_scratch)
    task_ids, reasons, unclassified = module.plan_paths(dag, normalized)
    return module, dag, task_ids, reasons, unclassified


def classify(
    paths: Iterable[str], *, allow_local_scratch: bool = False
) -> dict[str, bool]:
    module, dag, task_ids, _reasons, unclassified = _plan(
        paths, allow_local_scratch=allow_local_scratch
    )
    if unclassified:
        raise module.DagError(
            "unclassified behavioral paths: " + ", ".join(unclassified)
        )
    return module.legacy_lanes_for_tasks(dag, task_ids)


def build_task_plan(paths: Iterable[str]) -> list[dict[str, object]]:
    module, _dag, task_ids, reasons, unclassified = _plan(paths)
    if unclassified:
        raise module.DagError(
            "unclassified behavioral paths: " + ", ".join(unclassified)
        )
    return [
        {
            "id": task_id,
            "reasons": list(reasons.get(task_id, ())),
        }
        for task_id in task_ids
    ]


def _read_changed_paths() -> list[str]:
    raw = sys.stdin.buffer.read()
    chunks = raw.split(b"\0") if b"\0" in raw else raw.splitlines()
    return [chunk.decode("utf-8", errors="strict") for chunk in chunks if chunk]


def write_github_output(
    path: Path, lane_names: Iterable[str], selected: dict[str, bool]
) -> None:
    with path.open("a", encoding="utf-8") as output:
        for lane in lane_names:
            output.write(f"{lane}={'true' if selected[lane] else 'false'}\n")


def _print_unclassified(paths: Iterable[str]) -> None:
    print(
        "::error title=Unclassified behavioral paths::"
        "Verification DAG does not own these paths. "
        "Update trusted verification/dag.toml before merging:"
    )
    for path in paths:
        print(f"  {path}")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)

    head = subparsers.add_parser("assert-head")
    head.add_argument("--expected", required=True)
    head.add_argument("--repo", type=Path, default=Path.cwd())

    changes = subparsers.add_parser("classify")
    changes.add_argument("--github-output", type=Path)
    changes.add_argument("--allow-local-scratch", action="store_true")

    plan = subparsers.add_parser("plan")
    plan.add_argument("--json", action="store_true")
    return parser


def main() -> int:
    args = build_parser().parse_args()
    if args.command == "assert-head":
        return assert_exact_head(args.expected, args.repo)

    changed_paths = _read_changed_paths()

    try:
        if args.command == "plan":
            module, _dag, task_ids, reasons, unclassified = _plan(changed_paths)
            if unclassified:
                _print_unclassified(unclassified)
                return 3
            plan = [
                {"id": task_id, "reasons": list(reasons.get(task_id, ()))}
                for task_id in task_ids
            ]
            if args.json:
                print(json.dumps({"tasks": plan}, sort_keys=True, separators=(",", ":")))
            else:
                print("Required tasks:")
                for task in plan:
                    print(f"  {task['id']}")
                    for reason in task["reasons"]:
                        print(f"    - {reason}")
            return 0

        module, dag, task_ids, _reasons, unclassified = _plan(
            changed_paths,
            allow_local_scratch=args.allow_local_scratch,
        )
        if unclassified:
            _print_unclassified(unclassified)
            return 3
        selected = module.legacy_lanes_for_tasks(dag, task_ids)
        if args.github_output is not None:
            write_github_output(args.github_output, dag.legacy_lane_names, selected)
        print("Changed paths:")
        for changed_path in changed_paths:
            print(f"  {changed_path}")
        print("Behavioral lanes:")
        for lane in dag.legacy_lane_names:
            print(f"  {lane}: {'required' if selected[lane] else 'not required'}")
        return 0
    except Exception as error:
        print(f"::error title=Verification DAG error::{error}")
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
