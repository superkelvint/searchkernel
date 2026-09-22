#!/usr/bin/env bash
set -euo pipefail

matches="$(
  grep -REn 'actions/setup-python@|^[[:space:]]*python-version:' .github/workflows || true
)"

if [[ -n "$matches" ]]; then
  printf '%s\n' "$matches"
  echo "::error::GitHub workflows must use the runner-provided Python selected by .github/ci/select-python.sh; do not use actions/setup-python or python-version."
  exit 1
fi
