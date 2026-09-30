#!/usr/bin/env python3
"""Close issues resolved by merged PRs and repair stale in-progress labels."""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
from collections import defaultdict
from collections.abc import Sequence
from dataclasses import dataclass
from datetime import datetime
from typing import Any


WORKFLOW_LABELS = {
    "status:ready",
    "status:in-progress",
    "status:blocked",
    "status:needs-followup",
    "in-progress",
    "needs-followup",
    "followup-in-progress",
}
CANONICAL_WORKFLOW_LABELS = {
    "status:ready",
    "status:in-progress",
    "status:blocked",
    "status:needs-followup",
}
IN_PROGRESS_LABELS = {"status:in-progress", "in-progress"}

PULL_REQUESTS_QUERY = """
query($owner: String!, $name: String!, $endCursor: String) {
  repository(owner: $owner, name: $name) {
    pullRequests(first: 100, after: $endCursor, states: [OPEN, CLOSED, MERGED]) {
      nodes {
        number
        state
        baseRefName
        mergedAt
        closingIssuesReferences(first: 100) {
          nodes {
            number
            timelineItems(last: 1, itemTypes: [REOPENED_EVENT]) {
              nodes {
                ... on ReopenedEvent { createdAt }
              }
            }
          }
          pageInfo { hasNextPage }
        }
      }
      pageInfo { hasNextPage endCursor }
    }
  }
}
"""


class CleanupError(ValueError):
    """Invalid GitHub data or a failed gh command."""


@dataclass(frozen=True)
class CleanupAction:
    kind: str
    issue_number: int
    remove_labels: tuple[str, ...] = ()
    add_labels: tuple[str, ...] = ()
    reason: str = ""


def _labels(issue: dict[str, Any]) -> set[str]:
    raw = issue.get("labels", [])
    if not isinstance(raw, list):
        raise CleanupError(f"issue #{issue.get('number', '?')} has invalid labels")
    result = set()
    for label in raw:
        if isinstance(label, str):
            result.add(label)
        elif isinstance(label, dict) and isinstance(label.get("name"), str):
            result.add(label["name"])
        else:
            raise CleanupError(f"issue #{issue.get('number', '?')} has invalid labels")
    return result


def _issue_number(issue: dict[str, Any]) -> int:
    number = issue.get("number")
    if not isinstance(number, int) or number <= 0:
        raise CleanupError(f"issue has invalid number: {number!r}")
    return number


def _github_time(value: Any, context: str) -> datetime | None:
    if value is None:
        return None
    if not isinstance(value, str) or not value:
        raise CleanupError(f"{context} has invalid timestamp")
    try:
        return datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as error:
        raise CleanupError(f"{context} has invalid timestamp: {value!r}") from error


