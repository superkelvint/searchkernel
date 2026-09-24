#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
SearchKernel self-hosted GitHub Actions host bootstrap (Ubuntu/Debian x86_64)

Required:
  sudo ./install.sh --repo https://github.com/superkelvint/searchkernel --token <registration-token>

Optional:
  --name NAME                 Runner name (default: hostname-searchkernel)
  --labels CSV                Runner labels
  --runner-user USER          Service account (default: actions-runner)
  --runner-home PATH          Runner installation root (default: /opt/actions-runner)
  --config FILE               Load defaults from an env file
  -h, --help                  Show this help

The registration token is accepted only at runtime.
Native Vespa build/test dependencies are provisioned later inside the persistent
AlmaLinux container by tools/actions-runner-setup.sh from a checked-out job.
USAGE
}

for arg in "$@"; do
  case "$arg" in
    -h|--help) usage; exit 0 ;;
  esac
done

if [[ ${EUID} -ne 0 ]]; then
  echo "error: run as root (for example: sudo ./install.sh ...)" >&2
  exit 2
fi

CONFIG_FILE=""
ARGS=("$@")
for ((i=0; i<${#ARGS[@]}; i++)); do
  if [[ "${ARGS[$i]}" == "--config" ]]; then
    CONFIG_FILE="${ARGS[$((i+1))]:-}"
  fi
done
if [[ -n "$CONFIG_FILE" ]]; then
  [[ -f "$CONFIG_FILE" ]] || { echo "error: config file not found: $CONFIG_FILE" >&2; exit 2; }
  # shellcheck disable=SC1090
  set -a
  source "$CONFIG_FILE"
  set +a
fi

GITHUB_REPO_URL=${GITHUB_REPO_URL:-}
RUNNER_TOKEN=${RUNNER_TOKEN:-}
RUNNER_NAME=${RUNNER_NAME:-"$(hostname -s)-searchkernel"}
RUNNER_LABELS=${RUNNER_LABELS:-"self-hosted,linux,x64,searchkernel,native"}
RUNNER_USER=${RUNNER_USER:-actions-runner}
RUNNER_HOME=${RUNNER_HOME:-/opt/actions-runner}
RUNNER_WORK_DIR=${RUNNER_WORK_DIR:-_work}
SEARCHKERNEL_STATE_ROOT=${SEARCHKERNEL_STATE_ROOT:-/var/lib/searchkernel-ci}
SEARCHKERNEL_CACHE_ROOT=${SEARCHKERNEL_CACHE_ROOT:-/var/cache/searchkernel}
ACTIONS_RUNNER_VERSION=${ACTIONS_RUNNER_VERSION:-2.337.0}
FLATBUFFERS_VERSION=${FLATBUFFERS_VERSION:-25.12.19}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --repo) GITHUB_REPO_URL="$2"; shift 2 ;;
    --token) RUNNER_TOKEN="$2"; shift 2 ;;
    --name) RUNNER_NAME="$2"; shift 2 ;;
    --labels) RUNNER_LABELS="$2"; shift 2 ;;
    --runner-user) RUNNER_USER="$2"; shift 2 ;;
    --runner-home) RUNNER_HOME="$2"; shift 2 ;;
    --config) shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "error: unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
done

if [[ -z "$GITHUB_REPO_URL" || -z "$RUNNER_TOKEN" ]]; then
  echo "error: --repo and --token are required" >&2
  usage >&2
  exit 2
fi

case "$(uname -m)" in
  x86_64|amd64) ARCH=x64 ;;
  *) echo "error: this bundle currently supports Linux x86_64 only" >&2; exit 2 ;;
esac

if ! grep -qiE 'ubuntu|debian' /etc/os-release; then
  echo "error: supported bootstrap target is Ubuntu/Debian" >&2
  exit 2
fi

export DEBIAN_FRONTEND=noninteractive

apt-get update
apt-get install -y --no-install-recommends   ca-certificates curl git gh jq sudo tar gzip unzip zip file procps time   docker.io   build-essential cmake ninja-build pkg-config   clang llvm lld libclang-dev python3-clang   python3 python3-venv python3-pip python3-dev   php-cli

systemctl enable --now docker

if ! command -v node >/dev/null 2>&1 || ! node -e '
const [major, minor] = process.versions.node.split(".").map(Number);
process.exit(major > 20 || (major === 20 && minor >= 19) ? 0 : 1);
' >/dev/null 2>&1; then
  curl -fsSL https://deb.nodesource.com/setup_20.x | bash -
  apt-get install -y --no-install-recommends nodejs
fi

node -e '
const [major, minor] = process.versions.node.split(".").map(Number);
if (!(major > 20 || (major === 20 && minor >= 19))) {
  throw new Error("Node.js >=20.19 is required");
}
'

if ! id "$RUNNER_USER" >/dev/null 2>&1; then
  useradd --create-home --shell /bin/bash "$RUNNER_USER"
fi

RUNNER_USER_HOME=$(getent passwd "$RUNNER_USER" | cut -d: -f6)
usermod -aG docker "$RUNNER_USER"

install -d -o "$RUNNER_USER" -g "$RUNNER_USER"   "$RUNNER_HOME"   "$SEARCHKERNEL_STATE_ROOT"   "$SEARCHKERNEL_STATE_ROOT/tmp"   "$SEARCHKERNEL_CACHE_ROOT"   "$SEARCHKERNEL_CACHE_ROOT/native-artifacts"

