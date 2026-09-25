#!/usr/bin/env bash
set -euo pipefail

RUNNER_USER=${RUNNER_USER:-actions-runner}
RUNNER_HOME=${RUNNER_HOME:-/opt/actions-runner}
SEARCHKERNEL_CACHE_ROOT=${SEARCHKERNEL_CACHE_ROOT:-/var/cache/searchkernel}
FLATBUFFERS_VERSION=${FLATBUFFERS_VERSION:-25.12.19}
RUBY_VERSION=${RUBY_VERSION:-3.3.6}
RUBY_BIN_DIR="$SEARCHKERNEL_CACHE_ROOT/searchkernel/ruby/$RUBY_VERSION/bin"

RUNNER_USER_HOME=$(getent passwd "$RUNNER_USER" | cut -d: -f6)

sudo -u "$RUNNER_USER" -H env   HOME="$RUNNER_USER_HOME"   XDG_CACHE_HOME="$SEARCHKERNEL_CACHE_ROOT"   SEARCHKERNEL_ARTIFACT_CACHE="$SEARCHKERNEL_CACHE_ROOT/native-artifacts"   PATH="$RUBY_BIN_DIR:$RUNNER_USER_HOME/.cargo/bin:/usr/local/bin:/usr/local/sbin:/usr/bin:/usr/sbin:/bin:/sbin"   bash -lc '
set -euo pipefail
source "$HOME/.cargo/env"
rustc --version
cargo --version
rustfmt --version
cargo clippy --version
docker info >/dev/null
gh --version
python3 --version
node -e '"'"'
const [major, minor] = process.versions.node.split(".").map(Number);
process.exit(major > 20 || (major === 20 && minor >= 19) ? 0 : 1);
'"'"'
node --version
npm --version
php -r '"'"'exit(version_compare(PHP_VERSION, "8.1.0", ">=") ? 0 : 1);'"'"'
php --version | head -n 1
ruby -rrubygems -e '"'"'abort "Ruby >= 3.1 is required" if Gem::Version.new(RUBY_VERSION) < Gem::Version.new("3.1")'"'"'
ruby --version
gem --version
flatc --version
'

[[ "$(/usr/local/bin/flatc --version)" == "flatc version $FLATBUFFERS_VERSION" ]]
[[ -f /usr/local/include/flatbuffers/base.h ]]
[[ -f /usr/local/include/flatbuffers/idl.h ]]
[[ -f /usr/local/lib/libflatbuffers.a ]]
[[ -d "$SEARCHKERNEL_CACHE_ROOT/native-artifacts" ]]
[[ -x "$RUNNER_HOME/svc.sh" ]]

systemctl --no-pager --full --type=service | grep 'actions.runner'
echo "SearchKernel host runner bootstrap verification PASS"
