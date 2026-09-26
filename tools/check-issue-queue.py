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
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
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
DEFAULT_STALE_AFTER_HOURS = 24


@dataclass(frozen=True)
class ReconcileAction:
    """One conservative issue-state repair."""

    kind: str
    issue_number: int
    value: str | None = None
    reason: str = ""


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



def _updated_at(issue: dict[str, Any]) -> datetime | None:
    value = issue.get("updated_at")
    if not isinstance(value, str) or not value:
        return None
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return None
    if parsed.tzinfo is None:
        return None
    return parsed.astimezone(timezone.utc)


def plan_reconciliation(
    open_issues: Sequence[dict[str, Any]],
    closed_issues: Sequence[dict[str, Any]],
    claim_branches: Iterable[str],
    open_pr_heads: Iterable[str],
    *,
    now: datetime,
    stale_after: timedelta,
) -> list[ReconcileAction]:
    """Plan only repairs that are safe without deleting repository data.

    Closed issues can always shed workflow-state labels. Open in-progress work
    receives a lease based on the issue's updated_at timestamp. Once that lease
    expires, an open implementation PR keeps the claim active. A stale claim
    branch with no PR is preserved and moved to blocked; an in-progress issue
    with no claim branch can safely return to ready.

    Missing or multiple workflow states are intentionally not reconstructed:
    those remain validator errors because trackers and other special issues
    cannot safely be inferred from branch state alone.
    """
    if now.tzinfo is None:
        raise QueueError("reconciliation clock must be timezone-aware")
    if stale_after < timedelta(0):
        raise QueueError("stale_after must not be negative")

    actions: list[ReconcileAction] = []
    branches = set(claim_branches)
    pr_heads = set(open_pr_heads)

    for issue in sorted(closed_issues, key=_issue_number):
        number = _issue_number(issue)
        if _labels(issue) & WORKFLOW_STATES:
            actions.append(
                ReconcileAction(
                    "remove-workflow-states",
                    number,
                    reason="closed issues do not participate in the active workflow queue",
                )
            )

    for issue in sorted(open_issues, key=_issue_number):
        number = _issue_number(issue)
        state_labels = _labels(issue) & WORKFLOW_STATES
        if state_labels != {"status:in-progress"}:
            continue

        updated = _updated_at(issue)
        if updated is None:
            continue
        if now.astimezone(timezone.utc) - updated < stale_after:
            continue

        branch = f"codex/issue-{number}"
        if branch in pr_heads:
            continue

        if branch in branches:
            actions.append(
                ReconcileAction(
                    "set-state",
                    number,
                    "status:blocked",
                    "stale claim branch has no open implementation PR; preserving the ref",
                )
            )
        else:
            actions.append(
                ReconcileAction(
                    "set-state",
                    number,
                    "status:ready",
                    "stale in-progress issue has no canonical claim branch",
                )
            )

    return actions


def _github_request(
    repo: str,
    path: str,
    *,
    method: str = "GET",
    page: int | None = None,
    state: str | None = None,
    payload: dict[str, Any] | None = None,
) -> Any:
    token = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
    if not token:
        raise QueueError("GH_TOKEN or GITHUB_TOKEN is required for live issue queue validation")

    query = {"per_page": "100"}
    if page is not None:
        query["page"] = str(page)
    if state is not None:
        query["state"] = state
    url = f"https://api.github.com/repos/{repo}/{path}"
    if query:
        url = f"{url}?{urllib.parse.urlencode(query)}"

    data = None
    if payload is not None:
        data = json.dumps(payload).encode("utf-8")

    request = urllib.request.Request(
        url,
        data=data,
        method=method,
        headers={
            "Accept": "application/vnd.github+json",
            "Authorization": f"Bearer {token}",
            "Content-Type": "application/json",
            "X-GitHub-Api-Version": "2022-11-28",
            "User-Agent": "searchkernel-issue-queue-check",
        },
    )
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            if response.status == 204:
                return None
            return json.load(response)
    except (urllib.error.HTTPError, urllib.error.URLError, TimeoutError) as error:
        detail = getattr(error, "reason", None) or str(error)
        raise QueueError(
            f"GitHub API {method} failed for {path}: {detail}"
        ) from error


