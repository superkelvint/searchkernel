#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$ROOT"
source "$ROOT/tools/client-e2e-runtime.sh"
prepare_client_e2e_runtime

command -v ruby >/dev/null 2>&1 || {
  echo "Ruby package-mode E2E requires ruby" >&2
  exit 1
}

PACKAGE_DIR="$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-ruby-package-daemon-package.XXXXXX")"
GEM_HOME_DIR="$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-ruby-package-daemon-gem-home.XXXXXX")"
cleanup() {
  rm -rf -- "$PACKAGE_DIR" "$GEM_HOME_DIR"
}
trap cleanup EXIT INT TERM

(
  cd "$ROOT/clients/ruby"
  ruby -S gem build searchkernel.gemspec --output "$PACKAGE_DIR/searchkernel.gem" >/dev/null
)

GEM_HOME="$GEM_HOME_DIR" GEM_PATH="$GEM_HOME_DIR" \
  ruby -S gem install --local --install-dir "$GEM_HOME_DIR" \
    "$PACKAGE_DIR/searchkernel.gem" --no-document >/dev/null

GEM_HOME="$GEM_HOME_DIR" \
GEM_PATH="$GEM_HOME_DIR" \
RUBYLIB= \
RUBYOPT= \
  ruby "$ROOT/clients/ruby/examples/try_searchkernel_package_daemon_e2e.rb"
