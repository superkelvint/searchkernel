"""Tests for the GitHub issue queue consistency validator."""

from __future__ import annotations

import importlib.util
import unittest
from datetime import datetime, timedelta, timezone
from pathlib import Path
from unittest import mock


ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = ROOT / "tools" / "check-issue-queue.py"
SPEC = importlib.util.spec_from_file_location("check_issue_queue", MODULE_PATH)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def issue(number: int, *labels: str, title: str = "fixture") -> dict[str, object]:
    return {
        "number": number,
        "title": title,
        "labels": [{"name": label} for label in labels],
    }


class IssueQueueTests(unittest.TestCase):
    def test_canonical_queue_with_matching_claims_passes(self) -> None:
        issues = [
            issue(1, "status:ready", "priority:p1", "area:ir", "type:bug"),
            issue(2, "status:in-progress", "priority:p2", "area:ci", "type:chore"),
            issue(3, "status:blocked", "priority:p0", "area:native", "type:bug"),
            issue(4, "status:needs-followup", "priority:p2"),
        ]
        self.assertEqual(MODULE.validate_queue(issues, {"codex/issue-2"}), [])

    def test_missing_workflow_state_is_rejected(self) -> None:
        errors = MODULE.validate_queue(
            [issue(1, "priority:p1", "area:ir", "type:bug")],
            set(),
        )
        self.assertIn("expected exactly one workflow state label", errors[0])

    def test_multiple_workflow_states_are_rejected(self) -> None:
        errors = MODULE.validate_queue(
            [
                issue(
                    1,
                    "status:ready",
                    "status:in-progress",
                    "priority:p1",
                    "area:ir",
                    "type:bug",
                )
            ],
            set(),
        )
        self.assertTrue(any("exactly one workflow state" in error for error in errors))

    def test_ready_issue_with_claim_branch_is_rejected(self) -> None:
        errors = MODULE.validate_queue(
            [issue(1, "status:ready", "priority:p1", "area:ir", "type:bug")],
            {"codex/issue-1"},
        )
        self.assertTrue(any("active claim branch" in error for error in errors))

    def test_in_progress_issue_without_claim_branch_is_rejected(self) -> None:
        errors = MODULE.validate_queue(
            [issue(1, "status:in-progress", "priority:p1", "area:ir", "type:bug")],
            set(),
        )
        self.assertTrue(any("lacks claim branch" in error for error in errors))

    def test_actionable_issue_requires_priority_area_and_type(self) -> None:
        errors = MODULE.validate_queue([issue(1, "status:ready")], set())
        self.assertTrue(any("priority label" in error for error in errors))
        self.assertTrue(any("area:*" in error for error in errors))
        self.assertTrue(any("type:*" in error for error in errors))

    def test_actionable_issue_requires_exactly_one_priority(self) -> None:
        errors = MODULE.validate_queue(
            [
                issue(
                    1,
                    "status:ready",
                    "priority:p1",
                    "priority:p2",
                    "area:ir",
                    "type:bug",
                )
            ],
            set(),
        )
        self.assertTrue(any("exactly one priority" in error for error in errors))

    def test_legacy_workflow_labels_are_rejected(self) -> None:
        errors = MODULE.validate_queue(
            [
                issue(
                    1,
                    "status:ready",
                    "in-progress",
                    "priority:p1",
                    "area:ir",
                    "type:bug",
                )
            ],
            set(),
        )
        self.assertTrue(any("legacy workflow label" in error for error in errors))

    def test_duplicate_issue_numbers_are_rejected(self) -> None:
        fixture = issue(1, "status:ready", "priority:p1", "area:ir", "type:bug")
        errors = MODULE.validate_queue([fixture, fixture], set())
        self.assertTrue(any("duplicate issue" in error for error in errors))

    def test_closed_issues_must_not_keep_workflow_state_labels(self) -> None:
        for state in sorted(MODULE.WORKFLOW_STATES):
            with self.subTest(state=state):
                errors = MODULE.validate_closed_issue_labels([issue(42, state)])
                self.assertEqual(
                    errors,
                    [f"issue #42 (fixture): closed issue retains workflow state label {state}"],
                )

    def test_closed_issue_without_workflow_state_passes(self) -> None:
        self.assertEqual(
            MODULE.validate_closed_issue_labels(
                [issue(42, "priority:p1", "area:ci", "type:bug")]
            ),
            [],
        )


    def test_load_closed_issues_deduplicates_pagination_overlap(self) -> None:
        closed_issue = issue(42, "priority:p1")
        first_page = [closed_issue]
        first_page.extend(
            {
                "number": 1000 + index,
                "title": "pull request fixture",
                "labels": [],
                "pull_request": {"url": "https://example.invalid/pulls/fixture"},
            }
            for index in range(99)
        )
        second_page = [closed_issue]

        with mock.patch.object(
            MODULE,
            "_github_json",
            side_effect=[first_page, second_page],
        ) as github_json:
            self.assertEqual(MODULE.load_closed_issues("owner/repo"), [closed_issue])

        self.assertEqual(
            github_json.call_args_list,
            [
                mock.call("owner/repo", "issues", page=1, state="closed"),
                mock.call("owner/repo", "issues", page=2, state="closed"),
            ],
        )

    def test_load_closed_issues_requests_closed_state_and_ignores_pull_requests(self) -> None:
        closed_issue = issue(42, "priority:p1")
        pull_request = {
            "number": 99,
            "title": "not an issue",
            "labels": [],
            "pull_request": {"url": "https://example.invalid/pulls/99"},
        }
        with mock.patch.object(
            MODULE,
            "_github_json",
            return_value=[closed_issue, pull_request],
        ) as github_json:
            self.assertEqual(MODULE.load_closed_issues("owner/repo"), [closed_issue])
        github_json.assert_called_once_with(
            "owner/repo",
            "issues",
            page=1,
            state="closed",
        )