run_user() {
  sudo -u "$RUNNER_USER" -H env     HOME="$RUNNER_USER_HOME"     XDG_CACHE_HOME="$SEARCHKERNEL_CACHE_ROOT"     SEARCHKERNEL_ARTIFACT_CACHE="$SEARCHKERNEL_CACHE_ROOT/native-artifacts"     "$@"
}

if [[ ! -x "$RUNNER_USER_HOME/.cargo/bin/rustup" ]]; then
  run_user bash -lc 'curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal'
fi
run_user bash -lc 'source "$HOME/.cargo/env"; rustup toolchain install stable --profile minimal --no-self-update; rustup default stable; rustup component add rustfmt clippy'

flatbuffers_ok() {
  [[ -x /usr/local/bin/flatc ]] &&
  [[ "$(/usr/local/bin/flatc --version 2>/dev/null || true)" == "flatc version $FLATBUFFERS_VERSION" ]] &&
  [[ -f /usr/local/include/flatbuffers/base.h ]] &&
  [[ -f /usr/local/include/flatbuffers/idl.h ]] &&
  [[ -f /usr/local/lib/libflatbuffers.a ]]
}

if ! flatbuffers_ok; then
  tmp=$(mktemp -d /tmp/searchkernel-flatbuffers.XXXXXX)
  trap 'rm -rf "$tmp"' EXIT
  curl --fail --location --silent --show-error     "https://github.com/google/flatbuffers/archive/refs/tags/v${FLATBUFFERS_VERSION}.tar.gz"     -o "$tmp/flatbuffers.tar.gz"
  mkdir -p "$tmp/source"
  tar -xzf "$tmp/flatbuffers.tar.gz" --strip-components=1 -C "$tmp/source"
  cmake     -S "$tmp/source"     -B "$tmp/build"     -DCMAKE_BUILD_TYPE=Release     -DCMAKE_INSTALL_PREFIX=/usr/local     -DCMAKE_INSTALL_LIBDIR=lib     -DCMAKE_POSITION_INDEPENDENT_CODE=ON     -DFLATBUFFERS_BUILD_TESTS=OFF
  cmake --build "$tmp/build" --parallel "$(nproc)"
  cmake --install "$tmp/build"
  rm -rf "$tmp"
  trap - EXIT
fi

flatbuffers_ok || { echo "error: FlatBuffers $FLATBUFFERS_VERSION installation failed" >&2; exit 1; }

cd "$RUNNER_HOME"
RUNNER_TARBALL="actions-runner-linux-${ARCH}-${ACTIONS_RUNNER_VERSION}.tar.gz"
if [[ ! -x "$RUNNER_HOME/config.sh" ]]; then
  curl -fL --retry 5     "https://github.com/actions/runner/releases/download/v${ACTIONS_RUNNER_VERSION}/${RUNNER_TARBALL}"     -o "/tmp/${RUNNER_TARBALL}"
  tar xzf "/tmp/${RUNNER_TARBALL}" -C "$RUNNER_HOME"
  rm -f "/tmp/${RUNNER_TARBALL}"
  chown -R "$RUNNER_USER:$RUNNER_USER" "$RUNNER_HOME"
  "$RUNNER_HOME/bin/installdependencies.sh"
fi

if [[ -x "$RUNNER_HOME/svc.sh" ]]; then
  "$RUNNER_HOME/svc.sh" stop >/dev/null 2>&1 || true
  "$RUNNER_HOME/svc.sh" uninstall >/dev/null 2>&1 || true
fi

if [[ ! -f "$RUNNER_HOME/.runner" ]]; then
  run_user bash -lc "cd '$RUNNER_HOME'; ./config.sh --unattended --replace --url '$GITHUB_REPO_URL' --token '$RUNNER_TOKEN' --name '$RUNNER_NAME' --labels '$RUNNER_LABELS' --work '$RUNNER_WORK_DIR'"
else
  echo "info: runner is already registered; preserving existing registration"
fi

ENV_FILE="$RUNNER_HOME/.env"
touch "$ENV_FILE"
chown "$RUNNER_USER:$RUNNER_USER" "$ENV_FILE"

append_env() {
  local key=$1 value=$2
  sed -i "/^${key}=/d" "$ENV_FILE"
  printf '%s=%s\n' "$key" "$value" >> "$ENV_FILE"
}

append_env PATH "$RUNNER_USER_HOME/.cargo/bin:/usr/local/bin:/usr/local/sbin:/usr/bin:/usr/sbin:/bin:/sbin"
append_env HOME "$RUNNER_USER_HOME"
append_env XDG_CACHE_HOME "$SEARCHKERNEL_CACHE_ROOT"
append_env SEARCHKERNEL_ARTIFACT_CACHE "$SEARCHKERNEL_CACHE_ROOT/native-artifacts"
append_env TMPDIR "$SEARCHKERNEL_STATE_ROOT/tmp"

cd "$RUNNER_HOME"
./svc.sh install "$RUNNER_USER"
./svc.sh start

run_user bash -lc '
  source "$HOME/.cargo/env"
  rustc --version
  cargo --version
  rustfmt --version
  cargo clippy --version
  docker info >/dev/null
  gh --version
  python3 --version
  node --version
  npm --version
  php --version | head -n 1
  flatc --version
'

cat <<EOF2

SearchKernel Actions host runner installed and started.
  repo:           $GITHUB_REPO_URL
  runner:         $RUNNER_NAME
  labels:         $RUNNER_LABELS
  service dir:    $RUNNER_HOME
  cache root:     $SEARCHKERNEL_CACHE_ROOT
  artifact cache: $SEARCHKERNEL_CACHE_ROOT/native-artifacts

Native Vespa setup is intentionally deferred to repository workflow code:
  tools/actions-runner-setup.sh
EOF2
