"""Tests for the stale issue cleanup planner and gh command boundary."""

from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path
from unittest import mock


ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = ROOT / "tools" / "cleanup-stale-issues.py"
SPEC = importlib.util.spec_from_file_location("cleanup_stale_issues", MODULE_PATH)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def issue(number: int, state: str = "open", *labels: str) -> dict[str, object]:
    return {
        "number": number,
        "state": state,
        "labels": [{"name": label} for label in labels],
    }


def pr(number: int, state: str, *closing_issues: int) -> dict[str, object]:
    return {
        "number": number,
        "state": state,
        "closing_issue_numbers": list(closing_issues),
    }


class CleanupPlannerTests(unittest.TestCase):
    def test_merged_closing_pr_closes_open_issue_and_removes_workflow_labels(self) -> None:
        actions = MODULE.plan_cleanup(
            [issue(12, "open", "status:in-progress", "priority:p1")],
            [pr(34, "MERGED", 12)],
        )
        self.assertEqual(
            actions,
            [
                MODULE.CleanupAction(
                    "close-issue",
                    12,
                    remove_labels=("status:in-progress",),
                    reason="all linked closing-reference PRs are terminal and one was merged",
                )
            ],
        )

    def test_closed_unmerged_closing_pr_returns_in_progress_issue_to_ready(self) -> None:
        actions = MODULE.plan_cleanup(
            [issue(12, "open", "status:in-progress", "in-progress")],
            [pr(34, "CLOSED", 12)],
        )
        self.assertEqual(
            actions,
            [
                MODULE.CleanupAction(
                    "return-to-ready",
                    12,
                    remove_labels=("in-progress", "status:in-progress"),
                    add_labels=("status:ready",),
                    reason="all linked closing-reference PRs closed without a merge",
                )
            ],
        )

    def test_open_linked_pr_keeps_issue_untouched(self) -> None:
        actions = MODULE.plan_cleanup(
            [issue(12, "open", "status:in-progress")],
            [pr(34, "CLOSED", 12), pr(35, "OPEN", 12)],
        )
        self.assertEqual(actions, [])

    def test_merged_pr_does_not_close_while_another_closing_pr_is_open(self) -> None:
        actions = MODULE.plan_cleanup(
            [issue(12, "open", "status:in-progress")],
            [pr(34, "MERGED", 12), pr(35, "OPEN", 12)],
        )
        self.assertEqual(actions, [])

    def test_issue_without_in_progress_label_is_not_requeued(self) -> None:
        actions = MODULE.plan_cleanup(
            [issue(12, "open", "status:ready")],
            [pr(34, "CLOSED", 12)],
        )
        self.assertEqual(actions, [])

    def test_closed_issue_sheds_workflow_labels(self) -> None:
        actions = MODULE.plan_cleanup(
            [issue(12, "closed", "status:in-progress", "priority:p1")],
            [],
        )
        self.assertEqual(
            actions,
            [
                MODULE.CleanupAction(
                    "remove-labels",
                    12,
                    remove_labels=("status:in-progress",),
                    reason="closed issues do not participate in the workflow queue",
                )
            ],
        )

    def test_issue_with_multiple_terminal_prs_closes_when_any_was_merged(self) -> None:
        actions = MODULE.plan_cleanup(
            [issue(12, "open")],
            [pr(34, "MERGED", 12), pr(35, "CLOSED", 12)],
        )
        self.assertEqual([action.kind for action in actions], ["close-issue"])


class GhMutationTests(unittest.TestCase):
    def test_return_to_ready_removes_before_adding_state_label(self) -> None:
        action = MODULE.CleanupAction(
            "return-to-ready",
            12,
            remove_labels=("status:in-progress",),
            add_labels=("status:ready",),
        )
        with mock.patch.object(MODULE, "_run_gh") as run_gh:
            MODULE.apply_action("owner/repo", action)
        self.assertEqual(
            run_gh.call_args_list,
            [
                mock.call(
                    [
                        "issue",
                        "edit",
                        "12",
                        "--repo",
                        "owner/repo",
                        "--remove-label",
                        "status:in-progress",
                    ]
                ),
                mock.call(
                    [
                        "issue",
                        "edit",
                        "12",
                        "--repo",
                        "owner/repo",
                        "--add-label",
                        "status:ready",
                    ]
                ),
            ],
        )

    def test_merged_issue_is_closed_before_labels_are_removed(self) -> None:
        action = MODULE.CleanupAction(
            "close-issue",
            12,
            remove_labels=("status:in-progress",),
        )
        with mock.patch.object(MODULE, "_run_gh") as run_gh:
            MODULE.apply_action("owner/repo", action)
        self.assertEqual(run_gh.call_args_list[0], mock.call([
            "issue", "close", "12", "--repo", "owner/repo", "--reason", "completed"
        ]))
        self.assertIn("--remove-label", run_gh.call_args_list[1].args[0])

    def test_dry_run_does_not_apply_mutations(self) -> None:
        with (
            mock.patch.object(MODULE, "load_issues", return_value=[issue(12)]),
            mock.patch.object(MODULE, "_graphql_pages", return_value=[pr(34, "MERGED", 12)]),
            mock.patch.object(MODULE, "apply_action") as apply_action,
            mock.patch("builtins.print"),
        ):
            self.assertEqual(
                MODULE.main(["--repo", "owner/repo", "--dry-run"]),
                0,
            )
        apply_action.assert_not_called()

    def test_default_mode_applies_mutations(self) -> None:
        with (
            mock.patch.object(MODULE, "load_issues", return_value=[issue(12)]),
            mock.patch.object(MODULE, "_graphql_pages", return_value=[pr(34, "MERGED", 12)]),
            mock.patch.object(
                MODULE,
                "_refresh_actions",
                return_value=[MODULE.CleanupAction("close-issue", 12, reason="merged")],
            ),
            mock.patch.object(MODULE, "apply_action") as apply_action,
            mock.patch("builtins.print"),
        ):
            self.assertEqual(MODULE.main(["--repo", "owner/repo"]), 0)
        apply_action.assert_called_once()


if __name__ == "__main__":
    unittest.main()