class IssueQueueReconciliationTests(unittest.TestCase):
    NOW = datetime(2026, 9, 26, 12, 0, tzinfo=timezone.utc)
    STALE_AFTER = timedelta(hours=24)

    def _open_issue(self, number: int, *labels: str, updated_hours_ago: int) -> dict[str, object]:
        fixture = issue(number, *labels)
        fixture["updated_at"] = (
            self.NOW - timedelta(hours=updated_hours_ago)
        ).isoformat().replace("+00:00", "Z")
        return fixture

    def test_closed_issue_workflow_labels_are_removed(self) -> None:
        closed = issue(
            42,
            "status:in-progress",
            "priority:p1",
            "area:ci",
            "type:hardening",
        )
        actions = MODULE.plan_reconciliation(
            [],
            [closed],
            set(),
            set(),
            {},
            now=self.NOW,
            stale_after=self.STALE_AFTER,
        )
        self.assertEqual(
            [(action.kind, action.issue_number, action.value) for action in actions],
            [("remove-workflow-states", 42, None)],
        )

    def test_fresh_in_progress_claim_is_left_alone(self) -> None:
        current = self._open_issue(
            7,
            "status:in-progress",
            "priority:p1",
            "area:ci",
            "type:hardening",
            updated_hours_ago=1,
        )
        actions = MODULE.plan_reconciliation(
            [current],
            [],
            {"codex/issue-7"},
            set(),
            {"codex/issue-7": 0},
            now=self.NOW,
            stale_after=self.STALE_AFTER,
        )
        self.assertEqual(actions, [])

    def test_stale_in_progress_without_claim_is_requeued(self) -> None:
        stale = self._open_issue(
            8,
            "status:in-progress",
            "priority:p1",
            "area:ci",
            "type:hardening",
            updated_hours_ago=48,
        )
        actions = MODULE.plan_reconciliation(
            [stale],
            [],
            set(),
            set(),
            {},
            now=self.NOW,
            stale_after=self.STALE_AFTER,
        )
        self.assertEqual(
            [(action.kind, action.issue_number, action.value) for action in actions],
            [("set-state", 8, "status:ready")],
        )

    def test_stale_claim_branch_is_preserved_and_blocked(self) -> None:
        stale = self._open_issue(
            9,
            "status:in-progress",
            "priority:p1",
            "area:ci",
            "type:hardening",
            updated_hours_ago=48,
        )
        actions = MODULE.plan_reconciliation(
            [stale],
            [],
            {"codex/issue-9"},
            set(),
            {"codex/issue-9": 0},
            now=self.NOW,
            stale_after=self.STALE_AFTER,
        )
        self.assertEqual(
            [(action.kind, action.issue_number, action.value) for action in actions],
            [("set-state", 9, "status:blocked")],
        )

    def test_stale_claim_with_unique_commits_is_preserved_and_blocked(self) -> None:
        stale = self._open_issue(
            10,
            "status:in-progress",
            "priority:p1",
            "area:ci",
            "type:hardening",
            updated_hours_ago=48,
        )
        actions = MODULE.plan_reconciliation(
            [stale],
            [],
            {"codex/issue-10"},
            set(),
            {"codex/issue-10": 3},
            now=self.NOW,
            stale_after=self.STALE_AFTER,
        )
        self.assertEqual(
            [(action.kind, action.issue_number, action.value) for action in actions],
            [("set-state", 10, "status:blocked")],
        )

    def test_open_pr_keeps_stale_claim_in_progress(self) -> None:
        stale = self._open_issue(
            11,
            "status:in-progress",
            "priority:p1",
            "area:ci",
            "type:hardening",
            updated_hours_ago=72,
        )
        actions = MODULE.plan_reconciliation(
            [stale],
            [],
            {"codex/issue-11"},
            {"codex/issue-11"},
            {"codex/issue-11": 2},
            now=self.NOW,
            stale_after=self.STALE_AFTER,
        )
        self.assertEqual(actions, [])

    def test_multiple_open_workflow_states_remain_fail_closed(self) -> None:
        stale = self._open_issue(
            12,
            "status:ready",
            "status:in-progress",
            "priority:p1",
            "area:ci",
            "type:hardening",
            updated_hours_ago=72,
        )
        actions = MODULE.plan_reconciliation(
            [stale],
            [],
            {"codex/issue-12"},
            set(),
            {"codex/issue-12": 0},
            now=self.NOW,
            stale_after=self.STALE_AFTER,
        )
        self.assertEqual(actions, [])


if __name__ == "__main__":
    unittest.main()
