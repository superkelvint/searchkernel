#!/usr/bin/env python3
"""Validate the GitHub issue queue's labels and canonical claim branches."""

from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.error
import urllib.parse
import urllib.request
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


def validate_closed_issue_labels(
    issues: Sequence[dict[str, Any]],
) -> list[str]:
    """Reject canonical workflow-state labels on closed issues."""
    errors: list[str] = []
    seen_numbers: set[int] = set()

    for issue in sorted(issues, key=_issue_number):
        number = _issue_number(issue)
        if number in seen_numbers:
            errors.append(f"issue #{number}: duplicate issue in closed snapshot")
            continue
        seen_numbers.add(number)
        title = _issue_title(issue)
        description = f"issue #{number} ({title})" if title else f"issue #{number}"
        for state in sorted(_labels(issue) & WORKFLOW_STATES):
            errors.append(
                f"{description}: closed issue retains workflow state label {state}"
            )

    return errors


def _github_json(
    repo: str,
    path: str,
    *,
    page: int | None = None,
    state: str | None = None,
) -> Any:
    token = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
    if not token:
        raise QueueError("GH_TOKEN or GITHUB_TOKEN is required for live issue queue validation")

    query = {"per_page": "100"}
    if page is not None:
        query["page"] = str(page)
    if state is not None:
        query["state"] = state
    url = f"https://api.github.com/repos/{repo}/{path}?{urllib.parse.urlencode(query)}"
    request = urllib.request.Request(
        url,
        headers={
            "Accept": "application/vnd.github+json",
            "Authorization": f"Bearer {token}",
            "X-GitHub-Api-Version": "2022-11-28",
            "User-Agent": "searchkernel-issue-queue-check",
        },
    )
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            return json.load(response)
    except (urllib.error.HTTPError, urllib.error.URLError, TimeoutError) as error:
        detail = getattr(error, "reason", None) or str(error)
        raise QueueError(f"GitHub API request failed for {path}: {detail}") from error


def load_issues(repo: str, *, state: str) -> list[dict[str, Any]]:
    if state not in {"open", "closed"}:
        raise QueueError(f"unsupported issue state: {state}")

    issues: list[dict[str, Any]] = []
    seen_numbers: set[int] = set()
    page = 1
    while True:
        value = _github_json(repo, "issues", page=page, state=state)
        if not isinstance(value, list) or not all(isinstance(item, dict) for item in value):
            raise QueueError("GitHub issues API returned an invalid issue snapshot")
        # GitHub's REST issues collection includes pull requests. Queue state is
        # defined only for issues, matching `gh issue list` semantics.
        #
        # Pagination is not a transactional snapshot: an issue changing state while
        # pages are fetched can shift the collection and make the same issue appear
        # on adjacent pages. Normalize that transport-level overlap here; the pure
        # validators still reject duplicate numbers in deterministic snapshots.
        for item in value:
            if "pull_request" in item:
                continue
            number = item.get("number")
            if isinstance(number, int) and number > 0:
                if number in seen_numbers:
                    continue
                seen_numbers.add(number)
            issues.append(item)
        if len(value) < 100:
            break
        page += 1
    return issues


def load_open_issues(repo: str) -> list[dict[str, Any]]:
    return load_issues(repo, state="open")


def load_closed_issues(repo: str) -> list[dict[str, Any]]:
    return load_issues(repo, state="closed")


def load_claim_branches(repo: str) -> set[str]:
    value = _github_json(repo, "git/matching-refs/heads/codex/issue-")
    if not isinstance(value, list) or not all(isinstance(item, dict) for item in value):
        raise QueueError("GitHub matching-refs API returned an invalid branch snapshot")
    result: set[str] = set()
    for item in value:
        ref = item.get("ref")
        if isinstance(ref, str) and ref.startswith("refs/heads/"):
            result.add(ref.removeprefix("refs/heads/"))
    return result


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", required=True, help="GitHub repository owner/name")
    args = parser.parse_args(argv)

    try:
        issues = load_open_issues(args.repo)
        closed_issues = load_closed_issues(args.repo)
        branches = load_claim_branches(args.repo)
        errors = validate_queue(issues, branches)
        errors.extend(validate_closed_issue_labels(closed_issues))
    except (QueueError, json.JSONDecodeError) as error:
        print(f"Issue queue check FAILED: {error}", file=sys.stderr)
        return 2

    if errors:
        print("Issue queue check FAILED:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print(
        f"Issue queue check PASS ({len(issues)} open issues; "
        f"{len(closed_issues)} closed issues)."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