def plan_cleanup(
    issues: Sequence[dict[str, Any]],
    pull_requests: Sequence[dict[str, Any]],
    default_branch: str = "main",
) -> list[CleanupAction]:
    """Plan safe cleanup for issues whose closing-reference PRs are terminal."""
    if not isinstance(default_branch, str) or not default_branch:
        raise CleanupError("repository has invalid default branch")
    prs_by_issue: dict[int, list[dict[str, Any]]] = defaultdict(list)
    for pr in pull_requests:
        number = pr.get("number")
        state = pr.get("state")
        refs = pr.get("closing_issue_numbers", [])
        base_ref_name = pr.get("base_ref_name")
        merged_at = pr.get("merged_at")
        reopened_at_by_issue = pr.get("closing_issue_reopened_at", {})
        if not isinstance(number, int) or number <= 0:
            raise CleanupError(f"pull request has invalid number: {number!r}")
        if state not in {"OPEN", "CLOSED", "MERGED"}:
            raise CleanupError(f"pull request #{number} has invalid state: {state!r}")
        if not isinstance(base_ref_name, str) or not base_ref_name:
            raise CleanupError(f"pull request #{number} has invalid base branch")
        if not isinstance(refs, list) or not all(
            isinstance(ref, int) and ref > 0 for ref in refs
        ):
            raise CleanupError(f"pull request #{number} has invalid closing issue references")
        if state == "MERGED" and _github_time(
            merged_at, f"pull request #{number} merged_at"
        ) is None:
            raise CleanupError(f"pull request #{number} is merged without merged_at")
        if not isinstance(reopened_at_by_issue, dict):
            raise CleanupError(
                f"pull request #{number} has invalid closing issue reopen chronology"
            )
        for issue_number in refs:
            reopened_at = reopened_at_by_issue.get(str(issue_number))
            if reopened_at is not None:
                _github_time(
                    reopened_at,
                    f"issue #{issue_number} latest reopen from pull request #{number}",
                )
            prs_by_issue[issue_number].append(pr)

    actions: list[CleanupAction] = []
    seen: set[int] = set()
    for issue in sorted(issues, key=_issue_number):
        number = _issue_number(issue)
        if number in seen:
            raise CleanupError(f"duplicate issue #{number} in GitHub snapshot")
        seen.add(number)
        state = issue.get("state")
        if state not in {"open", "closed"}:
            raise CleanupError(f"issue #{number} has invalid state: {state!r}")
        labels = _labels(issue)
        in_progress = tuple(sorted(labels & IN_PROGRESS_LABELS))
        linked_prs = prs_by_issue.get(number, [])

        if state == "closed":
            # Closed issues should not retain queue state. This also cleans old
            # in-progress labels on issues that are no longer active.
            stale = tuple(sorted(labels & WORKFLOW_LABELS))
            if stale:
                actions.append(
                    CleanupAction(
                        "remove-labels",
                        number,
                        remove_labels=stale,
                        reason="closed issues do not participate in the workflow queue",
                    )
                )
            continue

        if not linked_prs or any(pr["state"] == "OPEN" for pr in linked_prs):
            continue

        default_branch_merges = [
            pr
            for pr in linked_prs
            if pr["state"] == "MERGED" and pr["base_ref_name"] == default_branch
        ]
        qualifying_merges = []
        for pr in default_branch_merges:
            merged_at = _github_time(
                pr.get("merged_at"), f"pull request #{pr['number']} merged_at"
            )
            reopened_at = _github_time(
                pr.get("closing_issue_reopened_at", {}).get(str(number)),
                f"issue #{number} latest reopen from pull request #{pr['number']}",
            )
            if reopened_at is None or (merged_at is not None and merged_at > reopened_at):
                qualifying_merges.append(pr)

        if qualifying_merges:
            actions.append(
                CleanupAction(
                    "close-issue",
                    number,
                    remove_labels=tuple(sorted(labels & WORKFLOW_LABELS)),
                    reason=(
                        "all linked closing-reference PRs are terminal and one "
                        "merged to the default branch"
                    ),
                )
            )
            continue

        # A deliberate reopen after every default-branch fixing merge supersedes
        # those old closing references. Leave both issue state and queue labels
        # untouched until a newer qualifying merge exists.
        if default_branch_merges:
            continue

        if in_progress:
            remaining_state_labels = labels & (
                CANONICAL_WORKFLOW_LABELS - IN_PROGRESS_LABELS
            )
            add_labels = () if remaining_state_labels else ("status:ready",)
            had_non_default_merge = any(
                pr["state"] == "MERGED" for pr in linked_prs
            )
            reason = (
                "all linked closing-reference PRs are terminal without "
                "a merge to the default branch"
                if had_non_default_merge
                else "all linked closing-reference PRs closed without a merge"
            )
            actions.append(
                CleanupAction(
                    "return-to-ready",
                    number,
                    remove_labels=in_progress,
                    add_labels=add_labels,
                    reason=reason,
                )
            )

    return actions


