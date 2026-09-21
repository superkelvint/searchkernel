# Search Kernel Issue Autopilot userscript

`tools/searchkernel-issue-autopilot.user.js` is a Greasemonkey/Tampermonkey control loop for the GitHub-native Search Kernel work queue.

It deliberately keeps implementation and verification separate:

1. On the filtered/sorted GitHub Issues page, it takes issue order from the visible page.
2. It re-reads each candidate through the GitHub API and skips issues that are closed, assigned, explicitly excluded, carry a blocked label, already contain a claim marker, or already have an open cross-referenced PR.
3. It claims the first eligible issue by assigning the configured GitHub login and posting a durable claim comment.
4. It opens the configured Codex/ChatGPT coding-agent page and submits a task prompt containing the issue URL and the repository's required `AGENTS.md` workflow.
5. It remains serial while that issue is active.
6. When a non-draft linked PR appears, it posts one `@codex review` request for independent coding-agent verification.
7. It never merges a PR or closes an issue.

## Install

Install the userscript from:

`tools/searchkernel-issue-autopilot.user.js`

The script matches both:

- `https://github.com/superkelvint/searchkernel/issues*`
- `https://chatgpt.com/*`

The ChatGPT match exists only so the same userscript storage can hand the selected issue to the configured coding-agent page. It performs no work on unrelated ChatGPT pages.

## Configure

Open the Search Kernel Issues page, then use the userscript menu:

`Configure Search Kernel issue autopilot`

Configuration asks for:

- a fine-grained GitHub token;
- the GitHub login used to claim issues;
- the coding-agent page URL;
- issue numbers that should never be selected;
- labels that make an issue ineligible.

The default coding-agent URL is:

`https://chatgpt.com/codex`

The script starts disabled. Successful configuration enables it.

The default exclusion list contains issue `#21`, which is the issue used to develop this script. Remove it later if desired.

## GitHub token permissions

Use the narrowest token practical for `superkelvint/searchkernel`.

The userscript needs enough access to:

- read issues, comments, and issue timelines;
- assign/unassign issues;
- post issue/PR conversation comments;
- read pull requests.

The token is stored in Greasemonkey/Tampermonkey userscript storage, not in the repository.

## Queue selection

The GitHub page is the queue.

Apply ordinary GitHub issue filtering/sorting before enabling the script. The script walks issue links in visible DOM order and selects the first issue that remains eligible after API revalidation.

Default blocked labels are:

`blocked`, `do-not-work`, `in-progress`, `claimed`, `needs-design`, and `waiting`.

Eligibility is rechecked immediately before the assignment/comment mutation to reduce duplicate-agent races.

## Claim semantics

A claim is durable only after both operations succeed:

- the configured login is assigned to the issue;
- the script posts a comment containing `<!-- searchkernel-issue-autopilot-claim -->`.

If posting the claim comment fails after assignment, the script rolls the assignment back because the candidate was verified unassigned immediately before the claim.

## Coding-agent handoff

After claiming, the script stores a pending task in userscript storage and opens the configured agent URL.

On that page it finds the normal ChatGPT/Codex composer and submits a prompt requiring the agent to:

- read and follow `AGENTS.md`;
- stay on the claimed issue;
- use a dedicated task branch/worktree;
- write and observe a failing executable regression first for reported defects;
- preserve frozen contracts and architecture boundaries;
- run the issue's exact verification plus focused repository checks;
- commit, push, and open a PR to `main`;
- not merge, close, or self-declare VERIFIED/CLOSED.

The handoff is intentionally a browser bridge. If the Codex/ChatGPT composer DOM changes, the badge will move to `ERROR` rather than silently claiming dispatch succeeded.

## Verification handoff

Once the issue has an open cross-referenced PR and that PR is no longer draft, the userscript posts:

`@codex review`

with a hidden `<!-- searchkernel-issue-autopilot-review -->` marker and a request to review the full issue, regression evidence, `AGENTS.md`, frozen contracts, and architectural invariants.

It posts this at most once per active PR according to its local state and the PR conversation.

A Codex review is evidence for the existing CTO/human review process. The userscript does not interpret it as authority to merge or close work.

## Serial state machine

At most one issue is active locally:

`IDLE -> WAITING_DISPATCH -> AGENT_RUNNING -> PR_DRAFT/AWAITING_REVIEW`

The queue becomes free automatically when the active issue is closed.

The menu command `Clear local active issue` is an escape hatch. It only clears userscript state; it intentionally does not unassign the issue, remove claim comments, close PRs, or undo agent work.

## Menu commands

- Configure Search Kernel issue autopilot
- Run issue autopilot now
- Toggle issue autopilot
- Clear local active issue
- Show issue autopilot diagnostics

## Important limitations

- The coding-agent submission depends on the current Codex/ChatGPT web composer DOM.
- The configured Codex environment must already have access to `superkelvint/searchkernel`.
- Linked-PR detection relies on GitHub issue timeline cross-reference events. PRs that do not reference the issue are intentionally not guessed.
- The userscript does not merge, close, approve, or repair review findings automatically.
