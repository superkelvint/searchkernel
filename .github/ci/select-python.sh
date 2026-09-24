#!/usr/bin/env bash
set -euo pipefail

if [[ -z "${RUNNER_TEMP:-}" || -z "${GITHUB_PATH:-}" || -z "${GITHUB_ENV:-}" ]]; then
  echo "::error::select-python.sh must run inside GitHub Actions."
  exit 1
fi

supported_python() {
  local candidate=$1 version
  [[ -x "$candidate" ]] || return 1
  version="$("$candidate" -c 'import sys; print(f"{sys.version_info.major}.{sys.version_info.minor}")' 2>/dev/null)" || return 1
  [[ "$version" == "3.10" || "$version" == "3.13" ]]
}

isolated_python() {
  local candidate=$1
  supported_python "$candidate" || return 1
  "$candidate" -c 'import sys; raise SystemExit(0 if sys.prefix != sys.base_prefix else 1)' >/dev/null 2>&1
}

selected=""
if [[ -n "${CI_PYTHON:-}" ]] && isolated_python "$CI_PYTHON"; then
  selected="$CI_PYTHON"
fi

if [[ -z "$selected" ]]; then
  base=""
  for candidate in python3 python3.13 python3.10; do
    resolved="$(command -v "$candidate" 2>/dev/null || true)"
    if [[ -n "$resolved" ]] && supported_python "$resolved"; then
      base="$resolved"
      break
    fi
  done

  if [[ -z "$base" ]]; then
    echo "::error::The self-hosted runner must provide Python 3.10 or Python 3.13."
    exit 1
  fi

  venv_dir="$RUNNER_TEMP/searchkernel-python-venv"
  rm -rf "$venv_dir"
  if ! "$base" -m venv "$venv_dir"; then
    echo "::notice::Python venv support is missing; attempting one-time runner migration."
    sudo -n apt-get update
    sudo -n apt-get install -y --no-install-recommends python3-venv
    rm -rf "$venv_dir"
    "$base" -m venv "$venv_dir"
  fi
  selected="$venv_dir/bin/python"
fi

if ! isolated_python "$selected"; then
  echo "::error::Selected CI Python must be a supported isolated virtual environment: $selected"
  exit 1
fi
"$selected" -m pip --version >/dev/null

shim_dir="$RUNNER_TEMP/searchkernel-python"
mkdir -p "$shim_dir"
ln -sfn "$selected" "$shim_dir/python"
ln -sfn "$selected" "$shim_dir/python3"

echo "$shim_dir" >> "$GITHUB_PATH"
echo "CI_PYTHON=$selected" >> "$GITHUB_ENV"

"$selected" --version
echo "Selected isolated CI Python: $selected"
