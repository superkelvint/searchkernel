#!/usr/bin/env bash
set -euo pipefail

if [[ -z "${RUNNER_TEMP:-}" || -z "${GITHUB_PATH:-}" || -z "${GITHUB_ENV:-}" ]]; then
  echo "::error::select-python.sh must run inside GitHub Actions."
  exit 1
fi

selected=""
for candidate in python3 python3.13 python3.10; do
  if ! command -v "$candidate" >/dev/null 2>&1; then
    continue
  fi

  version="$("$candidate" -c 'import sys; print(f"{sys.version_info.major}.{sys.version_info.minor}")')"
  case "$version" in
    3.10|3.13)
      selected="$(command -v "$candidate")"
      break
      ;;
  esac
done

if [[ -z "$selected" ]]; then
  echo "::error::The self-hosted runner must provide Python 3.10 or Python 3.13 (python3.10, python3.13, or python3 resolving to one of them)."
  exit 1
fi

shim_dir="$RUNNER_TEMP/searchkernel-python"
mkdir -p "$shim_dir"
ln -sfn "$selected" "$shim_dir/python"
ln -sfn "$selected" "$shim_dir/python3"

echo "$shim_dir" >> "$GITHUB_PATH"
echo "CI_PYTHON=$selected" >> "$GITHUB_ENV"

"$selected" --version