def _github_json(
    repo: str,
    path: str,
    *,
    page: int | None = None,
    state: str | None = None,
) -> Any:
    return _github_request(repo, path, page=page, state=state)


def _github_patch(repo: str, path: str, payload: dict[str, Any]) -> Any:
    return _github_request(repo, path, method="PATCH", payload=payload)


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


def load_open_pr_heads(repo: str) -> set[str]:
    """Return same-repository head refs for all open pull requests."""
    result: set[str] = set()
    page = 1
    while True:
        value = _github_json(repo, "pulls", page=page, state="open")
        if not isinstance(value, list) or not all(isinstance(item, dict) for item in value):
            raise QueueError("GitHub pulls API returned an invalid pull request snapshot")
        for item in value:
            head = item.get("head")
            if not isinstance(head, dict):
                continue
            ref = head.get("ref")
            head_repo = head.get("repo")
            full_name = head_repo.get("full_name") if isinstance(head_repo, dict) else None
            if isinstance(ref, str) and full_name == repo:
                result.add(ref)
        if len(value) < 100:
            break
        page += 1
    return result


def _live_issue(repo: str, number: int) -> dict[str, Any]:
    value = _github_json(repo, f"issues/{number}")
    if not isinstance(value, dict) or value.get("number") != number:
        raise QueueError(f"GitHub issue #{number} returned an invalid payload")
    return value


def _replace_workflow_state(labels: set[str], state: str | None) -> list[str]:
    result = labels - WORKFLOW_STATES
    if state is not None:
        if state not in WORKFLOW_STATES:
            raise QueueError(f"unsupported workflow state: {state}")
        result.add(state)
    return sorted(result)


def apply_reconciliation(repo: str, actions: Sequence[ReconcileAction]) -> list[str]:
    """Apply a precomputed plan with a final state check before each mutation."""
    messages: list[str] = []
    for action in actions:
        issue = _live_issue(repo, action.issue_number)
        labels = _labels(issue)
        state_labels = labels & WORKFLOW_STATES

        if action.kind == "remove-workflow-states":
            if issue.get("state") != "closed" or not state_labels:
                continue
            target = _replace_workflow_state(labels, None)
        elif action.kind == "set-state":
            if (
                issue.get("state") != "open"
                or state_labels != {"status:in-progress"}
                or action.value not in {"status:ready", "status:blocked"}
            ):
                continue
            target = _replace_workflow_state(labels, action.value)
        else:
            raise QueueError(f"unsupported reconciliation action: {action.kind}")

        _github_patch(
            repo,
            f"issues/{action.issue_number}",
            {"labels": target},
        )
        suffix = f" -> {action.value}" if action.value else ""
        messages.append(
            f"issue #{action.issue_number}: {action.kind}{suffix} ({action.reason})"
        )
    return messages


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", required=True, help="GitHub repository owner/name")
    parser.add_argument(
        "--fix",
        action="store_true",
        help="repair conservative stale workflow-state cases before validating",
    )
    parser.add_argument(
        "--stale-after-hours",
        type=float,
        default=DEFAULT_STALE_AFTER_HOURS,
        help=(
            "minimum issue inactivity before repairing open status:in-progress "
            f"claims (default: {DEFAULT_STALE_AFTER_HOURS})"
        ),
    )
    args = parser.parse_args(argv)

    if args.stale_after_hours < 0:
        parser.error("--stale-after-hours must not be negative")

    try:
        issues = load_open_issues(args.repo)
        closed_issues = load_closed_issues(args.repo)
        branches = load_claim_branches(args.repo)

        if args.fix:
            pr_heads = load_open_pr_heads(args.repo)
            actions = plan_reconciliation(
                issues,
                closed_issues,
                branches,
                pr_heads,
                now=datetime.now(timezone.utc),
                stale_after=timedelta(hours=args.stale_after_hours),
            )
            messages = apply_reconciliation(args.repo, actions)
            for message in messages:
                print(f"reconciled: {message}")

            # Validate a fresh snapshot after mutation. The reconciler is not a
            # substitute for the structural checker and never masks ambiguity.
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

    verb = "reconcile" if args.fix else "check"
    print(
        f"Issue queue {verb} PASS ({len(issues)} open issues; "
        f"{len(closed_issues)} closed issues)."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
