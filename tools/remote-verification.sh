#!/usr/bin/env bash
set -euo pipefail

usage() {
    echo "usage: $0 {test|verify} <target>" >&2
    exit 2
}

[[ "$#" -eq 2 ]] || usage
operation=$1
target=$2

case "$operation" in
    test|verify) ;;
    *) usage ;;
esac

for tool in git gh; do
    command -v "$tool" >/dev/null 2>&1 || {
        echo "error: remote verification requires $tool on the host" >&2
        exit 2
    }
done

root=$(git rev-parse --show-toplevel 2>/dev/null) || {
    echo "error: remote verification must run from a Search Kernel Git checkout" >&2
    exit 2
}
cd "$root"

if [[ -n "$(git status --porcelain --untracked-files=normal)" ]]; then
    echo "error: remote verification requires a clean worktree and index" >&2
    echo "commit or stash local changes before dispatching exact-SHA evidence" >&2
    exit 2
fi

revision=$(git rev-parse HEAD)
if [[ ! "$revision" =~ ^[0-9a-f]{40}$ ]]; then
    echo "error: could not resolve HEAD to an exact commit SHA" >&2
    exit 2
fi

repo=$(gh repo view --json nameWithOwner --jq '.nameWithOwner')
default_branch=$(gh repo view --json defaultBranchRef --jq '.defaultBranchRef.name')
if [[ -z "$repo" || -z "$default_branch" ]]; then
    echo "error: could not resolve GitHub repository/default branch" >&2
    exit 2
fi

if ! gh api "repos/$repo/commits/$revision" --silent >/dev/null 2>&1; then
    echo "error: HEAD $revision is not available from GitHub; push it before remote verification" >&2
    exit 2
fi

request_id="remote-${revision:0:12}-$(date -u +%Y%m%dT%H%M%SZ)-$$"
run_title="$operation $target @ $revision [$request_id]"

echo "Dispatching exact-SHA remote verification"
echo "  revision: $revision"
echo "  operation: $operation"
echo "  target: $target"

gh workflow run on-demand-verification.yml \
    --repo "$repo" \
    --ref "$default_branch" \
    -f "revision=$revision" \
    -f "operation=$operation" \
    -f "target=$target" \
    -f "request_id=$request_id"

run_id=
for ((attempt = 0; attempt < 30; attempt += 1)); do
    while IFS=$'\t' read -r candidate_id candidate_title; do
        if [[ "$candidate_title" == *"[$request_id]"* ]]; then
            run_id=$candidate_id
            break
        fi
    done < <(
        gh run list \
            --repo "$repo" \
            --workflow on-demand-verification.yml \
            --event workflow_dispatch \
            --limit 100 \
            --json databaseId,displayTitle \
            --jq '.[] | [.databaseId, .displayTitle] | @tsv'
    )
    [[ -n "$run_id" ]] && break
    sleep 2
done

if [[ -z "$run_id" ]]; then
    echo "error: dispatched workflow was not discoverable for request_id=$request_id" >&2
    exit 1
fi

run_url=$(gh run view "$run_id" --repo "$repo" --json url --jq '.url')
echo "Remote run: $run_url"

set +e
gh run watch "$run_id" --repo "$repo" --exit-status
status=$?
set -e

if [[ "$status" -ne 0 ]]; then
    echo
    echo "Remote verification failed; failed-step logs follow:" >&2
    gh run view "$run_id" --repo "$repo" --log-failed >&2 || true
fi

exit "$status"
