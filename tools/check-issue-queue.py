#!/usr/bin/env python3
"""Validate the GitHub issue queue's labels and canonical claim branches."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from collections.abc import Iterable, Sequence
from typing import Any


WORKFLOW_STATES = {
    "status:ready",
    "status:in-progress",
    "status:blocked",
    "status:needs-followup",
}
PRIORITY_LABELS = {f"priority:p{level}" for level in range(4)}
LEGACY_WORKFLOW_LABELS = {
    "in-progress",
    "needs-followup",
    "followup-in-progress",
}
REQUIRED_CLASSIFICATION_STATES = WORKFLOW_STATES - {"status:needs-followup"}


class QueueError(ValueError):
    """A queue consistency error or GitHub collection failure."""


def _labels(issue: dict[str, Any]) -> set[str]:
    raw_labels = issue.get("labels", [])
    if not isinstance(raw_labels, list):
        raise QueueError(f"issue #{issue.get('number', '?')} labels must be a list")
    result: set[str] = set()
    for label in raw_labels:
        if isinstance(label, str):
            result.add(label)
        elif isinstance(label, dict) and isinstance(label.get("name"), str):
            result.add(label["name"])
        else:
            raise QueueError(
                f"issue #{issue.get('number', '?')} contains an invalid label"
            )
    return result


def _issue_title(issue: dict[str, Any]) -> str:
    title = issue.get("title", "")
    return title if isinstance(title, str) else ""


def _issue_number(issue: dict[str, Any]) -> int:
    number = issue.get("number")
    if not isinstance(number, int) or number <= 0:
        raise QueueError(f"issue has invalid number: {number!r}")
    return number


def _classification_labels(labels: Iterable[str], prefix: str) -> set[str]:
    return {label for label in labels if label.startswith(prefix)}


def validate_queue(
    issues: Sequence[dict[str, Any]],
    claim_branches: Iterable[str],
) -> list[str]:
    """Return deterministic consistency errors for an open issue snapshot."""
    errors: list[str] = []
    branches = set(claim_branches)
    seen_numbers: set[int] = set()

    for issue in sorted(issues, key=_issue_number):
        number = _issue_number(issue)
        if number in seen_numbers:
            errors.append(f"issue #{number}: duplicate issue in snapshot")
            continue
        seen_numbers.add(number)
        title = _issue_title(issue)
        description = f"issue #{number} ({title})" if title else f"issue #{number}"
        labels = _labels(issue)

        state_labels = labels & WORKFLOW_STATES
        if len(state_labels) != 1:
            rendered = ", ".join(sorted(state_labels)) or "none"
            errors.append(
                f"{description}: expected exactly one workflow state label; found {rendered}"
            )
            state = None
        else:
            state = next(iter(state_labels))

        legacy = sorted(labels & LEGACY_WORKFLOW_LABELS)
        if legacy:
            errors.append(
                f"{description}: uses legacy workflow label(s): {', '.join(legacy)}"
            )

        priorities = _classification_labels(labels, "priority:")
        if state in REQUIRED_CLASSIFICATION_STATES and (
            len(priorities) != 1 or not priorities <= PRIORITY_LABELS
        ):
            rendered = ", ".join(sorted(priorities)) or "none"
            errors.append(
                f"{description}: expected exactly one priority label; found {rendered}"
            )

        if state in REQUIRED_CLASSIFICATION_STATES:
            areas = _classification_labels(labels, "area:")
            if not areas:
                errors.append(f"{description}: missing area:* label")
            types = _classification_labels(labels, "type:")
            if len(types) != 1:
                rendered = ", ".join(sorted(types)) or "none"
                errors.append(
                    f"{description}: expected exactly one type:* label; found {rendered}"
                )

        canonical_branch = f"codex/issue-{number}"
        if state == "status:ready" and canonical_branch in branches:
            errors.append(
                f"{description}: status:ready has active claim branch {canonical_branch}"
            )
        if state == "status:in-progress" and canonical_branch not in branches:
            errors.append(
                f"{description}: status:in-progress lacks claim branch {canonical_branch}"
            )

    return errors


def _run_gh(args: Sequence[str]) -> str:
    try:
        completed = subprocess.run(
            ["gh", *args],
            check=True,
            capture_output=True,
            text=True,
        )
    except FileNotFoundError as error:
        raise QueueError("gh CLI is required for live issue queue validation") from error
    except subprocess.CalledProcessError as error:
        detail = error.stderr.strip() or error.stdout.strip() or "unknown gh error"
        raise QueueError(f"gh {' '.join(args)} failed: {detail}") from error
    return completed.stdout


def load_open_issues(repo: str) -> list[dict[str, Any]]:
    payload = _run_gh(
        [
            "issue",
            "list",
            "--repo",
            repo,
            "--state",
            "open",
            "--limit",
            "1000",
            "--json",
            "number,title,labels",
        ]
    )
    value = json.loads(payload)
    if not isinstance(value, list) or not all(isinstance(item, dict) for item in value):
        raise QueueError("gh issue list returned an invalid issue snapshot")
    return value


def load_claim_branches(repo: str) -> set[str]:
    payload = _run_gh(
        [
            "api",
            f"repos/{repo}/git/matching-refs/heads/codex/issue-",
            "--jq",
            ".[].ref",
        ]
    )
    return {
        line.removeprefix("refs/heads/")
        for line in payload.splitlines()
        if line.startswith("refs/heads/")
    }


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", required=True, help="GitHub repository owner/name")
    args = parser.parse_args(argv)

    try:
        issues = load_open_issues(args.repo)
        branches = load_claim_branches(args.repo)
        errors = validate_queue(issues, branches)
    except (QueueError, json.JSONDecodeError) as error:
        print(f"Issue queue check FAILED: {error}", file=sys.stderr)
        return 2

    if errors:
        print("Issue queue check FAILED:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print(f"Issue queue check PASS ({len(issues)} open issues).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
