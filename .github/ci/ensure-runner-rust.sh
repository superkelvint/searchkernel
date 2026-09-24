#!/usr/bin/env bash
set -euo pipefail

# Self-heal the Rust toolchain on persistent self-hosted runners.
#
# All runner services on a host share the same user-level Cargo/rustup homes,
# so every mutation is serialized with a host-wide lock. Healthy toolchains
# take the fast path and perform no mutation.
#
# Optional arguments are rustup components required by the caller, e.g.:
#   .github/ci/ensure-runner-rust.sh rustfmt clippy

CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"
export CARGO_HOME RUSTUP_HOME
export PATH="$CARGO_HOME/bin:$PATH"

mkdir -p "$CARGO_HOME/bin" "$RUSTUP_HOME"

LOCK_FILE="${SEARCHKERNEL_RUST_LOCK:-/tmp/searchkernel-rust-toolchain.lock}"
exec 9>"$LOCK_FILE"
flock 9

rustup_bin="$CARGO_HOME/bin/rustup"

bootstrap_rustup() {
    echo "Rustup is missing; bootstrapping it under the shared toolchain lock."
    curl --proto '=https' --tlsv1.2 --retry 5 --retry-connrefused         --silent --show-error --fail https://sh.rustup.rs |
        sh -s -- -y --profile minimal --default-toolchain none --no-modify-path
}

if [[ ! -x "$rustup_bin" ]]; then
    if command -v rustup >/dev/null 2>&1; then
        rustup_bin="$(command -v rustup)"
    else
        bootstrap_rustup
        rustup_bin="$CARGO_HOME/bin/rustup"
    fi
fi

[[ -x "$rustup_bin" ]] || {
    echo "ERROR: rustup is unavailable after bootstrap" >&2
    exit 1
}

# rustup-managed commands are proxies to rustup. Some cache/cleanup tooling can
# remove individual proxies while leaving rustup and the installed toolchain
# intact. Restore only missing proxies; never replace healthy files.
for proxy in cargo rustc rustfmt cargo-clippy clippy-driver; do
    proxy_path="$CARGO_HOME/bin/$proxy"
    if [[ ! -e "$proxy_path" ]]; then
        ln -s rustup "$proxy_path"
        echo "Restored missing rustup proxy: $proxy_path"
    fi
done

toolchain_usable() {
    "$rustup_bin" run stable rustc --version >/dev/null 2>&1 &&
        "$rustup_bin" run stable cargo --version >/dev/null 2>&1
}

if ! toolchain_usable; then
    echo "Stable Rust toolchain is missing; installing it under the shared toolchain lock."
    "$rustup_bin" toolchain install stable --profile minimal --no-self-update
fi

toolchain_usable || {
    echo "ERROR: stable Rust toolchain is unusable after installation" >&2
    exit 1
}

# Ensure the rustup proxies have a default only when they currently cannot run.
# This is serialized and does not reinstall or replace an existing toolchain.
if ! cargo --version >/dev/null 2>&1 || ! rustc --version >/dev/null 2>&1; then
    "$rustup_bin" default stable >/dev/null
fi

for component in "$@"; do
    case "$component" in
        rustfmt)
            if ! rustfmt --version >/dev/null 2>&1; then
                echo "Installing missing Rust component: rustfmt"
                "$rustup_bin" component add rustfmt --toolchain stable
            fi
            ;;
        clippy)
            if ! cargo clippy --version >/dev/null 2>&1; then
                echo "Installing missing Rust component: clippy"
                "$rustup_bin" component add clippy --toolchain stable
            fi
            ;;
        *)
            echo "ERROR: unsupported Rust component requested: $component" >&2
            exit 2
            ;;
    esac
done

cargo --version
rustc --version