def _run_gh(arguments: Sequence[str]) -> str:
    gh = shutil.which("gh")
    if gh is None:
        raise CleanupError("GitHub CLI 'gh' is required; install it and run 'gh auth login'")
    result = subprocess.run(
        [gh, *arguments],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        detail = result.stderr.strip()
        message = "gh command failed: " + " ".join(arguments[:3])
        if detail:
            message += ": " + detail
        raise CleanupError(message)
    return result.stdout


def _json_from_gh(arguments: Sequence[str]) -> Any:
    output = _run_gh(arguments)
    try:
        return json.loads(output)
    except json.JSONDecodeError as error:
        raise CleanupError("gh returned invalid JSON") from error


def _graphql_pages(repo: str) -> list[dict[str, Any]]:
    owner, name = repo.split("/", 1)
    result: list[dict[str, Any]] = []
    cursor: str | None = None
    seen_cursors: set[str] = set()

    while True:
        arguments = [
            "api",
            "graphql",
            "-F",
            f"owner={owner}",
            "-F",
            f"name={name}",
        ]
        if cursor is not None:
            arguments.extend(["-F", f"endCursor={cursor}"])
        arguments.extend(["-f", f"query={PULL_REQUESTS_QUERY}"])
        page = _json_from_gh(arguments)

        try:
            connection = page["data"]["repository"]["pullRequests"]
            prs = connection["nodes"]
            page_info = connection["pageInfo"]
            has_next_page = page_info["hasNextPage"]
            end_cursor = page_info.get("endCursor")
        except (KeyError, TypeError) as error:
            raise CleanupError("gh GraphQL response omitted pull request data") from error
        if not isinstance(prs, list) or not isinstance(has_next_page, bool):
            raise CleanupError("gh GraphQL response had invalid pull request pagination")
        for pr in prs:
            if not isinstance(pr, dict):
                raise CleanupError("gh GraphQL response contained an invalid pull request")
            try:
                references = pr["closingIssuesReferences"]
                nodes = references["nodes"]
                has_more = references["pageInfo"]["hasNextPage"]
                state = pr["state"]
                number = pr["number"]
                base_ref_name = pr["baseRefName"]
                merged_at = pr.get("mergedAt")
            except (KeyError, TypeError) as error:
                raise CleanupError("gh GraphQL response omitted PR closing references") from error
            if has_more:
                raise CleanupError(
                    f"pull request #{number} has more than 100 closing issue references"
                )
            if not isinstance(base_ref_name, str) or not base_ref_name:
                raise CleanupError(f"pull request #{number} has invalid base branch")
            if not isinstance(nodes, list) or not all(
                isinstance(node, dict) and isinstance(node.get("number"), int)
                for node in nodes
            ):
                raise CleanupError(f"pull request #{number} has invalid closing references")
            reopened_at_by_issue: dict[str, str | None] = {}
            for node in nodes:
                timeline = node.get("timelineItems")
                if not isinstance(timeline, dict):
                    raise CleanupError(
                        f"pull request #{number} closing issue chronology is missing"
                    )
                events = timeline.get("nodes")
                if not isinstance(events, list) or len(events) > 1:
                    raise CleanupError(
                        f"pull request #{number} closing issue chronology is invalid"
                    )
                reopened_at = None
                if events:
                    event = events[0]
                    if not isinstance(event, dict):
                        raise CleanupError(
                            f"pull request #{number} closing issue reopen event is invalid"
                        )
                    reopened_at = event.get("createdAt")
                    _github_time(
                        reopened_at,
                        f"issue #{node['number']} latest reopen from pull request #{number}",
                    )
                reopened_at_by_issue[str(node["number"])] = reopened_at
            if state == "MERGED":
                _github_time(merged_at, f"pull request #{number} mergedAt")
            result.append(
                {
                    "number": number,
                    "state": state,
                    "base_ref_name": base_ref_name,
                    "merged_at": merged_at,
                    "closing_issue_numbers": [node["number"] for node in nodes],
                    "closing_issue_reopened_at": reopened_at_by_issue,
                }
            )

        if not has_next_page:
            break
        if not isinstance(end_cursor, str) or not end_cursor:
            raise CleanupError("gh GraphQL pagination omitted the next cursor")
        if end_cursor in seen_cursors:
            raise CleanupError("gh GraphQL pagination repeated a cursor")
        seen_cursors.add(end_cursor)
        cursor = end_cursor

    return result


def load_default_branch(repo: str) -> str:
    value = _json_from_gh(["api", f"repos/{repo}"])
    if not isinstance(value, dict):
        raise CleanupError("gh repository response was not an object")
    default_branch = value.get("default_branch")
    if not isinstance(default_branch, str) or not default_branch:
        raise CleanupError("gh repository response omitted the default branch")
    return default_branch


def load_issues(repo: str) -> list[dict[str, Any]]:
    issues: list[dict[str, Any]] = []
    seen: set[int] = set()
    page_number = 1

    while True:
        page = _json_from_gh(
            [
                "api",
                (
                    f"repos/{repo}/issues?state=all&per_page=100"
                    f"&page={page_number}"
                ),
            ]
        )
        if not isinstance(page, list):
            raise CleanupError("gh issues response was not a paginated issue list")
        for issue in page:
            if not isinstance(issue, dict):
                raise CleanupError("gh issues response contained an invalid issue")
            if "pull_request" in issue:
                continue
            number = _issue_number(issue)
            if number in seen:
                continue
            seen.add(number)
            issues.append(issue)

        if len(page) < 100:
            break
        page_number += 1

    return issues


def apply_action(repo: str, action: CleanupAction) -> None:
    if action.kind not in {"close-issue", "remove-labels", "return-to-ready"}:
        raise CleanupError(f"unsupported cleanup action: {action.kind}")
    if action.kind == "close-issue":
        _run_gh(
            [
                "issue",
                "close",
                str(action.issue_number),
                "--repo",
                repo,
                "--reason",
                "completed",
            ]
        )
    if action.remove_labels:
        _run_gh(
            [
                "issue",
                "edit",
                str(action.issue_number),
                "--repo",
                repo,
                *[
                    argument
                    for label in action.remove_labels
                    for argument in ("--remove-label", label)
                ],
            ]
        )
    if action.add_labels:
        _run_gh(
            [
                "issue",
                "edit",
                str(action.issue_number),
                "--repo",
                repo,
                *[
                    argument
                    for label in action.add_labels
                    for argument in ("--add-label", label)
                ],
            ]
        )


def _refresh_actions(repo: str, actions: Sequence[CleanupAction]) -> list[CleanupAction]:
    """Re-plan immediately before mutation to avoid applying a stale snapshot."""
    if not actions:
        return []
    current_issues = load_issues(repo)
    current_prs = _graphql_pages(repo)
    current_default_branch = load_default_branch(repo)
    current_by_number = {_issue_number(issue): issue for issue in current_issues}
    planned_by_number = {action.issue_number: action for action in actions}
    relevant = [
        issue
        for number, issue in current_by_number.items()
        if number in planned_by_number
    ]
    refreshed = plan_cleanup(relevant, current_prs, current_default_branch)
    return [action for action in refreshed if action.issue_number in planned_by_number]


def _validate_repo(repo: str) -> None:
    parts = repo.split("/")
    if len(parts) != 2 or any(not part for part in parts):
        raise CleanupError("--repo must have the form owner/name")


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", required=True, help="GitHub repository owner/name")
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="show cleanup actions without changing GitHub (changes apply by default)",
    )
    args = parser.parse_args(argv)

    try:
        _validate_repo(args.repo)
        issues = load_issues(args.repo)
        pull_requests = _graphql_pages(args.repo)
        default_branch = load_default_branch(args.repo)
        actions = plan_cleanup(issues, pull_requests, default_branch)
        if not args.dry_run:
            actions = _refresh_actions(args.repo, actions)
        for action in actions:
            details = []
            if action.remove_labels:
                details.append("remove " + ", ".join(action.remove_labels))
            if action.add_labels:
                details.append("add " + ", ".join(action.add_labels))
            change = "; ".join(details)
            suffix = f"; {change}" if change else ""
            verb = "would " if args.dry_run else ""
            print(f"{verb}{action.kind} issue #{action.issue_number}{suffix} ({action.reason})")
            if not args.dry_run:
                apply_action(args.repo, action)
    except (CleanupError, json.JSONDecodeError) as error:
        print(f"Stale issue cleanup FAILED: {error}", file=sys.stderr)
        return 2

    mode = "preview" if args.dry_run else "apply"
    print(f"Stale issue cleanup {mode} complete ({len(actions)} action(s)).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
